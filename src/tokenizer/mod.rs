//! Tokenizer for BBC BASIC source code
//! 
//! Converts BBC BASIC source code into internal token representation compatible
//! with the original BBC Micro tokenized format.

use crate::error::Result;
use std::collections::HashMap;

/// Represents a single token in BBC BASIC
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Single-byte keyword tokens (0x7F-0xFF)
    Keyword(u8),
    /// Extended tokens with prefix bytes (0xC6, 0xC7, 0xC8)
    ExtendedKeyword(u8, u8),
    /// Line number reference (0x8D prefix + 2 bytes)
    LineNumber(u16),
    /// Integer literal
    Integer(i32),
    /// Real number literal
    Real(f64),
    /// String literal
    String(String),
    /// Variable or procedure name
    Identifier(String),
    /// Operators (+, -, *, etc.)
    Operator(char),
    /// Separators (,, ;, :)
    Separator(char),
    /// End of line marker
    EndOfLine,
}

/// A complete tokenized line with line number and tokens
#[derive(Debug, Clone, PartialEq)]
pub struct TokenizedLine {
    pub line_number: Option<u16>,
    pub tokens: Vec<Token>,
}

impl TokenizedLine {
    /// Create a new tokenized line
    pub fn new(line_number: Option<u16>, tokens: Vec<Token>) -> Self {
        Self { line_number, tokens }
    }

    /// Create an empty tokenized line
    pub fn empty() -> Self {
        Self {
            line_number: None,
            tokens: Vec::new(),
        }
    }
}

/// Tokenize a BBC BASIC source line
pub fn tokenize(source_line: &str) -> Result<TokenizedLine> {
    // For now, return a basic implementation
    // Full implementation will be done in task 2
    Ok(TokenizedLine::empty())
}

/// Convert tokens back to BBC BASIC source
pub fn detokenize(tokenized_line: &TokenizedLine) -> Result<String> {
    // For now, return a basic implementation
    // Full implementation will be done in task 2
    Ok(String::new())
}

// BBC BASIC keyword to token mappings
// Main keywords (0x80-0xFF) - corrected to match BBC BASIC specification
const MAIN_KEYWORDS: &[(&str, u8)] = &[
    // Operators and logical functions (0x80-0x8C)
    ("AND", 0x80),
    ("DIV", 0x81),
    ("EOR", 0x82),
    ("MOD", 0x83),
    ("OR", 0x84),
    ("ERROR", 0x85),
    ("LINE", 0x86),
    ("OFF", 0x87),
    ("STEP", 0x88),
    ("SPC", 0x89),
    ("TAB", 0x8A),
    ("ELSE", 0x8B),
    ("THEN", 0x8C),
    // Line number reference marker (0x8D is special - handled separately)
    
    // Functions and system variables (0x8E-0xC5)
    ("OPENIN", 0x8E),
    ("PTR", 0x8F),
    ("PAGE", 0x90),
    ("TIME", 0x91),
    ("LOMEM", 0x92),
    ("HIMEM", 0x93),
    ("ABS", 0x94),
    ("ACS", 0x95),
    ("ADVAL", 0x96),
    ("ASC", 0x97),
    ("ASN", 0x98),
    ("ATN", 0x99),
    ("BGET", 0x9A),
    ("COS", 0x9B),
    ("COUNT", 0x9C),
    ("DEG", 0x9D),
    ("ERL", 0x9E),
    ("ERR", 0x9F),
    ("EVAL", 0xA0),
    ("EXP", 0xA1),
    ("EXT", 0xA2),
    ("FALSE", 0xA3),
    ("FN", 0xA4),
    ("GET", 0xA5),
    ("INKEY", 0xA6),
    ("INSTR", 0xA7),
    ("INT", 0xA8),
    ("LEN", 0xA9),
    ("LN", 0xAA),
    ("LOG", 0xAB),
    ("NOT", 0xAC),
    ("OPENOUT", 0xAD),
    ("OPENUP", 0xAE),
    ("PI", 0xAF),
    ("POINT", 0xB0),
    ("POS", 0xB1),
    ("RAD", 0xB2),
    ("RND", 0xB3),
    ("SGN", 0xB4),
    ("SIN", 0xB5),
    ("SQR", 0xB6),
    ("TAN", 0xB7),
    ("TO", 0xB8),
    ("TRUE", 0xB9),
    ("USR", 0xBA),
    ("VAL", 0xBB),
    ("VPOS", 0xBC),
    ("CHR$", 0xBD),
    ("GET$", 0xBE),
    ("INKEY$", 0xBF),
    ("LEFT$", 0xC0),
    ("MID$", 0xC1),
    ("RIGHT$", 0xC2),
    ("STR$", 0xC3),
    ("STRING$", 0xC4),
    ("EOF", 0xC5),
    
    // Commands and statements (0xC6-0xFF) - these are NOT extended tokens
    ("AUTO", 0xC6),
    ("DELETE", 0xC7),
    ("LOAD", 0xC8),
    ("LIST", 0xC9),
    ("NEW", 0xCA),
    ("OLD", 0xCB),
    ("RENUMBER", 0xCC),
    ("SAVE", 0xCD),
    ("EDIT", 0xCE),
    ("PTR", 0xCF),
    ("PAGE", 0xD0),
    ("TIME", 0xD1),
    ("LOMEM", 0xD2),
    ("HIMEM", 0xD3),
    ("SOUND", 0xD4),
    ("BPUT", 0xD5),
    ("CALL", 0xD6),
    ("CHAIN", 0xD7),
    ("CLEAR", 0xD8),
    ("CLOSE", 0xD9),
    ("CLG", 0xDA),
    ("CLS", 0xDB),
    ("DATA", 0xDC),
    ("DEF", 0xDD),
    ("DIM", 0xDE),
    ("DRAW", 0xDF),
    ("END", 0xE0),
    ("ENDPROC", 0xE1),
    ("ENVELOPE", 0xE2),
    ("FOR", 0xE3),
    ("GOSUB", 0xE4),
    ("GOTO", 0xE5),
    ("GCOL", 0xE6),
    ("IF", 0xE7),
    ("INPUT", 0xE8),
    ("LET", 0xE9),
    ("LOCAL", 0xEA),
    ("MODE", 0xEB),
    ("MOVE", 0xEC),
    ("NEXT", 0xED),
    ("ON", 0xEE),
    ("VDU", 0xEF),
    ("PLOT", 0xF0),
    ("PRINT", 0xF1),
    ("PROC", 0xF2),
    ("READ", 0xF3),
    ("REM", 0xF4),
    ("REPEAT", 0xF5),
    ("REPORT", 0xF6),
    ("RESTORE", 0xF7),
    ("RETURN", 0xF8),
    ("RUN", 0xF9),
    ("STOP", 0xFA),
    ("COLOUR", 0xFB),
    ("TRACE", 0xFC),
    ("UNTIL", 0xFD),
    ("WIDTH", 0xFE),
    ("OSCLI", 0xFF),
];

// Extended functions (0xC6 prefix) - BBC BASIC 4 extensions
const EXTENDED_FUNCTIONS: &[(&str, u8)] = &[
    ("SUM", 0x8E),
    ("BEAT", 0x8F),
];

// Extended commands (0xC7 prefix) - BBC BASIC 4 extensions
const EXTENDED_COMMANDS: &[(&str, u8)] = &[
    ("APPEND", 0x8E),
    ("AUTO", 0x8F),
    ("CRUNCH", 0x90),
    ("DELETE", 0x91),
    ("EDIT", 0x92),
    ("HELP", 0x93),
    ("LIST", 0x94),
    ("LOAD", 0x95),
    ("LVAR", 0x96),
    ("NEW", 0x97),
    ("OLD", 0x98),
    ("RENUMBER", 0x99),
    ("SAVE", 0x9A),
    ("TEXTLOAD", 0x9B),
    ("TEXTSAVE", 0x9C),
    ("TWIN", 0x9D),
    ("TWINO", 0x9E),
];

// Extended statements (0xC8 prefix) - BBC BASIC 4 extensions
const EXTENDED_STATEMENTS: &[(&str, u8)] = &[
    ("CASE", 0x8E),
    ("CIRCLE", 0x8F),
    ("FILL", 0x90),
    ("ORIGIN", 0x91),
    ("POINT", 0x92),
    ("RECTANGLE", 0x93),
    ("SWAP", 0x94),
    ("WHILE", 0x95),
    ("WAIT", 0x96),
    ("MOUSE", 0x97),
    ("QUIT", 0x98),
    ("SYS", 0x99),
    ("INSTALL", 0x9A),
    ("LIBRARY", 0x9B),
    ("TINT", 0x9C),
    ("ELLIPSE", 0x9D),
    ("BEATS", 0x9E),
    ("TEMPO", 0x9F),
    ("VOICES", 0xA0),
    ("VOICE", 0xA1),
    ("STEREO", 0xA2),
    ("OVERLAY", 0xA3),
];

/// Create keyword lookup tables for tokenization
pub fn create_keyword_maps() -> (HashMap<String, u8>, HashMap<String, (u8, u8)>) {
    let mut main_keywords = HashMap::new();
    let mut extended_keywords = HashMap::new();
    
    // Add main keywords
    for &(keyword, token) in MAIN_KEYWORDS {
        main_keywords.insert(keyword.to_string(), token);
    }
    
    // Add extended functions with 0xC6 prefix
    for &(keyword, token) in EXTENDED_FUNCTIONS {
        extended_keywords.insert(keyword.to_string(), (0xC6, token));
    }
    
    // Add extended commands with 0xC7 prefix
    for &(keyword, token) in EXTENDED_COMMANDS {
        extended_keywords.insert(keyword.to_string(), (0xC7, token));
    }
    
    // Add extended statements with 0xC8 prefix
    for &(keyword, token) in EXTENDED_STATEMENTS {
        extended_keywords.insert(keyword.to_string(), (0xC8, token));
    }
    
    (main_keywords, extended_keywords)
}

/// Create reverse lookup tables for detokenization
pub fn create_reverse_keyword_maps() -> (HashMap<u8, String>, HashMap<(u8, u8), String>) {
    let mut main_reverse = HashMap::new();
    let mut extended_reverse = HashMap::new();
    
    // Add main keywords
    for &(keyword, token) in MAIN_KEYWORDS {
        main_reverse.insert(token, keyword.to_string());
    }
    
    // Add extended functions with 0xC6 prefix
    for &(keyword, token) in EXTENDED_FUNCTIONS {
        extended_reverse.insert((0xC6, token), keyword.to_string());
    }
    
    // Add extended commands with 0xC7 prefix
    for &(keyword, token) in EXTENDED_COMMANDS {
        extended_reverse.insert((0xC7, token), keyword.to_string());
    }
    
    // Add extended statements with 0xC8 prefix
    for &(keyword, token) in EXTENDED_STATEMENTS {
        extended_reverse.insert((0xC8, token), keyword.to_string());
    }
    
    (main_reverse, extended_reverse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenized_line_creation() {
        let line = TokenizedLine::new(Some(10), vec![Token::Keyword(0x80)]);
        assert_eq!(line.line_number, Some(10));
        assert_eq!(line.tokens.len(), 1);
    }

    #[test]
    fn test_empty_tokenized_line() {
        let line = TokenizedLine::empty();
        assert_eq!(line.line_number, None);
        assert_eq!(line.tokens.len(), 0);
    }

    #[test]
    fn test_keyword_maps_creation() {
        let (main_keywords, extended_keywords) = create_keyword_maps();
        
        // Test some main keywords
        assert_eq!(main_keywords.get("PRINT"), Some(&0xF1));
        assert_eq!(main_keywords.get("FOR"), Some(&0xE3));
        assert_eq!(main_keywords.get("AND"), Some(&0x80));
        
        // Test extended keywords
        assert_eq!(extended_keywords.get("SUM"), Some(&(0xC6, 0x8E)));
        assert_eq!(extended_keywords.get("AUTO"), Some(&(0xC7, 0x8F)));
        assert_eq!(extended_keywords.get("CASE"), Some(&(0xC8, 0x8E)));
    }

    #[test]
    fn test_reverse_keyword_maps_creation() {
        let (main_reverse, extended_reverse) = create_reverse_keyword_maps();
        
        // Test main reverse lookup
        assert_eq!(main_reverse.get(&0xF1), Some(&"PRINT".to_string()));
        assert_eq!(main_reverse.get(&0xE3), Some(&"FOR".to_string()));
        
        // Test extended reverse lookup
        assert_eq!(extended_reverse.get(&(0xC6, 0x8E)), Some(&"SUM".to_string()));
        assert_eq!(extended_reverse.get(&(0xC7, 0x8F)), Some(&"AUTO".to_string()));
        assert_eq!(extended_reverse.get(&(0xC8, 0x8E)), Some(&"CASE".to_string()));
    }
}