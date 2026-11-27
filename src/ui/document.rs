//! The program text of the IDE: the source that the panes share.
//!
//! The text is lines as the user typed them, in the form that `LIST` shows:
//! a line number, a space and the statements. The editor never rewrites the
//! text, except for a renumber. `to_program` makes the stored program from it.

use crate::parser::parse_line;
use crate::program::ProgramStore;
use crate::tokenizer::{detokenize, tokenize, TokenizedLine};
use crate::BBCBasicError;
use std::collections::{HashMap, HashSet};

/// The mark text for a line with no line number.
const NO_NUMBER: &str = "No line number";

/// A fault in one line of the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMark {
    /// The index of the line in the text, from 0
    pub index: usize,
    /// The BBC BASIC text of the fault
    pub message: String,
}

/// The program text, its file name and the mark for changes that are not
/// saved. The text always has 1 line or more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    lines: Vec<String>,
    file_name: Option<String>,
    changed: bool,
}

impl Document {
    /// An empty text with one empty line.
    pub fn new(file_name: Option<String>) -> Self {
        Self {
            lines: vec![String::new()],
            file_name,
            changed: false,
        }
    }

    /// A text from the lines of `text`, with no changes.
    pub fn from_text(file_name: Option<String>, text: &str) -> Self {
        let mut document = Self::new(file_name);
        document.lines = text.lines().map(str::to_string).collect();
        if document.lines.is_empty() {
            document.lines.push(String::new());
        }
        document
    }

    /// Read the file text. A file that does not exist gives an empty text
    /// with that name. Another read fault gives an empty text and a message.
    pub fn open(file_name: Option<String>) -> (Self, Option<String>) {
        let Some(name) = file_name else {
            return (Self::new(None), None);
        };
        match std::fs::read_to_string(&name) {
            Ok(text) => (Self::from_text(Some(name), &text), None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::new(Some(name)), None),
            Err(e) => {
                let message = format!("Cannot read {}: {}", name, e);
                (Self::new(Some(name)), Some(message))
            }
        }
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The text, with a newline after each line.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| format!("{}\n", line))
            .collect()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    pub fn set_file_name(&mut self, file_name: Option<String>) {
        self.file_name = file_name;
    }

    /// True after a change that is not saved.
    pub fn is_changed(&self) -> bool {
        self.changed
    }

    pub fn set_changed(&mut self, changed: bool) {
        self.changed = changed;
    }

    /// The number of characters in a line.
    pub fn line_len(&self, row: usize) -> usize {
        self.lines[row].chars().count()
    }

    /// Put `text` (one line) in line `row` before character `col`.
    pub fn insert(&mut self, row: usize, col: usize, text: &str) {
        let at = byte_index(&self.lines[row], col);
        self.lines[row].insert_str(at, text);
        self.changed = true;
    }

    /// Remove the character at `col` in line `row`.
    pub fn remove(&mut self, row: usize, col: usize) {
        let at = byte_index(&self.lines[row], col);
        self.lines[row].remove(at);
        self.changed = true;
    }

    /// Move the text after character `col` of line `row` to a new line below,
    /// after `prefix`.
    pub fn split(&mut self, row: usize, col: usize, prefix: &str) {
        let at = byte_index(&self.lines[row], col);
        let rest = self.lines[row].split_off(at);
        self.lines.insert(row + 1, format!("{}{}", prefix, rest));
        self.changed = true;
    }

    /// Put the line below `row` at the end of line `row`.
    pub fn join(&mut self, row: usize) {
        let next = self.lines.remove(row + 1);
        self.lines[row].push_str(&next);
        self.changed = true;
    }

    /// The line number at the start of a line, if it has one.
    pub fn line_number(&self, row: usize) -> Option<u16> {
        leading_number(&self.lines[row])
    }

    /// The number for a new line after line `row`: the number of the line
    /// plus 10, or the middle of the gap to the number of the next line, if
    /// the text does not use it. None if the line has no number or the gap
    /// has no free number.
    pub fn next_line_number(&self, row: usize) -> Option<u16> {
        let number = u32::from(self.line_number(row)?);
        let limit = self
            .lines
            .get(row + 1)
            .and_then(|line| leading_number(line))
            .map_or(u32::from(u16::MAX) + 1, u32::from);
        let used: HashSet<u16> = (0..self.lines.len())
            .filter_map(|r| self.line_number(r))
            .collect();
        [number + 10, number + limit.saturating_sub(number) / 2]
            .into_iter()
            .filter(|&n| n > number && n < limit)
            .filter_map(|n| u16::try_from(n).ok())
            .find(|n| !used.contains(n))
    }

    /// Make the stored program from the text. Tokenize and parse each line,
    /// and give a mark for a line with a fault, a line with no number, and
    /// the second line with a number. A marked line, an empty line and a line
    /// with only a number give no program line.
    pub fn to_program(&self) -> (ProgramStore, Vec<LineMark>) {
        self.to_program_cached(&mut LineCache::default())
    }

    /// `to_program` with the results of the last call in `cache`, so that
    /// only a line with a new text is tokenized and parsed again.
    pub fn to_program_cached(&self, cache: &mut LineCache) -> (ProgramStore, Vec<LineMark>) {
        let mut old = std::mem::take(&mut cache.0);
        let mut program = ProgramStore::new();
        let mut marks = Vec::new();
        let mut seen = HashSet::new();
        for (index, line) in self.lines.iter().enumerate() {
            let checked = match cache.0.get(line) {
                Some(checked) => checked,
                None => {
                    let checked = old.remove(line).unwrap_or_else(|| check_line(line));
                    cache.0.entry(line.clone()).or_insert(checked)
                }
            };
            let fault = match checked {
                Checked::Empty => None,
                Checked::Fault { number, .. }
                | Checked::Line(TokenizedLine {
                    line_number: number,
                    ..
                }) if number.is_some_and(|n| !seen.insert(n)) => Some(format!(
                    "Line number {} used twice",
                    number.unwrap_or_default()
                )),
                Checked::Fault { message, .. } => Some(message.clone()),
                Checked::Line(tokenized) => {
                    program.store_line(tokenized.clone());
                    None
                }
            };
            if let Some(message) = fault {
                marks.push(LineMark { index, message });
            }
        }
        (program, marks)
    }

    /// Replace the text with the listed form of each program line. The same
    /// text is not a change.
    pub fn set_program(&mut self, program: &ProgramStore) {
        let mut lines: Vec<String> = program
            .list()
            .iter()
            .filter_map(|(_, line)| detokenize(line).ok())
            .collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        if lines != self.lines {
            self.lines = lines;
            self.changed = true;
        }
    }
}

/// The result of the check of one line text, with no look at the other lines.
#[derive(Debug, Clone)]
enum Checked {
    /// An empty line or a line with only a number: no program line
    Empty,
    /// A line with a fault. The number is None if the line has no number
    /// that the tokenizer accepts.
    Fault {
        number: Option<u16>,
        message: String,
    },
    /// A correct line with a number
    Line(TokenizedLine),
}

/// The check results of the lines at the last `to_program_cached` call, by
/// line text.
#[derive(Debug, Default)]
pub struct LineCache(HashMap<String, Checked>);

/// Tokenize and parse one line.
fn check_line(line: &str) -> Checked {
    let text = line.trim();
    if text.is_empty() {
        return Checked::Empty;
    }
    let fault = |number, message| Checked::Fault { number, message };
    match tokenize(text) {
        Err(e) => fault(None, error_text(&e, leading_number(text))),
        Ok(tokenized) => match tokenized.line_number {
            None => fault(None, NO_NUMBER.to_string()),
            Some(_) if tokenized.tokens.is_empty() => Checked::Empty,
            Some(number) => match parse_line(&tokenized) {
                Err(e) => fault(Some(number), error_text(&e, Some(number))),
                Ok(_) => Checked::Line(tokenized),
            },
        },
    }
}

/// The line number at the start of a text, if it has one that fits.
fn leading_number(text: &str) -> Option<u16> {
    let digits: String = text
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The byte index of character `col` in a text.
fn byte_index(text: &str, col: usize) -> usize {
    text.char_indices().nth(col).map_or(text.len(), |(i, _)| i)
}

/// The BBC BASIC text of a fault, as RUN gives it.
fn error_text(error: &BBCBasicError, number: Option<u16>) -> String {
    match (error, number) {
        (BBCBasicError::SyntaxError { message, .. }, Some(n)) => {
            format!("Syntax error at line {}: {}", n, message)
        }
        (BBCBasicError::SyntaxError { message, .. }, None) => format!("Syntax error: {}", message),
        (_, Some(n)) => format!("{} at line {}", error, n),
        (_, None) => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(text: &str) -> Document {
        Document::from_text(None, text)
    }

    fn marks(text: &str) -> Vec<(usize, String)> {
        document(text)
            .to_program()
            .1
            .into_iter()
            .map(|m| (m.index, m.message))
            .collect()
    }

    #[test]
    fn a_new_document_has_one_empty_line_and_no_changes() {
        let doc = Document::new(Some("a.bbas".to_string()));
        assert_eq!(doc.lines(), [""]);
        assert_eq!(doc.file_name(), Some("a.bbas"));
        assert!(!doc.is_changed());
    }

    #[test]
    fn text_gives_a_newline_after_each_line() {
        assert_eq!(document("10 A=1\n20 END").text(), "10 A=1\n20 END\n");
    }

    #[test]
    fn to_program_stores_the_correct_lines_in_number_order() {
        let (program, marks) = document("20 END\n10 PRINT 1\n").to_program();
        assert_eq!(marks, []);
        assert_eq!(program.get_line_numbers(), [10, 20]);
    }

    #[test]
    fn to_program_gives_no_mark_and_no_line_for_an_empty_line() {
        let (program, marks) = document("10 END\n   \n\n20 ").to_program();
        assert_eq!(marks, []);
        assert_eq!(program.get_line_numbers(), [10]);
    }

    #[test]
    fn to_program_marks_a_tokenizer_fault() {
        assert_eq!(
            marks("10 PRINT \\"),
            [(
                0,
                "Syntax error at line 10: Unknown character '\\'".to_string()
            )]
        );
    }

    #[test]
    fn to_program_marks_a_line_number_that_does_not_fit() {
        assert_eq!(
            marks("70000 END"),
            [(0, "Syntax error: Line number out of range".to_string())]
        );
    }

    #[test]
    fn to_program_marks_a_line_with_no_number() {
        assert_eq!(marks("PRINT 1"), [(0, "No line number".to_string())]);
    }

    #[test]
    fn to_program_marks_the_second_line_with_the_same_number() {
        let (program, marks) = document("10 PRINT 1\n10 PRINT 2").to_program();
        assert_eq!(
            marks,
            [LineMark {
                index: 1,
                message: "Line number 10 used twice".to_string()
            }]
        );
        let line = program.get_line(10).unwrap();
        assert_eq!(detokenize(line).unwrap(), "10 PRINT 1");
    }

    #[test]
    fn to_program_cached_gives_the_result_of_to_program_after_each_change() {
        let mut cache = LineCache::default();
        let listing = |program: &ProgramStore| -> Vec<String> {
            program
                .list()
                .iter()
                .map(|(_, line)| detokenize(line).unwrap())
                .collect()
        };
        for text in [
            "10 PRINT 1\n20 PRINT \\\n30 END",
            "10 PRINT 1\n20 PRINT 2\n30 END",
            "10 PRINT 1\n10 PRINT 2\n30 END\n10 PRINT 1",
            "PRINT 1\n\n30 END",
        ] {
            let doc = document(text);
            let (cached, cached_marks) = doc.to_program_cached(&mut cache);
            let (full, full_marks) = doc.to_program();
            assert_eq!(cached_marks, full_marks, "{text:?}");
            assert_eq!(listing(&cached), listing(&full), "{text:?}");
        }
        // The cache keeps only the texts of the last call.
        assert_eq!(cache.0.len(), 3);
    }

    #[test]
    fn next_line_number_is_the_number_plus_10() {
        assert_eq!(document("10 END").next_line_number(0), Some(20));
    }

    #[test]
    fn next_line_number_is_the_middle_of_the_gap() {
        assert_eq!(document("10 A\n14 B").next_line_number(0), Some(12));
        assert_eq!(document("10 A\n12 B").next_line_number(0), Some(11));
    }

    #[test]
    fn next_line_number_is_none_with_no_free_number() {
        assert_eq!(document("10 A\n11 B").next_line_number(0), None);
        assert_eq!(document("65535 END").next_line_number(0), None);
        assert_eq!(document("PRINT").next_line_number(0), None);
    }

    #[test]
    fn next_line_number_does_not_use_a_number_that_the_text_uses() {
        // 20 is used later in the text, so the middle of the gap to 40.
        assert_eq!(document("10 A\n40 B\n20 C").next_line_number(0), Some(25));
    }

    #[test]
    fn next_line_number_near_the_largest_number() {
        assert_eq!(document("65530 END").next_line_number(0), Some(65533));
    }

    #[test]
    fn edits_work_on_characters_and_set_the_changed_mark() {
        let mut doc = document("10 PRINT \"é\"");
        doc.insert(0, 11, "x");
        assert_eq!(doc.lines(), ["10 PRINT \"éx\""]);
        assert!(doc.is_changed());
        doc.remove(0, 10);
        assert_eq!(doc.lines(), ["10 PRINT \"x\""]);
        doc.split(0, 8, "20 ");
        assert_eq!(doc.lines(), ["10 PRINT", "20  \"x\""]);
        doc.join(0);
        assert_eq!(doc.lines(), ["10 PRINT20  \"x\""]);
    }

    #[test]
    fn set_program_writes_the_listed_form() {
        let mut doc = document("20  END\n10 PRINT  1");
        let (program, _) = doc.to_program();
        doc.set_program(&program);
        assert_eq!(doc.lines(), ["10 PRINT 1", "20 END"]);
    }

    #[test]
    fn set_program_with_the_same_text_is_not_a_change() {
        let mut doc = document("10 PRINT 1\n20 END");
        let (program, _) = doc.to_program();
        doc.set_program(&program);
        assert!(!doc.is_changed());
        let mut empty = Document::new(None);
        empty.set_program(&ProgramStore::new());
        assert!(!empty.is_changed());
    }
}
