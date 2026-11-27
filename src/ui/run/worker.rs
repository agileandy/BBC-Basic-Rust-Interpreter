//! The worker thread of the RUN pane. It owns the executor and the program
//! for the life of the IDE, because the executor cannot move to another
//! thread. It shows the prompt `>`, reads a typed line or a run request, and
//! runs it with the commands of the prompt mode.
//!
//! The pane and the worker talk through 2 channels and 2 flags. The keys go
//! to the worker, and the worker edits the typed line and shows its echo, as
//! the line editor of a BBC Micro does. The Escape flag and the Quit flag
//! go to the stop check of the run loop.

use crate::commands::run_command;
use crate::executor::{Console, Executor, Interrupt};
use crate::program::ProgramStore;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The prompt of the pane.
const PROMPT: &str = "> ";
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
    /// The editor text changed: the program for the next typed line
    Program(ProgramStore),
    /// The flags changed: stop a wait for a key
    Wake,
}

/// A message from the worker to the pane.
pub(super) enum FromWorker {
    /// Text for the screen
    Text(String),
    /// Clear the screen (CLS)
    Clear,
    /// A command or a run starts
    Busy,
    /// The command ended, and the prompt shows. `error` is the text of its
    /// error. `changed` is true if the command changed the program.
    Ready {
        error: Option<String>,
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
    /// The program of the editor text, for the next command
    program: Option<ProgramStore>,
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
            program: None,
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
                Ok(ToWorker::Key(key)) => self.keys.push_back(key),
                Ok(ToWorker::Run(program)) => self.run = Some(program),
                Ok(ToWorker::Program(program)) => self.program = Some(program),
                Ok(ToWorker::Wake) => {}
                Err(RecvTimeoutError::Timeout) => return KeyWait::TimeOut,
                Err(RecvTimeoutError::Disconnected) => self.closed = true,
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
}

/// The loop of the worker thread: the prompt, then a typed line or a run
/// request, until the pane closes.
pub(super) fn run_worker(link: Link) {
    let link = Rc::new(RefCell::new(link));
    let mut executor = Executor::new();
    executor.set_console(Box::new(WorkerConsole(Rc::clone(&link))));
    let mut program = ProgramStore::new();
    link.borrow_mut().write(PROMPT);

    loop {
        let line = {
            let mut link = link.borrow_mut();
            let line = link.read_line(true);
            if let Some(editor_program) = link.program.take() {
                program = editor_program;
            }
            match (link.run.take(), line) {
                (Some(run), _) => {
                    program = run;
                    link.write("RUN\n");
                    "RUN".to_string()
                }
                (None, Some(line)) => line,
                (None, None) => return,
            }
        };

        link.borrow_mut().send(FromWorker::Busy);
        let before = program.clone();
        let result = run_command(&mut executor, &mut program, line.trim());
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
        link.new_line();
        link.write(PROMPT);
        link.send(FromWorker::Ready { error, changed });
    }
}
