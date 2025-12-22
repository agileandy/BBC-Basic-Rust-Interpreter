//! BBC BASIC Interpreter
//!
//! A complete implementation of the BBC BASIC language as found on the BBC Micro Model B.
//! This interpreter emulates the original 6502-based system with 32K RAM and full
//! compatibility with BBC BASIC programs.

pub mod executor;
pub mod filesystem;
pub mod graphics;
pub mod memory;
pub mod os;
pub mod parser;
pub mod program;
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
        SubscriptOutOfRange,
        DivisionByZero,
        StringTooLong,

        // Variable and array errors
        NoSuchVariable(String),
        ArrayNotDimensioned(String),

        // Memory errors
        InvalidAddress(u16),
        MemoryExhausted,

        // File system errors
        FileNotFound(String),
        DiskError(String),

        // System errors
        IllegalFunction,
        BadCall,

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
                BBCBasicError::SubscriptOutOfRange => write!(f, "Subscript out of range"),
                BBCBasicError::DivisionByZero => write!(f, "Division by zero"),
                BBCBasicError::StringTooLong => write!(f, "String too long"),
                BBCBasicError::NoSuchVariable(name) => write!(f, "No such variable: {}", name),
                BBCBasicError::ArrayNotDimensioned(name) => {
                    write!(f, "Array not dimensioned: {}", name)
                }
                BBCBasicError::InvalidAddress(addr) => write!(f, "Invalid address: ${:04X}", addr),
                BBCBasicError::MemoryExhausted => write!(f, "Memory exhausted"),
                BBCBasicError::FileNotFound(name) => write!(f, "File not found: {}", name),
                BBCBasicError::DiskError(msg) => write!(f, "Disk error: {}", msg),
                BBCBasicError::IllegalFunction => write!(f, "Illegal function"),
                BBCBasicError::BadCall => write!(f, "Bad call"),
                BBCBasicError::UserError(code) => write!(f, "Error {}", code),
            }
        }
    }

    impl std::error::Error for BBCBasicError {}
}
