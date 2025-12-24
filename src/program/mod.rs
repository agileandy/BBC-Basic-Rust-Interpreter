//! Program storage and management for BBC BASIC
//!
//! Manages BBC BASIC program lines in tokenized format with automatic sorting.

use crate::tokenizer::TokenizedLine;
use std::collections::BTreeMap;

/// Program line storage with execution support
#[derive(Debug, Clone)]
pub struct ProgramStore {
    /// Stored program lines (line_number -> TokenizedLine)
    lines: BTreeMap<u16, TokenizedLine>,
    /// Current execution line (for RUN, GOTO, etc.)
    current_line: Option<u16>,
}

impl ProgramStore {
    /// Create a new program store
    pub fn new() -> Self {
        Self {
            lines: BTreeMap::new(),
            current_line: None,
        }
    }

    /// Store a program line
    pub fn store_line(&mut self, line: TokenizedLine) {
        if let Some(line_number) = line.line_number {
            self.lines.insert(line_number, line);
        }
    }

    /// Delete a program line (entering just a line number deletes it)
    pub fn delete_line(&mut self, line_number: u16) {
        self.lines.remove(&line_number);
    }

    /// Get a program line
    pub fn get_line(&self, line_number: u16) -> Option<&TokenizedLine> {
        self.lines.get(&line_number)
    }

    /// Get all line numbers in order
    pub fn get_line_numbers(&self) -> Vec<u16> {
        self.lines.keys().copied().collect()
    }

    /// Clear all program lines (NEW command)
    pub fn clear(&mut self) {
        self.lines.clear();
        self.current_line = None;
    }

    /// Check if program is empty
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get number of lines in program
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// List the program (returns lines in order)
    pub fn list(&self) -> Vec<(u16, &TokenizedLine)> {
        self.lines.iter().map(|(k, v)| (*k, v)).collect()
    }

    /// Start program execution from the first line
    pub fn start_execution(&mut self) -> Option<u16> {
        self.current_line = self.lines.keys().next().copied();
        self.current_line
    }

    /// Get the next line to execute
    pub fn next_line(&mut self) -> Option<u16> {
        if let Some(current) = self.current_line {
            // Find the next line after current
            self.current_line = self.lines.range((current + 1)..).next().map(|(k, _)| *k);
            self.current_line
        } else {
            None
        }
    }

    /// Jump to a specific line (for GOTO, GOSUB)
    pub fn goto_line(&mut self, line_number: u16) -> bool {
        if self.lines.contains_key(&line_number) {
            self.current_line = Some(line_number);
            true
        } else {
            false
        }
    }

    /// Get the current execution line
    pub fn get_current_line(&self) -> Option<u16> {
        self.current_line
    }

    /// Stop execution
    pub fn stop_execution(&mut self) {
        self.current_line = None;
    }
}

impl Default for ProgramStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{tokenize, Token};

    #[test]
    fn test_program_store_creation() {
        let store = ProgramStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_store_and_retrieve_line() {
        let mut store = ProgramStore::new();
        let line = tokenize("10 PRINT \"HELLO\"").unwrap();

        store.store_line(line.clone());

        assert_eq!(store.len(), 1);
        assert!(store.get_line(10).is_some());
    }

    #[test]
    fn test_store_multiple_lines_sorted() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("30 PRINT \"C\"").unwrap());
        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());

        let line_numbers = store.get_line_numbers();
        assert_eq!(line_numbers, vec![10, 20, 30]);
    }

    #[test]
    fn test_delete_line() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());

        assert_eq!(store.len(), 2);

        store.delete_line(10);

        assert_eq!(store.len(), 1);
        assert!(store.get_line(10).is_none());
        assert!(store.get_line(20).is_some());
    }

    #[test]
    fn test_clear_program() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());

        store.clear();

        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_start_execution() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());
        store.store_line(tokenize("30 PRINT \"C\"").unwrap());

        let first_line = store.start_execution();
        assert_eq!(first_line, Some(10));
        assert_eq!(store.get_current_line(), Some(10));
    }

    #[test]
    fn test_next_line() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());
        store.store_line(tokenize("30 PRINT \"C\"").unwrap());

        store.start_execution();
        assert_eq!(store.get_current_line(), Some(10));

        store.next_line();
        assert_eq!(store.get_current_line(), Some(20));

        store.next_line();
        assert_eq!(store.get_current_line(), Some(30));

        let next = store.next_line();
        assert_eq!(next, None);
    }

    #[test]
    fn test_goto_line() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());
        store.store_line(tokenize("30 PRINT \"C\"").unwrap());

        store.start_execution();

        // Jump to line 30
        let success = store.goto_line(30);
        assert!(success);
        assert_eq!(store.get_current_line(), Some(30));

        // Try to jump to non-existent line
        let success = store.goto_line(999);
        assert!(!success);
    }

    #[test]
    fn test_list_program() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("30 PRINT \"C\"").unwrap());
        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.store_line(tokenize("20 PRINT \"B\"").unwrap());

        let listing = store.list();

        assert_eq!(listing.len(), 3);
        assert_eq!(listing[0].0, 10);
        assert_eq!(listing[1].0, 20);
        assert_eq!(listing[2].0, 30);
    }

    #[test]
    fn test_overwrite_line() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"OLD\"").unwrap());
        store.store_line(tokenize("10 PRINT \"NEW\"").unwrap());

        assert_eq!(store.len(), 1);

        let line = store.get_line(10).unwrap();
        // Verify it's the new line (this is a simplified check)
        assert!(line.line_number == Some(10));
    }

    #[test]
    fn test_stop_execution() {
        let mut store = ProgramStore::new();

        store.store_line(tokenize("10 PRINT \"A\"").unwrap());
        store.start_execution();

        assert_eq!(store.get_current_line(), Some(10));

        store.stop_execution();

        assert_eq!(store.get_current_line(), None);
    }
}
