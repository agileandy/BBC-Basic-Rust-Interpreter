//! BBC BASIC Interpreter
//!
//! A complete implementation of the BBC BASIC language as found on the BBC Micro Model B.
//! This interpreter emulates the original 6502-based system with 32K RAM and full
//! compatibility with BBC BASIC programs.

// The tests use 3.14 as an arbitrary real value, not as PI.
#![cfg_attr(test, allow(clippy::approx_constant))]

pub mod executor;
pub mod extensions;
pub mod filesystem;
pub mod graphics;
pub mod json;
pub mod memory;
pub mod os;
pub mod parser;
pub mod program;
pub mod runner;
pub mod sound;
pub mod tokenizer;
pub mod variables;

// Re-export core types for convenience
pub use crate::error::{BBCBasicError, Result};
pub use memory::MemoryManager;
pub use parser::{BinaryOperator, Expression, Statement, UnaryOperator};
pub use program::ProgramStore;
pub use tokenizer::{Token, TokenizedLine};
pub use variables::{VarType, Variable};

/// Core error handling types for the BBC BASIC interpreter
pub mod error {
    use std::fmt;

    /// Result type for BBC BASIC operations
    pub type Result<T> = std::result::Result<T, BBCBasicError>;

    /// Comprehensive error types matching BBC BASIC error conditions
    #[derive(Debug, Clone, PartialEq)]
    pub enum BBCBasicError {
        // Syntax errors
        SyntaxError { message: String, line: Option<u16> },
        BadProgram,

        // Runtime errors
        TypeMismatch,
        NoRoom,
        BadDim,
        SubscriptOutOfRange,
        DivisionByZero,
        StringTooLong,
        TooBig,
        // ON GOTO or ON GOSUB index out of range, with no ELSE
        OnRange,
        // READ with no DATA left
        OutOfData,
        // NEXT with no open FOR loop
        NoFor,
        // UNTIL with no open REPEAT loop
        NoRepeat,
        // ENDWHILE with no open WHILE loop
        NoWhile,
        // NEXT with a name that no open FOR loop has
        CantMatchFor,
        // LOCAL outside a procedure or function
        NotLocal,
        // GOTO, GOSUB or ON to a line that does not exist
        NoSuchLine(i32),
        // RETURN with no open GOSUB
        NoGosub,
        // A PROC or FN call with the wrong number of arguments
        Arguments,
        // ENDPROC with no open PROC
        NoProc,
        // A call to a PROC or FN that is not defined. The name has its prefix.
        NoSuchFnProc(String),

        // Variable and array errors
        NoSuchVariable(String),
        ArrayNotDimensioned(String),

        // Memory errors
        InvalidAddress(u16),
        MemoryExhausted,

        // File system errors
        FileNotFound(String),
        DiskError(String),
        ChannelNotOpen(i32),
        TooManyOpenFiles,

        // System errors
        IllegalFunction,
        BadCall,
        // MODE with a number that is not 0 to 7
        BadMode,
        // A text that is not JSON, with the detail from the parser (issue #102)
        BadJson(String),

        // Custom error for ON ERROR handling
        UserError(u8),
    }

    impl fmt::Display for BBCBasicError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                BBCBasicError::SyntaxError { message, line } => {
                    if let Some(line_num) = line {
                        write!(f, "Syntax error at line {}: {}", line_num, message)
                    } else {
                        write!(f, "Syntax error: {}", message)
                    }
                }
                BBCBasicError::BadProgram => write!(f, "Bad program"),
                BBCBasicError::TypeMismatch => write!(f, "Type mismatch"),
                BBCBasicError::NoRoom => write!(f, "No room"),
                BBCBasicError::BadDim => write!(f, "Bad DIM"),
                BBCBasicError::SubscriptOutOfRange => write!(f, "Subscript"),
                BBCBasicError::DivisionByZero => write!(f, "Division by zero"),
                BBCBasicError::StringTooLong => write!(f, "String too long"),
                BBCBasicError::TooBig => write!(f, "Too big"),
                BBCBasicError::OnRange => write!(f, "ON range"),
                BBCBasicError::OutOfData => write!(f, "Out of DATA"),
                BBCBasicError::NoFor => write!(f, "No FOR"),
                BBCBasicError::NoRepeat => write!(f, "No REPEAT"),
                BBCBasicError::NoWhile => write!(f, "No WHILE"),
                BBCBasicError::CantMatchFor => write!(f, "Can't match FOR"),
                BBCBasicError::NotLocal => write!(f, "Not LOCAL"),
                BBCBasicError::NoSuchLine(line) => write!(f, "No such line: {}", line),
                BBCBasicError::NoGosub => write!(f, "No GOSUB"),
                BBCBasicError::Arguments => write!(f, "Arguments"),
                BBCBasicError::NoProc => write!(f, "No PROC"),
                BBCBasicError::NoSuchFnProc(name) => write!(f, "No such FN/PROC: {}", name),
                BBCBasicError::NoSuchVariable(name) => write!(f, "No such variable: {}", name),
                BBCBasicError::ArrayNotDimensioned(name) => {
                    write!(f, "Array not dimensioned: {}", name)
                }
                BBCBasicError::InvalidAddress(addr) => write!(f, "Invalid address: ${:04X}", addr),
                BBCBasicError::MemoryExhausted => write!(f, "Memory exhausted"),
                BBCBasicError::FileNotFound(name) => write!(f, "File not found: {}", name),
                BBCBasicError::DiskError(msg) => write!(f, "Disk error: {}", msg),
                BBCBasicError::ChannelNotOpen(_) => write!(f, "Channel"),
                BBCBasicError::TooManyOpenFiles => write!(f, "Too many open files"),
                BBCBasicError::IllegalFunction => write!(f, "Illegal function"),
                BBCBasicError::BadMode => write!(f, "Bad MODE"),
                BBCBasicError::BadJson(detail) => write!(f, "Bad JSON: {}", detail),
                BBCBasicError::BadCall => write!(f, "Bad call"),
                BBCBasicError::UserError(code) => write!(f, "Error {}", code),
            }
        }
    }

    impl std::error::Error for BBCBasicError {}
}
