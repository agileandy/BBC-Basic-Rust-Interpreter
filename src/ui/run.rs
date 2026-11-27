//! The RUN pane (issue #171). Ctrl-R runs the program on a worker thread
//! with a large stack. The pane shows the text of the program on a text
//! screen, gives the typed keys to the program, and stops it at Escape.
//! When no program runs, the pane has the prompt `>` with the commands of
//! the prompt mode.
//!
//! The frame stays live during a run: the worker sends its text through a
//! channel, and the frame takes the messages at each tick. Graphics
//! statements run and show nothing: issue #173 adds the canvas.

mod worker;

use super::{Context, Outcome};
use crate::program::ProgramStore;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use worker::{Flags, FromWorker, Link, ToWorker, DELETE, ENTER};

/// The name of the worker thread. The panic hook of the IDE uses it.
pub(super) const WORKER_NAME: &str = "ide-interpreter";
/// The native stack size of the worker, as for the prompt mode (issue #151).
const STACK_SIZE: usize = 64 * 1024 * 1024;
/// The count of lines that the screen keeps for Page Up.
const HISTORY: usize = 1000;
/// The count of messages that wait for the pane before the worker waits.
const QUEUE: usize = 1024;
/// The longest time that one tick takes messages, so that the frame draws.
const TICK: Duration = Duration::from_millis(20);
/// The width and the height of the screen before the first draw.
const DEFAULT_WIDTH: usize = 80;
const DEFAULT_PAGE: usize = 20;
/// The status message when a typed command changed the program of the pane.
const CHANGED: &str = "The RUN program changed. The editor text is the source";
/// The status message for Ctrl-R during a run.
const RUNNING: &str = "A program runs. Press Escape to stop it";

/// The key codes of a BBC Micro for the cursor keys.
const LEFT: char = '\u{88}';
const RIGHT: char = '\u{89}';
const DOWN: char = '\u{8a}';
const UP: char = '\u{8b}';

/// A grid of text that scrolls up at the last line. The cursor is on the
/// last line. A line feed starts a new line at column 0, a carriage return
/// goes to column 0, and a backspace goes back one column.
#[derive(Debug, Default)]
struct Screen {
    lines: VecDeque<Vec<char>>,
    col: usize,
}

impl Screen {
    fn write(&mut self, text: &str, width: usize) {
        if self.lines.is_empty() {
            self.lines.push_back(Vec::new());
        }
        for c in text.chars() {
            match c {
                '\n' => self.new_line(),
                '\r' => self.col = 0,
                '\x08' => self.col = self.col.saturating_sub(1),
                c if c.is_control() => {}
                c => {
                    if self.col >= width {
                        self.new_line();
                    }
                    let col = self.col;
                    let line = self.lines.back_mut().expect("the screen has a line");
                    if col < line.len() {
                        line[col] = c;
                    } else {
                        line.resize(col, ' ');
                        line.push(c);
                    }
                    self.col += 1;
                }
            }
        }
    }

    fn new_line(&mut self) {
        self.lines.push_back(Vec::new());
        self.col = 0;
        while self.lines.len() > HISTORY {
            self.lines.pop_front();
        }
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.col = 0;
    }

    /// The count of lines, with the line of the cursor.
    fn len(&self) -> usize {
        self.lines.len().max(1)
    }
}

/// The pane end of the worker.
struct Worker {
    input: Sender<ToWorker>,
    output: Receiver<FromWorker>,
    flags: Arc<Flags>,
    thread: JoinHandle<()>,
}

/// The state of the RUN pane.
#[derive(Default)]
pub struct RunPane {
    screen: Screen,
    worker: Option<Worker>,
    /// True from the Busy message to the Ready message of the worker
    busy: bool,
    /// True from a run request or an Enter at the prompt to the Busy message
    starting: bool,
    /// True if a key went to the prompt after the last Ready message
    typed: bool,
    /// The line number of the error of the last command
    error_line: Option<u16>,
    /// The document line for the edit mode to show
    edit_request: Option<usize>,
    /// The program of the editor text that the worker has
    synced: Option<ProgramStore>,
    /// The count of lines above the last page of the screen
    scroll: usize,
    /// The width and the height of the pane at the last draw
    width: Cell<usize>,
    page: Cell<usize>,
    /// True if a message came after the last tick
    changed: bool,
}

impl RunPane {
    /// True if a program or a command runs.
    pub fn is_running(&self) -> bool {
        self.busy || self.starting
    }

    /// The text for the status line.
    pub fn status(&self) -> String {
        if self.is_running() {
            "running"
        } else {
            "ready"
        }
        .to_string()
    }

    /// The document line of an error, after Enter on an empty prompt line.
    pub fn take_edit_request(&mut self) -> Option<usize> {
        self.edit_request.take()
    }

    /// The user comes to the run mode: start the worker, so that the prompt
    /// shows.
    pub fn open(&mut self, ctx: &mut Context) {
        self.start_worker(ctx);
    }

    /// Run the program (Ctrl-R). The frame made it from the document.
    pub fn start(&mut self, program: ProgramStore, ctx: &mut Context) {
        if self.is_running() {
            ctx.message = RUNNING.to_string();
            return;
        }
        if !self.start_worker(ctx) {
            return;
        }
        self.synced = Some(program.clone());
        self.send(ToWorker::Run(program));
        self.starting = true;
        self.error_line = None;
        self.scroll = 0;
    }

    /// Start the worker thread if it does not run. Give false if the system
    /// refuses the thread.
    fn start_worker(&mut self, ctx: &mut Context) -> bool {
        if self.worker.is_some() {
            return true;
        }
        let (input, worker_input) = mpsc::channel();
        let (worker_output, output) = mpsc::sync_channel(QUEUE);
        let flags = Arc::new(Flags::default());
        let link = Link::new(worker_input, worker_output, Arc::clone(&flags));
        let spawned = thread::Builder::new()
            .name(WORKER_NAME.to_string())
            .stack_size(STACK_SIZE)
            .spawn(move || worker::run_worker(link));
        match spawned {
            Ok(thread) => {
                self.worker = Some(Worker {
                    input,
                    output,
                    flags,
                    thread,
                });
                self.synced = None;
                true
            }
            Err(error) => {
                ctx.message = format!("No interpreter thread: {}", error);
                false
            }
        }
    }

    fn send(&mut self, message: ToWorker) {
        if let Some(worker) = &self.worker {
            let _ = worker.input.send(message);
        }
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &mut Context) -> Outcome {
        let idle = !self.is_running();
        match key.code {
            KeyCode::PageUp if idle => {
                let max = self.screen.len().saturating_sub(self.page());
                self.scroll = (self.scroll + self.page()).min(max);
                return Outcome::Used;
            }
            KeyCode::PageDown if idle => {
                self.scroll = self.scroll.saturating_sub(self.page());
                return Outcome::Used;
            }
            KeyCode::Esc => {
                if let (false, Some(worker)) = (idle, &self.worker) {
                    worker.flags.escape.store(true, Ordering::SeqCst);
                    let _ = worker.input.send(ToWorker::Wake);
                }
                return Outcome::Used;
            }
            KeyCode::Enter if idle && !self.typed && self.error_line.is_some() => {
                let number = self.error_line;
                let doc = &ctx.document;
                if let Some(row) = (0..doc.lines().len()).find(|&r| doc.line_number(r) == number) {
                    self.edit_request = Some(row);
                    return Outcome::Used;
                }
            }
            _ => {}
        }
        let Some(code) = key_code(key) else {
            return Outcome::Ignored;
        };
        if !self.start_worker(ctx) {
            return Outcome::Used;
        }
        if idle && code == ENTER {
            self.sync_program(ctx);
            self.starting = true;
            self.error_line = None;
        } else if idle {
            self.typed = true;
        }
        self.send(ToWorker::Key(code));
        self.scroll = 0;
        Outcome::Used
    }

    /// Give the worker the program of the editor text, if it changed after
    /// the last run request or the last typed line.
    fn sync_program(&mut self, ctx: &Context) {
        let same = self
            .synced
            .as_ref()
            .is_some_and(|synced| synced.list() == ctx.program.list());
        if !same {
            self.synced = Some(ctx.program.clone());
            self.send(ToWorker::Program(ctx.program.clone()));
        }
    }

    /// Take the messages of the worker for a short time, so that the frame
    /// can draw and read the keys. Give true if a message came.
    pub fn tick(&mut self, ctx: &mut Context) -> bool {
        self.take_messages(ctx, Instant::now() + TICK, false);
        std::mem::take(&mut self.changed)
    }

    /// Take the messages of the worker until the run ends, for a maximum of
    /// `limit`. Give true if no program runs.
    pub fn wait_for_run(&mut self, ctx: &mut Context, limit: Duration) -> bool {
        self.take_messages(ctx, Instant::now() + limit, true)
    }

    fn take_messages(&mut self, ctx: &mut Context, deadline: Instant, until_ready: bool) -> bool {
        loop {
            if until_ready && !self.is_running() {
                return true;
            }
            let Some(worker) = &self.worker else {
                return !self.is_running();
            };
            let message = if until_ready {
                worker
                    .output
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            } else {
                worker.output.try_recv().map_err(|error| match error {
                    mpsc::TryRecvError::Empty => RecvTimeoutError::Timeout,
                    mpsc::TryRecvError::Disconnected => RecvTimeoutError::Disconnected,
                })
            };
            match message {
                Ok(message) => self.apply(message, ctx),
                Err(RecvTimeoutError::Timeout) => return !self.is_running(),
                Err(RecvTimeoutError::Disconnected) => {
                    self.worker_ended(ctx);
                    return true;
                }
            }
            if Instant::now() >= deadline {
                return !self.is_running();
            }
        }
    }

    fn apply(&mut self, message: FromWorker, ctx: &mut Context) {
        self.changed = true;
        match message {
            FromWorker::Text(text) => self.screen.write(&text, self.width()),
            FromWorker::Clear => self.screen.clear(),
            FromWorker::Busy => {
                self.busy = true;
                self.starting = false;
            }
            FromWorker::Ready { error, changed } => {
                self.busy = false;
                self.typed = false;
                self.error_line = error.as_deref().and_then(error_line);
                // An Escape for the run that ended must not stop the next one
                if let Some(worker) = &self.worker {
                    worker.flags.escape.store(false, Ordering::SeqCst);
                }
                if changed {
                    ctx.message = CHANGED.to_string();
                }
            }
        }
    }

    /// The worker thread ended with no request: the interpreter panicked.
    /// The next run starts a new worker.
    fn worker_ended(&mut self, ctx: &mut Context) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        let panic = worker.thread.join().err();
        let detail = panic
            .as_ref()
            .and_then(|payload| {
                payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
            })
            .unwrap_or("no detail");
        ctx.message = format!("The interpreter stopped: {}", detail);
        self.changed = true;
        self.busy = false;
        self.starting = false;
    }

    /// Stop the worker and wait for its end, for a maximum of `limit`. Give
    /// true if it ended. A program stops at its next statement, also if it
    /// traps Escape.
    pub fn stop(&mut self, limit: Duration) -> bool {
        let Some(worker) = self.worker.take() else {
            return true;
        };
        worker.flags.quit.store(true, Ordering::SeqCst);
        let _ = worker.input.send(ToWorker::Wake);
        // The worker cannot wait to send text or to read a key now
        drop(worker.input);
        drop(worker.output);
        let deadline = Instant::now() + limit;
        while !worker.thread.is_finished() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        let _ = worker.thread.join();
        self.busy = false;
        self.starting = false;
        true
    }

    fn width(&self) -> usize {
        match self.width.get() {
            0 => DEFAULT_WIDTH,
            width => width,
        }
    }

    fn page(&self) -> usize {
        match self.page.get() {
            0 => DEFAULT_PAGE,
            page => page,
        }
    }

    /// Draw the text screen. Issue #173 adds the canvas for the graphics.
    pub fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &Context) {
        let style = Style::new().fg(Color::White).bg(Color::Black);
        let height = usize::from(area.height);
        self.width.set(usize::from(area.width));
        self.page.set(height);
        let total = self.screen.len();
        let scroll = self.scroll.min(total.saturating_sub(height));
        let start = total.saturating_sub(height + scroll);
        let rows: Vec<Line> = self
            .screen
            .lines
            .iter()
            .skip(start)
            .take(height)
            .map(|line| Line::from(line.iter().collect::<String>()))
            .collect();
        frame.render_widget(Paragraph::new(rows).style(style), area);
        if scroll == 0 && area.width > 0 && area.height > 0 {
            let x = self.screen.col.min(usize::from(area.width) - 1);
            let y = total - 1 - start;
            let position = Position::new(area.x + x as u16, area.y + y as u16);
            if let Some(cell) = frame.buffer_mut().cell_mut(position) {
                cell.set_style(style.add_modifier(Modifier::REVERSED));
            }
        }
    }
}

impl Drop for RunPane {
    fn drop(&mut self) {
        self.stop(Duration::from_secs(2));
    }
}

/// The key code that a key gives to the program, if it gives one.
fn key_code(key: KeyEvent) -> Option<char> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    Some(match key.code {
        KeyCode::Char(c) => c,
        KeyCode::Enter => ENTER,
        KeyCode::Backspace => DELETE,
        KeyCode::Left => LEFT,
        KeyCode::Right => RIGHT,
        KeyCode::Down => DOWN,
        KeyCode::Up => UP,
        _ => return None,
    })
}

/// The line number in the text of an error: "... at line N".
fn error_line(message: &str) -> Option<u16> {
    const AT_LINE: &str = " at line ";
    let rest = &message[message.find(AT_LINE)? + AT_LINE.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_wraps_at_the_width_and_keeps_1000_lines() {
        let mut screen = Screen::default();
        screen.write("abcde", 3);
        assert_eq!(screen.lines, [vec!['a', 'b', 'c'], vec!['d', 'e']]);
        for _ in 0..HISTORY + 5 {
            screen.write("x\n", 3);
        }
        assert_eq!(screen.lines.len(), HISTORY);
    }

    #[test]
    fn a_carriage_return_and_a_backspace_move_the_cursor_on_the_line() {
        let mut screen = Screen::default();
        screen.write("abc\rX\x08Y", 80);
        assert_eq!(screen.lines, [vec!['Y', 'b', 'c']]);
    }

    #[test]
    fn the_error_line_comes_from_the_text_of_the_error() {
        assert_eq!(error_line("Division by zero at line 20"), Some(20));
        assert_eq!(error_line("Syntax error at line 10: x"), Some(10));
        assert_eq!(error_line("Escape"), None);
    }
}
