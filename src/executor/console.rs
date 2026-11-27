//! The seam between the interpreter and the screen and keyboard of a front
//! end (issue #171).
//!
//! With no console, the executor uses standard output and standard input,
//! as the command prompt does. The IDE gives a console that sends the text
//! to its run pane and takes the keys from it.

use std::time::Duration;

/// A request to stop the program. The run loop asks for it before each
/// statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupt {
    /// The Escape key: the error `Escape` (ERR 17). ON ERROR can trap it.
    Escape,
    /// The front end closes: the program stops at once, with no error.
    Quit,
}

/// The screen and the keyboard of a front end.
pub trait Console {
    /// Show program text. A line feed starts a new line, a carriage return
    /// goes to column 0, and a backspace goes back one column.
    fn write(&mut self, text: &str);

    /// Clear the text screen (CLS).
    fn clear(&mut self);

    /// Read one line for INPUT, with no line end. None if an interrupt
    /// stopped the read, or at the end of input.
    fn read_line(&mut self) -> Option<String>;

    /// Read one key for GET or INKEY. With a limit, wait for a maximum of
    /// the limit. None if no key comes.
    fn read_key(&mut self, limit: Option<Duration>) -> Option<char>;

    /// Give the pending interrupt, if there is one. An Escape stops the
    /// program one time, so this call clears it. A Quit stays.
    fn interrupt(&mut self) -> Option<Interrupt>;
}

/// The console of an executor, if a front end gave one.
#[derive(Default)]
pub(super) struct ConsoleSlot(pub(super) Option<Box<dyn Console>>);

impl std::fmt::Debug for ConsoleSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Some(<console>)"
        } else {
            "None"
        })
    }
}
