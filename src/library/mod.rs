//! The libraries of `INSTALL file$` (issue #185).
//!
//! A library is a second set of program lines that is not part of the listed
//! program. The program calls its procedures and functions as its own. The
//! reference is the INSTALL entry of the BBC BASIC for Windows manual:
//! https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin5.html#install
//!
//! The rules of the manual:
//! - The last library that defines a name wins. A routine of the program
//!   wins over each library.
//! - The libraries "remain resident until the BASIC program's window is
//!   closed (they are not cleared by a CHAIN statement)". RUN and CHAIN keep
//!   them. The manual has no entry for NEW and LOAD: here they remove them.
//! - "Line numbers and labels are not recognised in library files". Here a
//!   library line can have a number or not. A GOTO goes only to a line with
//!   a number.
//! - "INSTALL checks to see if the library is already loaded, and if so it
//!   does nothing", with "a case-sensitive comparison of the supplied
//!   path/filename". The same file with a different name text loads again.
//!
//! The run loop parses the lines and runs them. This module keeps the
//! installed files and reads a file.

use crate::error::BBCBasicError;
use crate::tokenizer::{tokenize, TokenizedLine};

/// One line of a library file
#[derive(Debug, Clone)]
pub struct LibraryLine {
    /// The count of the line in the file, from 1. Empty lines count.
    pub place: u16,
    /// The tokens of the line. `line_number` is the number at the start of
    /// the line, if it has one.
    pub line: TokenizedLine,
}

/// One installed library
#[derive(Debug, Clone)]
pub struct Library {
    /// The file name as the INSTALL statement supplied it
    pub name: String,
    /// The file name with the extension, for the error messages
    pub path: String,
    /// The lines of the file that have statements, in file order
    pub lines: Vec<LibraryLine>,
}

/// The installed libraries, in the order of the INSTALL statements.
/// Library `n` of the store has the line set `n + 1` in the run loop: the
/// program has the line set 0.
#[derive(Debug, Clone, Default)]
pub struct LibraryStore {
    libraries: Vec<Library>,
}

impl LibraryStore {
    /// True if a library with this supplied name is installed. As in the
    /// manual, the comparison is case-sensitive and uses the text of the name.
    pub fn is_installed(&self, name: &str) -> bool {
        self.libraries.iter().any(|library| library.name == name)
    }

    /// Add a library after the others
    pub fn add(&mut self, library: Library) {
        self.libraries.push(library);
    }

    /// Remove each library (NEW and LOAD)
    pub fn clear(&mut self) {
        self.libraries.clear();
    }

    /// The installed libraries, in the order of the INSTALL statements
    pub fn libraries(&self) -> &[Library] {
        &self.libraries
    }
}

/// The Bad library error of a fault at a place in a library file. A syntax
/// error gives only its detail.
pub fn fault_at(place: u16, path: &str, error: &BBCBasicError) -> BBCBasicError {
    let message = match error {
        BBCBasicError::SyntaxError { message, .. } => message.clone(),
        other => other.to_string(),
    };
    BBCBasicError::BadLibrary(format!("line {} of \"{}\": {}", place, path, message))
}

/// Read and tokenize each line of the library file with the supplied name.
/// The name rule is that of LOAD: the extension `.bbas` is added when the
/// name has none, and the name is relative to the working directory.
///
/// A file that is not there gives File not found. A line that does not
/// tokenize gives Bad library with its place in the file. ON ERROR can catch
/// both, and no part of a bad file is used.
pub fn read_library(name: &str) -> Result<Library, BBCBasicError> {
    let path = crate::commands::resolve_program_path(name);
    let content = std::fs::read_to_string(&path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => BBCBasicError::FileNotFound(path.clone()),
        _ => BBCBasicError::DiskError(format!("{}: {}", path, error)),
    })?;

    let mut lines = Vec::new();
    for (index, text) in content.lines().enumerate() {
        let place = u16::try_from(index + 1).map_err(|_| {
            BBCBasicError::BadLibrary(format!("\"{}\" has more than 65535 lines", path))
        })?;
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let line = tokenize(text).map_err(|error| fault_at(place, &path, &error))?;
        // A line number with no statement is an empty line
        if !line.tokens.is_empty() {
            lines.push(LibraryLine { place, line });
        }
    }
    Ok(Library {
        name: name.to_string(),
        path,
        lines,
    })
}
