//! The RUN pane (issue #171). Ctrl-R runs the program on a worker thread
//! with a large stack. The pane shows the text of the program on a text
//! screen, gives the typed keys to the program, and stops it at Escape.
//! When no program runs, the pane has the prompt `>` with the commands of
//! the prompt mode.
//!
//! The frame stays live during a run: the worker sends its text through a
//! channel, and the frame takes the messages at each tick. Graphics
//! statements run and show nothing: issue #173 adds the canvas.
//!
//! The editor text is the source. Each typed line runs with the program of
//! the editor text, and a typed line is refused while the text has a fault.

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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use worker::{Answer, Flags, FromWorker, Link, ToWorker, DELETE, ENTER};

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

/// One row of the screen.
#[derive(Debug, Default)]
struct Row {
    chars: Vec<char>,
    /// True if the row continues the row above it: the text wrapped
    continued: bool,
}

/// A grid of text that scrolls up at the last line. A line feed goes to
/// column 0 of the next row, a carriage return goes to column 0, and a
/// backspace goes back one column. A backspace at column 0 of a wrapped row
/// goes to the last column of the row above it.
#[derive(Debug, Default)]
struct Screen {
    rows: VecDeque<Row>,
    /// The cursor: the row index and the column
    row: usize,
    col: usize,
}

impl Screen {
    fn write(&mut self, text: &str, width: usize) {
        if self.rows.is_empty() {
            self.rows.push_back(Row::default());
        }
        for c in text.chars() {
            match c {
                '\n' => self.next_row(false),
                '\r' => self.col = 0,
                '\x08' if self.col > 0 => self.col -= 1,
                '\x08' if self.rows[self.row].continued && self.row > 0 => {
                    self.row -= 1;
                    self.col = width - 1;
                }
                c if c.is_control() => {}
                c => {
                    if self.col >= width {
                        self.next_row(true);
                    }
                    let col = self.col;
                    let chars = &mut self.rows[self.row].chars;
                    if col < chars.len() {
                        chars[col] = c;
                    } else {
                        chars.resize(col, ' ');
                        chars.push(c);
                    }
                    self.col += 1;
                }
            }
        }
    }

    /// Go to column 0 of the next row. Add a row at the end if there is none.
    fn next_row(&mut self, continued: bool) {
        if self.row + 1 < self.rows.len() {
            self.row += 1;
        } else {
            self.rows.push_back(Row::default());
            self.row = self.rows.len() - 1;
            while self.rows.len() > HISTORY {
                self.rows.pop_front();
                self.row -= 1;
            }
        }
        self.rows[self.row].continued = continued;
        self.col = 0;
    }

    fn clear(&mut self) {
        self.rows.clear();
        self.row = 0;
        self.col = 0;
    }

    /// The count of rows, with the row of the cursor.
    fn len(&self) -> usize {
        self.rows.len().max(1)
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
    /// Worker threads that a stop asked to end, which did not end in time
    stopping: Vec<JoinHandle<()>>,
    /// True from the Busy message to the Ready message of the worker
    busy: bool,
    /// True from a run request or an Enter at the prompt to the Busy message
    starting: bool,
    /// True from a run request (Ctrl-R) to its Busy message
    run_pending: bool,
    /// The line number of the error of the last run of the editor program
    error_line: Option<u16>,
    /// The document line for the edit mode to show
    edit_request: Option<usize>,
    /// The program of the editor text that the worker has
    synced: Option<ProgramStore>,
    /// The count of rows above the last page of the screen
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

    /// The document line to show in the edit mode: the line of an error
    /// after Enter on an empty prompt line, or the fault of the text.
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
        self.clear_escape();
        self.send(ToWorker::Run(program));
        self.starting = true;
        self.run_pending = true;
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

    /// A new command starts: an Escape before this belongs to the command
    /// before it. The worker starts a command only after this go-ahead.
    fn clear_escape(&self) {
        if let Some(worker) = &self.worker {
            worker.flags.escape.store(false, Ordering::SeqCst);
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
            _ => {}
        }
        let Some(code) = key_code(key) else {
            return Outcome::Ignored;
        };
        if !self.start_worker(ctx) {
            return Outcome::Used;
        }
        if idle && code == ENTER {
            // The worker sends the typed line for an answer
            self.starting = true;
        }
        self.send(ToWorker::Key(code));
        self.scroll = 0;
        Outcome::Used
    }

    /// The answer for a typed line of the worker.
    ///
    /// - During a run request, skip the line: the run comes first.
    /// - An empty line after an error of the editor program goes to the line
    ///   of the error in the edit mode.
    /// - While the editor text has a fault, refuse each line that is not
    ///   empty: each statement can call a PROC or FN of the program, so no
    ///   typed line can run with a part of the program. Show the fault in
    ///   the edit mode, as Ctrl-R does.
    /// - If not, run the line with the program of the editor text.
    fn answer(&mut self, line: &str, ctx: &mut Context) -> Answer {
        if self.run_pending {
            return Answer::Skip;
        }
        let error_line = self.error_line.take();
        let doc = &ctx.document;
        if line.trim().is_empty() {
            let row = error_line.and_then(|number| {
                (0..doc.lines().len()).find(|&r| doc.line_number(r) == Some(number))
            });
            if let Some(row) = row {
                self.edit_request = Some(row);
                return Answer::Skip;
            }
            return Answer::Run(None);
        }
        let (program, marks) = doc.to_program();
        if let Some(mark) = marks.first() {
            self.edit_request = Some(mark.index);
            ctx.message = mark.message.clone();
            return Answer::Skip;
        }
        let same = self
            .synced
            .as_ref()
            .is_some_and(|synced| synced.list() == program.list());
        if same {
            return Answer::Run(None);
        }
        self.synced = Some(program.clone());
        Answer::Run(Some(program))
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

    /// Take each message that waits. Then, with `until_ready`, wait for more
    /// messages while a program runs. Give true if no program runs.
    fn take_messages(&mut self, ctx: &mut Context, deadline: Instant, until_ready: bool) -> bool {
        loop {
            let Some(worker) = &self.worker else {
                return !self.is_running();
            };
            let message = match worker.output.try_recv() {
                Ok(message) => Ok(message),
                Err(TryRecvError::Empty) if until_ready && self.is_running() => worker
                    .output
                    .recv_timeout(deadline.saturating_duration_since(Instant::now())),
                Err(TryRecvError::Empty) => return !self.is_running(),
                Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
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
            FromWorker::Line(line) => {
                let answer = self.answer(&line, ctx);
                match answer {
                    Answer::Run(_) => {
                        self.clear_escape();
                        self.starting = true;
                    }
                    Answer::Skip => self.starting = self.run_pending,
                }
                self.send(ToWorker::Answer(answer));
            }
            FromWorker::Busy => {
                self.busy = true;
                self.starting = false;
                self.run_pending = false;
            }
            FromWorker::Ready {
                error_line,
                changed,
            } => {
                self.busy = false;
                self.error_line = error_line;
                if changed {
                    // The worker program is not the editor program now: the
                    // next typed line gets the editor program again
                    self.synced = None;
                    ctx.message = CHANGED.to_string();
                }
            }
        }
    }

    /// The state of a run that no worker runs now.
    fn reset_run(&mut self) {
        self.busy = false;
        self.starting = false;
        self.run_pending = false;
        self.error_line = None;
        self.edit_request = None;
        self.synced = None;
    }

    /// The worker thread ended with no request: the interpreter panicked.
    /// The next run starts a new worker, with its prompt on a new line.
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
        self.reset_run();
        if self.screen.col > 0 {
            self.screen.write("\n", self.width());
        }
        self.changed = true;
    }

    /// Stop the worker and wait for its end, for a maximum of `limit`. Give
    /// true if it ended. A program stops at its next statement, also if it
    /// traps Escape. A worker that does not end in time stays in a list, and
    /// the next stop waits for it again.
    pub fn stop(&mut self, limit: Duration) -> bool {
        if let Some(worker) = self.worker.take() {
            worker.flags.quit.store(true, Ordering::SeqCst);
            let _ = worker.input.send(ToWorker::Wake);
            // The worker cannot wait to send text or to read a key now
            drop(worker.input);
            drop(worker.output);
            self.stopping.push(worker.thread);
        }
        self.reset_run();
        let deadline = Instant::now() + limit;
        while self.stopping.iter().any(|thread| !thread.is_finished()) {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        for thread in self.stopping.drain(..) {
            let _ = thread.join();
        }
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
            .rows
            .iter()
            .skip(start)
            .take(height)
            .map(|row| Line::from(row.chars.iter().collect::<String>()))
            .collect();
        frame.render_widget(Paragraph::new(rows).style(style), area);
        let on_page = (start..start + height).contains(&self.screen.row);
        if scroll == 0 && on_page && area.width > 0 {
            let x = self.screen.col.min(usize::from(area.width) - 1);
            let y = self.screen.row - start;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    use crate::ui::Document;

    const LIMIT: Duration = Duration::from_secs(5);

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn context(text: &str) -> Context {
        let document = Document::from_text(None, text);
        let program = document.to_program().0;
        Context {
            document,
            program,
            executor: Executor::new(),
            message: String::new(),
        }
    }

    fn text(screen: &Screen) -> Vec<String> {
        screen
            .rows
            .iter()
            .map(|row| row.chars.iter().collect::<String>().trim_end().to_string())
            .collect()
    }

    /// A worker whose thread runs `run`. The channels of the worker end
    /// are closed.
    fn fake_worker(run: impl FnOnce() + Send + 'static) -> Worker {
        let (input, _) = mpsc::channel();
        let (_, output) = mpsc::sync_channel(1);
        Worker {
            input,
            output,
            flags: Arc::new(Flags::default()),
            thread: thread::spawn(run),
        }
    }

    #[test]
    fn the_screen_wraps_at_the_width_and_keeps_1000_lines() {
        let mut screen = Screen::default();
        screen.write("abcde", 3);
        assert_eq!(text(&screen), ["abc", "de"]);
        for _ in 0..HISTORY + 5 {
            screen.write("x\n", 3);
        }
        assert_eq!(screen.rows.len(), HISTORY);
        assert_eq!(screen.row, HISTORY - 1);
    }

    #[test]
    fn a_carriage_return_and_a_backspace_move_the_cursor_on_the_line() {
        let mut screen = Screen::default();
        screen.write("abc\rX\x08Y", 80);
        assert_eq!(text(&screen), ["Ybc"]);
    }

    #[test]
    fn a_backspace_at_column_0_of_a_wrapped_row_goes_to_the_row_above() {
        let mut screen = Screen::default();
        screen.write("abcde", 3);
        screen.write("\x08 \x08\x08 \x08\x08 \x08", 3);
        // 3 backspaces remove e, d and c
        assert_eq!(text(&screen), ["ab", ""]);
        assert_eq!((screen.row, screen.col), (0, 2));
        // No row above a row that did not wrap
        screen.write("\n\x08Z", 3);
        assert_eq!(text(&screen), ["ab", "Z"]);
    }

    #[test]
    fn an_escape_before_a_typed_line_starts_does_not_stop_it() {
        // K2: the first run ends, the line typed during it waits for its
        // answer, and then Escape comes, before the pane sees the end
        let program_text = "10 FOR I% = 1 TO 20000: NEXT\n20 PRINT \"A\"";
        let mut ctx = context(program_text);
        let mut pane = RunPane::default();
        pane.start(ctx.program.clone(), &mut ctx);
        for c in "RUN".chars() {
            pane.key(press(KeyCode::Char(c)), &mut ctx);
        }
        pane.key(press(KeyCode::Enter), &mut ctx);
        let mut waiting = Vec::new();
        let deadline = Instant::now() + LIMIT;
        loop {
            let output = &pane.worker.as_ref().expect("a worker").output;
            let message = output.recv_timeout(LIMIT).expect("a message");
            let is_line = matches!(message, FromWorker::Line(_));
            waiting.push(message);
            if is_line {
                break;
            }
            assert!(Instant::now() < deadline);
        }
        pane.key(press(KeyCode::Esc), &mut ctx);
        for message in waiting {
            pane.apply(message, &mut ctx);
        }
        assert!(pane.wait_for_run(&mut ctx, LIMIT));
        let rows = text(&pane.screen);
        assert_eq!(rows, ["> RUN", "A", "> RUN", "A", ">"]);
    }

    #[test]
    fn a_worker_that_ended_leaves_no_state_of_its_run() {
        // K7
        let mut ctx = context("10 END");
        let mut pane = RunPane::default();
        pane.worker = Some(fake_worker(|| {}));
        pane.busy = true;
        pane.error_line = Some(10);
        pane.screen.write("> PRI", 80);
        assert!(pane.wait_for_run(&mut ctx, LIMIT));
        assert!(!pane.is_running());
        assert_eq!(pane.error_line, None);
        assert_eq!(pane.screen.col, 0);
        assert!(
            ctx.message.starts_with("The interpreter stopped"),
            "{}",
            ctx.message
        );
    }

    #[test]
    fn a_stop_that_times_out_leaves_no_run_and_waits_again_later() {
        // K8
        let (release, wait) = mpsc::channel::<()>();
        let mut pane = RunPane::default();
        pane.worker = Some(fake_worker(move || {
            let _ = wait.recv();
        }));
        pane.busy = true;
        pane.starting = true;
        assert!(!pane.stop(Duration::from_millis(10)));
        assert!(!pane.is_running());
        drop(release);
        assert!(pane.stop(LIMIT));
        assert!(pane.stopping.is_empty());
    }
}
