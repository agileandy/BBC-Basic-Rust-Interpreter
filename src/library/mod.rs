//! The libraries of `INSTALL file$` (issue #185).
//!
//! A library is a second set of program lines that is not part of the listed
//! program. The program calls its procedures and functions as its own. The
//! reference is the INSTALL entry of the BBC BASIC for Windows manual:
//! https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin5.html#install
//!
//! The issue sets 3 rules that are different from that manual: the first
//! library that defines a name wins (the manual: the last), CHAIN removes the
//! libraries (the manual: CHAIN keeps them), and a GOTO in a library goes to a
//! line of the library (the manual: a library has no line numbers).
//!
//! The run loop parses the lines and runs them. This module keeps the
//! installed files and reads a file.

use crate::error::BBCBasicError;
use crate::program::ProgramStore;

/// One installed library: its file name and its lines
#[derive(Debug, Clone)]
pub struct Library {
    /// The file name as INSTALL used it, with the extension
    pub path: String,
    /// The tokenized lines of the file
    pub lines: ProgramStore,
}

/// The installed libraries, in the order of the INSTALL statements.
/// Library `n` of the store has the line set `n + 1` in the run loop: the
/// program has the line set 0.
#[derive(Debug, Clone, Default)]
pub struct LibraryStore {
    libraries: Vec<Library>,
}

impl LibraryStore {
    /// True if a library with this file name is installed. As in the
    /// manual, the comparison is case-sensitive.
    pub fn is_installed(&self, path: &str) -> bool {
        self.libraries.iter().any(|library| library.path == path)
    }

    /// Add a library after the others
    pub fn add(&mut self, library: Library) {
        self.libraries.push(library);
    }

    /// Remove each library (RUN, CHAIN, NEW and LOAD)
    pub fn clear(&mut self) {
        self.libraries.clear();
    }

    /// The installed libraries, in the order of the INSTALL statements
    pub fn libraries(&self) -> &[Library] {
        &self.libraries
    }
}

/// Why a library file did not load.
#[derive(Debug, PartialEq)]
pub enum ReadError {
    /// A BASIC error, such as File not found. ON ERROR can catch it.
    Basic(BBCBasicError),
    /// The file text is not a program. The run stops.
    Fatal(String),
}

/// The file name of a library. The rule is that of LOAD: the extension
/// `.bbas` is added when the name has none. The name is relative to the
/// working directory.
pub fn library_path(file: &str) -> String {
    crate::commands::resolve_program_path(file)
}

/// Read and tokenize each line of the library file at `path`. A bad line
/// gives an error, and no part of the file is used.
pub fn read_library(path: &str) -> Result<Library, ReadError> {
    let path = path.to_string();
    let content = std::fs::read_to_string(&path).map_err(|error| {
        ReadError::Basic(match error.kind() {
            std::io::ErrorKind::NotFound => BBCBasicError::FileNotFound(path.clone()),
            _ => BBCBasicError::DiskError(format!("{}: {}", path, error)),
        })
    })?;
    let lines = crate::commands::parse_program(&content)
        .map_err(|message| ReadError::Fatal(format!("Bad library \"{}\": {}", path, message)))?;
    Ok(Library { path, lines })
}
