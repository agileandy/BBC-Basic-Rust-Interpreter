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

use crate::commands::run_command;
use crate::executor::{Console, Executor, Interrupt};
use crate::graphics::ScreenMode;
use crate::program::ProgramStore;
use crate::tokenizer::expand_short_form;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
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
}

/// The flags that the pane sets and the stop check reads.
#[derive(Default)]
pub(super) struct Flags {
    pub escape: AtomicBool,
    pub quit: AtomicBool,
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
}

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
        }
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
    fn next_key(&mut self, limit: Option<Duration>, at_prompt: bool) -> KeyWait {
        let deadline = limit.map(|limit| Instant::now() + limit);
        loop {
            if self.stopped(at_prompt) {
                return KeyWait::Stop;
            }
            if let Some(key) = self.keys.pop_front() {
                return KeyWait::Key(key);
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
    fn read_line(&mut self, at_prompt: bool) -> Option<String> {
        let mut line = String::new();
        loop {
            match self.next_key(None, at_prompt) {
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
        self.0.borrow_mut().read_line(false)
    }

    fn read_key(&mut self, limit: Option<Duration>) -> Option<char> {
        match self.0.borrow_mut().next_key(limit, false) {
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
}

/// A command for the worker: the text, and true if it is a run request of
/// the pane (Ctrl-R).
struct Command {
    line: String,
    run_request: bool,
}

/// Read the next command: a run request, or a typed line that the pane
/// accepts. None if the pane closes.
fn next_command(link: &mut Link, program: &mut ProgramStore) -> Option<Command> {
    loop {
        let line = link.read_line(true);
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
        let Some(command) = next_command(&mut link.borrow_mut(), &mut program) else {
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
