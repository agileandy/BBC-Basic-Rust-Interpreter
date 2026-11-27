//! Program storage and management for BBC BASIC
//!
//! Manages BBC BASIC program lines in tokenized format with automatic sorting.

use crate::tokenizer::{Token, TokenizedLine};
use std::collections::BTreeMap;

/// The line key of a line typed with no line number. It is wider than a
/// stored line number, so it cannot be the key of a stored line.
pub const TYPED_LINE: u32 = u16::MAX as u32 + 1;

/// The position of a statement in a running program: a line key (a stored
/// line number or `TYPED_LINE`) and an index in the statements of that line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub index: usize,
}

impl Position {
    /// The first statement of a stored line
    pub fn line_start(line: u16) -> Self {
        Self {
            line: line.into(),
            index: 0,
        }
    }

    /// The stored line number of the position (None for the typed line)
    pub fn stored_line(&self) -> Option<u16> {
        u16::try_from(self.line).ok()
    }
}

/// Program line storage with execution support
#[derive(Debug, Clone)]
pub struct ProgramStore {
    /// Stored program lines (line_number -> TokenizedLine)
    lines: BTreeMap<u16, TokenizedLine>,
}

impl ProgramStore {
    /// Create a new program store
    pub fn new() -> Self {
        Self {
            lines: BTreeMap::new(),
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

    /// Delete each line from `start` to `end`, the 2 numbers included (DELETE command)
    pub fn delete_range(&mut self, start: u16, end: u16) {
        self.lines
            .retain(|line_number, _| !(start..=end).contains(line_number));
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

    /// The size of the program in BBC Micro memory: for each line, 4 header
    /// bytes and the bytes of its tokens, then 2 bytes for the end marker.
    /// TOP is PAGE plus this size.
    pub fn size_in_bytes(&self) -> usize {
        const LINE_HEADER: usize = 4; // CR, line number high and low, length
        const END_MARKER: usize = 2; // CR, &FF
        self.lines
            .values()
            .map(|line| LINE_HEADER + line.tokens.iter().map(token_size).sum::<usize>())
            .sum::<usize>()
            + END_MARKER
    }
}

/// The number of bytes that a token uses in a stored line
fn token_size(token: &Token) -> usize {
    match token {
        Token::Keyword(_) | Token::Operator(_) | Token::Separator(_) => 1,
        Token::ExtendedKeyword(..) => 2,
        Token::LineNumber(_) => 4, // &8D and 3 encoded bytes
        Token::Integer(value) => value.to_string().len(),
        Token::Real(value) => value.to_string().len(),
        Token::String(text) => text.len() + 2,
        Token::DataText(text) | Token::Identifier(text) => text.len(),
        Token::EndOfLine => 0,
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
    use crate::tokenizer::tokenize;

    #[test]
    fn test_size_of_empty_program_is_end_marker() {
        assert_eq!(ProgramStore::new().size_in_bytes(), 2);
    }

    #[test]
    fn test_size_counts_line_header_and_tokens() {
        let mut store = ProgramStore::new();
        // 4 header bytes, 1 byte for the END token, 2 end marker bytes
        store.store_line(tokenize("10 END").unwrap());
        assert_eq!(store.size_in_bytes(), 7);

        // 4 header bytes, 1 PRINT token, 2 quotes and 2 characters
        store.store_line(tokenize("20 PRINT \"AB\"").unwrap());
        assert_eq!(store.size_in_bytes(), 7 + 9);
    }

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
    fn test_delete_range_removes_lines_from_start_to_end() {
        let mut store = ProgramStore::new();
        for line in ["10 END", "20 END", "30 END", "40 END", "50 END"] {
            store.store_line(tokenize(line).unwrap());
        }

        store.delete_range(20, 40);

        assert_eq!(store.get_line_numbers(), vec![10, 50]);
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
}
