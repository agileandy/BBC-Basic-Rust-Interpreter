//! Operating system interface for BBC BASIC
//!
//! Handles OS calls and ROM functionality.

/// Operating system interface
#[derive(Debug)]
pub struct OSInterface {
    // Implementation will be added in later tasks
}

impl OSInterface {
    /// Create a new OS interface
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for OSInterface {
    fn default() -> Self {
        Self::new()
    }
}
