//! The worker thread of the RUN pane. It owns the executor and the program
//! for the life of the IDE, because the executor cannot move to another
//! thread. It shows the prompt `>`, reads a typed line or a run request, and
//! runs it with the commands of the prompt mode.
//!
//! The pane and the worker talk through 2 channels and 2 flags. The keys go
//! to the worker, and the worker edits the typed line and shows its echo, as
//! the line editor of a BBC Micro does. The Escape flag and the Quit flag
//! go to the stop check of the run loop.
//!
//! A typed line does not run at once. The worker sends it to the pane and
//! waits for the answer, because only the pane knows the editor text: the
//! pane can refuse the line, or give the program of the editor text. Thus
//! a line that the user typed during a run also gets this check.
//!
//! The worker also sends the picture of the graphics canvas (issue #173),
//! scaled to the picture area that the size flag gives. It sends a picture
//! only if the canvas, the mode or the size changed: during a run at most
//! 30 times in one second, and at the end of each command.

use crate::commands::run_command;
use crate::executor::{Console, Executor, Interrupt};
use crate::graphics::{GraphicsSystem, ScreenMode};
use crate::program::ProgramStore;
use crate::tokenizer::expand_short_form;
use crate::ui::canvas::Picture;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The prompt of the pane.
const PROMPT: &str = "> ";
/// The start text of a BBC Micro, before the first prompt (issue #174).
const BANNER: &str = "BBC Computer 32K\n\nBASIC\n\n";
/// The key code of Enter.
pub(super) const ENTER: char = '\r';
/// The key code of the Delete key of a BBC Micro (Backspace).
pub(super) const DELETE: char = '\x7f';

/// A message from the pane to the worker.
pub(super) enum ToWorker {
    /// A typed key
    Key(char),
    /// Ctrl-R: run this program
    Run(ProgramStore),
    /// The answer for the typed line that the worker sent
    Answer(Answer),
    /// The flags changed: stop a wait for a key
    Wake,
}

/// The answer of the pane for a typed line.
pub(super) enum Answer {
    /// Run the line, first with this program if the editor text changed
    Run(Option<ProgramStore>),
    /// Do not run the line: show the prompt again
    Skip,
}

/// A message from the worker to the pane.
pub(super) enum FromWorker {
    /// Text for the screen
    Text(String),
    /// Clear the screen (CLS)
    Clear,
    /// MODE selected this screen mode. A Clear comes after it.
    Mode(ScreenMode),
    /// The logical text colours: the foreground and the background
    Colours(u8, u8),
    /// A typed line, with no line end. The worker waits for the answer.
    Line(String),
    /// A command or a run starts
    Busy,
    /// The command ended, and the prompt shows. `error_line` is the line
    /// number of an error of the editor program (a run with Ctrl-R, or a
    /// typed RUN). `changed` is true if the command changed the program.
    Ready {
        error_line: Option<u16>,
        changed: bool,
    },
    /// The picture of the canvas, or None if the mode has no graphics
    Picture(Option<Picture>),
}

/// The shortest time between 2 pictures during a run: 30 in one second.
const PICTURE_GAP: Duration = Duration::from_micros(1_000_000 / 30);

/// The flags that the pane sets and the stop check reads.
#[derive(Default)]
pub(super) struct Flags {
    pub escape: AtomicBool,
    pub quit: AtomicBool,
    /// The picture area in cells: the width in the high 16 bits and the
    /// height in the low 16 bits. The pane sets it with `pack_size`.
    pub size: AtomicU32,
}

/// The value of the size flag for an area of `columns` by `rows` cells.
pub(super) fn pack_size(columns: u16, rows: u16) -> u32 {
    (u32::from(columns) << 16) | u32::from(rows)
}

/// What the last picture showed. If it is the same now, the worker sends
/// no picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    /// The change count of the canvas (0 in a mode with no graphics)
    changes: u64,
    /// The size flag (0 in a mode with no graphics)
    size: u32,
    graphics: bool,
    colours: u8,
}

/// The result of a wait for a key.
enum KeyWait {
    Key(char),
    TimeOut,
    /// An interrupt, the end of the pane, or (at the prompt) a run request
    Stop,
}

/// The worker end of the channels, with the keys and the requests that came
/// before the worker could use them.
pub(super) struct Link {
    input: Receiver<ToWorker>,
    output: SyncSender<FromWorker>,
    flags: Arc<Flags>,
    keys: VecDeque<char>,
    /// A run request that came during a command
    run: Option<ProgramStore>,
    /// The answer for the last typed line
    answer: Option<Answer>,
    /// True if the pane closed its end of a channel
    closed: bool,
    /// True if the cursor is at the start of a line
    at_line_start: bool,
    /// What the last picture showed, and when the worker sent it
    shown: Option<Shown>,
    shown_at: Option<Instant>,
}

/// The canvas and the screen mode of the executor, for a wait at the prompt.
type View<'a> = Option<(&'a GraphicsSystem, ScreenMode)>;

impl Link {
    pub(super) fn new(
        input: Receiver<ToWorker>,
        output: SyncSender<FromWorker>,
        flags: Arc<Flags>,
    ) -> Self {
        Self {
            input,
            output,
            flags,
            keys: VecDeque::new(),
            run: None,
            answer: None,
            closed: false,
            at_line_start: true,
            shown: None,
            shown_at: None,
        }
    }

    /// Send the picture of the canvas if the canvas, the mode or the size
    /// changed after the last picture. With no `force`, wait for the gap
    /// after the last picture. A canvas that no statement changed since the
    /// start gives no picture.
    fn picture(&mut self, graphics: &GraphicsSystem, mode: ScreenMode, force: bool) {
        if graphics.changes() == 0 {
            return;
        }
        let size = self.flags.size.load(Ordering::Relaxed);
        let shown = Shown {
            changes: if mode.has_graphics {
                graphics.changes()
            } else {
                0
            },
            size: if mode.has_graphics { size } else { 0 },
            graphics: mode.has_graphics,
            colours: mode.colours,
        };
        if self.shown == Some(shown) {
            return;
        }
        let now = Instant::now();
        let early = self
            .shown_at
            .is_some_and(|at| now.duration_since(at) < PICTURE_GAP);
        if early && !force {
            return;
        }
        let (columns, rows) = ((size >> 16) as usize, (size & 0xFFFF) as usize);
        self.send(FromWorker::Picture(Picture::of(
            graphics, mode, columns, rows,
        )));
        self.shown = Some(shown);
        self.shown_at = Some(now);
    }

    fn send(&mut self, message: FromWorker) {
        if self.output.send(message).is_err() {
            self.closed = true;
        }
    }

    fn write(&mut self, text: &str) {
        if let Some(last) = text.chars().last() {
            self.at_line_start = last == '\n';
            self.send(FromWorker::Text(text.to_string()));
        }
    }

    /// Start a new line if the cursor is not at the start of a line.
    fn new_line(&mut self) {
        if !self.at_line_start {
            self.write("\n");
        }
    }

    fn is_quit(&self) -> bool {
        self.closed || self.flags.quit.load(Ordering::SeqCst)
    }

    /// True if a wait must stop. At the prompt, Escape does nothing and a
    /// run request stops the wait. In a program, Escape stops the wait.
    fn stopped(&self, at_prompt: bool) -> bool {
        self.is_quit()
            || if at_prompt {
                self.run.is_some()
            } else {
                self.flags.escape.load(Ordering::SeqCst)
            }
    }

    /// Keep a message of the pane for later use.
    fn keep(&mut self, message: ToWorker) {
        match message {
            ToWorker::Key(key) => self.keys.push_back(key),
            ToWorker::Run(program) => self.run = Some(program),
            ToWorker::Answer(answer) => self.answer = Some(answer),
            ToWorker::Wake => {}
        }
    }

    /// Wait for the next key, for a maximum of `limit` if there is a limit.
    /// With a view of the canvas, a new size of the pane gets a new picture
    /// during the wait.
    fn next_key(&mut self, limit: Option<Duration>, at_prompt: bool, view: View) -> KeyWait {
        let deadline = limit.map(|limit| Instant::now() + limit);
        loop {
            if self.stopped(at_prompt) {
                return KeyWait::Stop;
            }
            if let Some(key) = self.keys.pop_front() {
                return KeyWait::Key(key);
            }
            if let Some((graphics, mode)) = view {
                self.picture(graphics, mode, true);
            }
            let message = match deadline {
                None => self
                    .input
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected),
                Some(deadline) => self
                    .input
                    .recv_timeout(deadline.saturating_duration_since(Instant::now())),
            };
            match message {
                Ok(message) => self.keep(message),
                Err(RecvTimeoutError::Timeout) => return KeyWait::TimeOut,
                Err(RecvTimeoutError::Disconnected) => self.closed = true,
            }
        }
    }

    /// Wait for the answer of the pane for a typed line. None if the pane
    /// closes.
    fn wait_for_answer(&mut self) -> Option<Answer> {
        loop {
            if let Some(answer) = self.answer.take() {
                return Some(answer);
            }
            if self.is_quit() {
                return None;
            }
            match self.input.recv() {
                Ok(message) => self.keep(message),
                Err(_) => self.closed = true,
            }
        }
    }

    /// Read one typed line, with its echo. Enter ends the line, and the
    /// Delete key removes the last character. Give None if the wait stops.
    fn read_line(&mut self, at_prompt: bool, view: View) -> Option<String> {
        let mut line = String::new();
        loop {
            match self.next_key(None, at_prompt, view) {
                KeyWait::Key(ENTER) => {
                    self.write("\n");
                    return Some(line);
                }
                KeyWait::Key(DELETE | '\x08') => {
                    if line.pop().is_some() {
                        self.write("\x08 \x08");
                    }
                }
                KeyWait::Key(key) if !key.is_control() => {
                    line.push(key);
                    self.write(&key.to_string());
                }
                KeyWait::Key(_) | KeyWait::TimeOut => {}
                KeyWait::Stop => {
                    if !line.is_empty() && at_prompt {
                        // A run request: the next prompt is on a new line
                        self.write("\n");
                        self.write(PROMPT);
                    }
                    return None;
                }
            }
        }
    }
}

/// The console that the worker gives to its executor.
struct WorkerConsole(Rc<RefCell<Link>>);

impl Console for WorkerConsole {
    fn write(&mut self, text: &str) {
        self.0.borrow_mut().write(text);
    }

    fn clear(&mut self) {
        let mut link = self.0.borrow_mut();
        link.at_line_start = true;
        link.send(FromWorker::Clear);
    }

    fn read_line(&mut self) -> Option<String> {
        self.0.borrow_mut().read_line(false, None)
    }

    fn read_key(&mut self, limit: Option<Duration>) -> Option<char> {
        match self.0.borrow_mut().next_key(limit, false, None) {
            KeyWait::Key(key) => Some(key),
            KeyWait::TimeOut | KeyWait::Stop => None,
        }
    }

    fn interrupt(&mut self) -> Option<Interrupt> {
        let mut link = self.0.borrow_mut();
        if link.is_quit() {
            return Some(Interrupt::Quit);
        }
        if link.flags.escape.swap(false, Ordering::SeqCst) {
            // As on a BBC Micro, Escape empties the key buffer
            link.keys.clear();
            return Some(Interrupt::Escape);
        }
        None
    }

    fn screen_mode(&mut self, mode: ScreenMode) {
        self.0.borrow_mut().send(FromWorker::Mode(mode));
    }

    fn text_colours(&mut self, foreground: u8, background: u8) {
        self.0
            .borrow_mut()
            .send(FromWorker::Colours(foreground, background));
    }

    fn see_graphics(&mut self, graphics: &GraphicsSystem, mode: ScreenMode) {
        self.0.borrow_mut().picture(graphics, mode, false);
    }
}

/// A command for the worker: the text, and true if it is a run request of
/// the pane (Ctrl-R).
struct Command {
    line: String,
    run_request: bool,
}

/// Read the next command: a run request, or a typed line that the pane
/// accepts. None if the pane closes.
fn next_command(link: &mut Link, program: &mut ProgramStore, view: View) -> Option<Command> {
    loop {
        let line = link.read_line(true, view);
        if let Some(run) = link.run.take() {
            *program = run;
            link.write("RUN\n");
            return Some(Command {
                line: "RUN".to_string(),
                run_request: true,
            });
        }
        let line = line?;
        link.send(FromWorker::Line(line.clone()));
        match link.wait_for_answer()? {
            Answer::Run(editor_program) => {
                if let Some(editor_program) = editor_program {
                    *program = editor_program;
                }
                return Some(Command {
                    line,
                    run_request: false,
                });
            }
            Answer::Skip => {
                link.new_line();
                link.write(PROMPT);
            }
        }
    }
}

/// The loop of the worker thread: the prompt, then a typed line or a run
/// request, until the pane closes.
pub(super) fn run_worker(link: Link) {
    let link = Rc::new(RefCell::new(link));
    let mut executor = Executor::new();
    executor.set_console(Box::new(WorkerConsole(Rc::clone(&link))));
    let mut program = ProgramStore::new();
    link.borrow_mut().write(BANNER);
    link.borrow_mut().write(PROMPT);

    loop {
        let view = Some((executor.graphics(), executor.screen_mode()));
        let Some(command) = next_command(&mut link.borrow_mut(), &mut program, view) else {
            return;
        };

        link.borrow_mut().send(FromWorker::Busy);
        let before = program.clone();
        let result = run_command(&mut executor, &mut program, command.line.trim());
        let changed = before.list() != program.list();

        let mut link = link.borrow_mut();
        let error = result.err();
        if let Some(error) = &error {
            link.new_line();
            link.write(&format!("Error: {}\n", error));
        }
        if link.is_quit() {
            return;
        }
        // Only an error of the editor program has a line of the editor
        let editor_run = !changed && (command.run_request || is_run(&command.line));
        let error_line = error.as_deref().filter(|_| editor_run).and_then(error_line);
        // The last picture of the command comes before the end of the run
        link.picture(executor.graphics(), executor.screen_mode(), true);
        link.new_line();
        link.write(PROMPT);
        link.send(FromWorker::Ready {
            error_line,
            changed,
        });
    }
}

/// True if the typed line is the command RUN, also as a short form.
fn is_run(line: &str) -> bool {
    expand_short_form(line.trim()).eq_ignore_ascii_case("run")
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
    use std::sync::mpsc;

    /// A link with a picture area of 4 by 2 cells, and the pane end of its
    /// output channel. The input channel stays open.
    fn link() -> (Link, Receiver<FromWorker>, mpsc::Sender<ToWorker>) {
        let (input, worker_input) = mpsc::channel();
        let (worker_output, output) = mpsc::sync_channel(16);
        let flags = Arc::new(Flags::default());
        flags.size.store(pack_size(4, 2), Ordering::Relaxed);
        (Link::new(worker_input, worker_output, flags), output, input)
    }

    fn pictures(output: &Receiver<FromWorker>) -> Vec<Option<Picture>> {
        output
            .try_iter()
            .filter_map(|message| match message {
                FromWorker::Picture(picture) => Some(picture),
                _ => None,
            })
            .collect()
    }

    fn mode(number: i32) -> ScreenMode {
        ScreenMode::from_number(number).expect("a mode")
    }

    #[test]
    fn a_canvas_that_did_not_change_gives_no_picture() {
        let (mut link, output, _input) = link();
        let mut graphics = GraphicsSystem::new();
        // No statement drew since the start
        link.picture(&graphics, mode(0), true);
        assert!(pictures(&output).is_empty());
        graphics.plot(69, 0, 0);
        link.picture(&graphics, mode(0), true);
        link.picture(&graphics, mode(0), true);
        assert_eq!(pictures(&output).len(), 1);
        // A new size gives a new picture of the same canvas
        link.flags.size.store(pack_size(8, 4), Ordering::Relaxed);
        link.picture(&graphics, mode(0), true);
        assert_eq!(pictures(&output).len(), 1);
    }

    #[test]
    fn during_a_run_the_pictures_come_at_most_30_times_in_one_second() {
        let (mut link, output, _input) = link();
        let mut graphics = GraphicsSystem::new();
        graphics.plot(69, 0, 0);
        link.picture(&graphics, mode(0), false);
        graphics.plot(69, 4, 0);
        // Directly after a picture: no picture, then one after the gap
        link.picture(&graphics, mode(0), false);
        assert_eq!(pictures(&output).len(), 1);
        link.shown_at = Some(Instant::now() - PICTURE_GAP);
        link.picture(&graphics, mode(0), false);
        assert_eq!(pictures(&output).len(), 1);
        // The end of a command does not wait for the gap
        graphics.plot(69, 8, 0);
        link.picture(&graphics, mode(0), true);
        assert_eq!(pictures(&output).len(), 1);
    }

    #[test]
    fn a_mode_with_no_graphics_gives_one_empty_picture() {
        let (mut link, output, _input) = link();
        let mut graphics = GraphicsSystem::new();
        graphics.clear();
        link.picture(&graphics, mode(3), true);
        graphics.clear();
        link.flags.size.store(pack_size(8, 4), Ordering::Relaxed);
        link.picture(&graphics, mode(3), true);
        assert_eq!(pictures(&output), [None]);
    }

    #[test]
    fn the_error_line_comes_from_the_text_of_the_error() {
        assert_eq!(error_line("Division by zero at line 20"), Some(20));
        assert_eq!(error_line("Syntax error at line 10: x"), Some(10));
        assert_eq!(error_line("Escape"), None);
    }

    #[test]
    fn run_is_the_run_command_also_as_a_short_form() {
        assert!(is_run(" RUN "));
        assert!(is_run("run"));
        assert!(!is_run("RUN 10"));
        assert!(!is_run("LIST"));
    }
}
