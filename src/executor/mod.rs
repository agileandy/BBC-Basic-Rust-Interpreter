//! Execution engine for BBC BASIC statements
//! 
//! Executes parsed BBC BASIC statements with proper control flow handling.

use crate::error::Result;
use crate::parser::Statement;

/// BBC BASIC statement executor
#[derive(Debug)]
pub struct Executor {
    // Implementation will be added in later tasks
}

impl Executor {
    /// Create a new executor
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a statement
    pub fn execute_statement(&mut self, _statement: &Statement) -> Result<()> {
        // Implementation will be added in task 8
        Ok(())
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}