//! File system operations for BBC BASIC
//! 
//! Handles file I/O operations and star commands.

/// File system interface
#[derive(Debug)]
pub struct FileSystem {
    // Implementation will be added in later tasks
}

impl FileSystem {
    /// Create a new file system interface
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FileSystem {
    fn default() -> Self {
        Self::new()
    }
}