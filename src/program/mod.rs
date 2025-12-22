//! Program storage and management for BBC BASIC
//! 
//! Manages BBC BASIC program lines in tokenized format with automatic sorting.

use std::collections::BTreeMap;

/// Program line storage
#[derive(Debug, Clone)]
pub struct ProgramStore {
    lines: BTreeMap<u16, Vec<u8>>,
}

impl ProgramStore {
    /// Create a new program store
    pub fn new() -> Self {
        Self {
            lines: BTreeMap::new(),
        }
    }

    /// Store a program line
    pub fn store_line(&mut self, line_number: u16, tokenized_data: Vec<u8>) {
        self.lines.insert(line_number, tokenized_data);
    }

    /// Get a program line
    pub fn get_line(&self, line_number: u16) -> Option<&Vec<u8>> {
        self.lines.get(&line_number)
    }

    /// Delete a program line
    pub fn delete_line(&mut self, line_number: u16) {
        self.lines.remove(&line_number);
    }

    /// Clear all program lines
    pub fn clear_program(&mut self) {
        self.lines.clear();
    }
}

impl Default for ProgramStore {
    fn default() -> Self {
        Self::new()
    }
}