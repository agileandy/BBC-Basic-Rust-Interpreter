//! Tokenizer for BBC BASIC source code
//!
//! Converts BBC BASIC source code into internal token representation compatible
//! with the original BBC Micro tokenized format.

use crate::error::{BBCBasicError, Result};
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
        Self {
            line_number,
            tokens,
        }
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
    let mut tokens = Vec::new();
    let mut line_number = None;
    let (keyword_map, extended_map) = create_keyword_maps();

    // Trim the input
    let line = source_line.trim();
    if line.is_empty() {
        return Ok(TokenizedLine::empty());
    }

    let mut chars = line.chars().peekable();

    // Check for line number at the start (only if followed by whitespace and a keyword/identifier)
    if let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            // Peek ahead to check if this looks like a line number
            let mut temp_chars = chars.clone();
            let mut num_str = String::new();

            // Manually collect digits to avoid consuming past them
            while let Some(&ch) = temp_chars.peek() {
                if ch.is_ascii_digit() {
                    num_str.push(ch);
                    temp_chars.next();
                } else {
                    break;
                }
            }

            // Line numbers must be followed by whitespace AND start of statement
            let has_whitespace = temp_chars
                .peek()
                .map(|c| c.is_whitespace())
                .unwrap_or(false);
            if has_whitespace {
                // Skip whitespace
                while temp_chars
                    .peek()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false)
                {
                    temp_chars.next();
                }
                // Check if what follows looks like a statement (keyword or identifier, not
                // operator). A `=expr` line ends a multi-line function.
                let next_is_statement = temp_chars
                    .peek()
                    .map(|c| c.is_alphabetic() || *c == '_' || *c == '=')
                    .unwrap_or(false);

                if next_is_statement {
                    // This is a line number - consume it from the actual iterator
                    let mut actual_num_str = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_digit() {
                            actual_num_str.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    if let Ok(num) = actual_num_str.parse::<u16>() {
                        line_number = Some(num);
                        // Skip whitespace after line number
                        while let Some(&ch) = chars.peek() {
                            if ch.is_whitespace() {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Tokenize the rest of the line
    while let Some(&ch) = chars.peek() {
        // Skip whitespace
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        // String literal
        if ch == '"' {
            chars.next(); // consume opening quote
            let mut string_content = String::new();

            while let Some(ch) = chars.next() {
                if ch == '"' {
                    // A doubled quote is one quote character in the string
                    if chars.peek() == Some(&'"') {
                        chars.next();
                    } else {
                        break; // found closing quote
                    }
                }
                string_content.push(ch);
            }

            tokens.push(Token::String(string_content));
            continue;
        }

        // Numbers (integer or real, including negative)
        if ch.is_ascii_digit()
            || (ch == '-'
                && chars
                    .clone()
                    .nth(1)
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false))
        {
            let mut num_str = String::new();
            let mut is_real = false;

            // Handle negative sign
            if ch == '-' {
                num_str.push(ch);
                chars.next();
            }

            // Collect digits before decimal point
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    num_str.push(ch);
                    chars.next();
                } else if ch == '.' {
                    // Check if the next character is a digit (to distinguish from range operator)
                    let mut temp_chars = chars.clone();
                    temp_chars.next(); // skip the '.'
                    if temp_chars
                        .peek()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                    {
                        is_real = true;
                        num_str.push(ch);
                        chars.next(); // consume the '.'
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // An exponent is E, an optional sign and one digit or more.
            // If not, the E starts the next token.
            let mut lookahead = chars.clone();
            let has_e = lookahead.next() == Some('E');
            let sign = lookahead.next_if(|c| *c == '+' || *c == '-');
            if has_e && lookahead.peek().is_some_and(|c| c.is_ascii_digit()) {
                is_real = true;
                num_str.push('E');
                chars.next(); // consume the 'E'
                if let Some(sign) = sign {
                    num_str.push(sign);
                    chars.next(); // consume the sign
                }
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() {
                        num_str.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
            }

            // Parse the number. A real number that is not finite, such as 1E400, is too big.
            if is_real {
                if let Ok(val) = num_str.parse::<f64>() {
                    if !val.is_finite() {
                        return Err(BBCBasicError::TooBig);
                    }
                    tokens.push(Token::Real(val));
                }
            } else if let Ok(val) = num_str.parse::<i32>() {
                tokens.push(Token::Integer(val));
            } else if let Ok(val) = num_str.parse::<f64>() {
                // An integer that does not fit in an i32 is a real
                if !val.is_finite() {
                    return Err(BBCBasicError::TooBig);
                }
                tokens.push(Token::Real(val));
            }
            continue;
        }

        // A hex number is & and one to 8 hex digits. It is a 32-bit integer.
        if ch == '&' {
            chars.next(); // consume the '&'
            let mut hex = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() || ('A'..='F').contains(&ch) {
                    hex.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            if hex.is_empty() {
                return Err(BBCBasicError::SyntaxError {
                    message: "Bad hex".to_string(),
                    line: line_number,
                });
            }
            if hex.len() > 8 {
                return Err(BBCBasicError::TooBig);
            }
            let value = u32::from_str_radix(&hex, 16).map_err(|_| BBCBasicError::TooBig)?;
            tokens.push(Token::Integer(value as i32));
            continue;
        }

        // Keywords and identifiers
        if ch.is_alphabetic() || ch == '_' {
            let mut word = String::new();

            // Collect alphanumeric characters, underscores, % (integer), $ (string)
            while let Some(&ch) = chars.peek() {
                if ch.is_alphanumeric() || ch == '_' || ch == '%' || ch == '$' {
                    word.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }

            // Convert to uppercase for keyword matching
            let upper_word = word.to_uppercase();

            // Check if it's a keyword
            if let Some(&token_byte) = keyword_map.get(&upper_word) {
                tokens.push(Token::Keyword(token_byte));
            } else if let Some(&(prefix, token_byte)) = extended_map.get(&upper_word) {
                tokens.push(Token::ExtendedKeyword(prefix, token_byte));
            } else if let Some((keyword, name)) = split_proc_or_fn(&word) {
                // PROCname or FNname: the name joins the keyword with no space
                tokens.push(Token::Keyword(keyword_map[keyword]));
                tokens.push(Token::Identifier(name.to_string()));
            } else {
                // It's an identifier (variable, procedure, or function name)
                tokens.push(Token::Identifier(word));
            }
            continue;
        }

        // Operators and separators
        match ch {
            '\'' => {
                // Apostrophe is shorthand for REM - rest of line is a comment
                chars.next(); // consume apostrophe
                tokens.push(Token::Keyword(0xF4)); // REM token
                                                   // Consume rest of line (don't tokenize comment text)
                while chars.next().is_some() {}
            }
            // # starts the channel of a file statement, such as PRINT#
            '+' | '*' | '/' | '^' | '<' | '>' | '=' | '#' => {
                chars.next();
                tokens.push(Token::Operator(ch));
            }
            '-' => {
                // Only treat as operator if not followed by digit (handled above)
                chars.next();
                tokens.push(Token::Operator(ch));
            }
            ',' | ';' | ':' | '(' | ')' => {
                chars.next();
                tokens.push(Token::Separator(ch));
            }
            _ => {
                // A REM or DATA statement uses the rest of the line, so the
                // tokenizer skips a character that it does not know there.
                // In other text, the character is an error.
                if !tokens
                    .iter()
                    .any(|t| matches!(t, Token::Keyword(0xF4 | 0xDC)))
                {
                    return Err(BBCBasicError::SyntaxError {
                        message: format!("Unknown character '{}'", ch),
                        line: line_number,
                    });
                }
                chars.next();
            }
        }
    }

    Ok(TokenizedLine::new(line_number, tokens))
}

/// Convert tokens back to BBC BASIC source
pub fn detokenize(tokenized_line: &TokenizedLine) -> Result<String> {
    let (main_reverse, extended_reverse) = create_reverse_keyword_maps();
    let mut result = String::new();

    // Add line number if present
    if let Some(line_num) = tokenized_line.line_number {
        result.push_str(&format!("{} ", line_num));
    }

    // Convert each token
    for (i, token) in tokenized_line.tokens.iter().enumerate() {
        // Add space before token (except first one or after certain tokens)
        if i > 0 {
            let needs_space = match (&tokenized_line.tokens[i - 1], token) {
                // No space after opening paren or before closing paren
                (_, Token::Separator(')')) => false,
                (Token::Separator('('), _) => false,
                // No space before/after certain operators
                (Token::Separator(_), _) => false,
                (_, Token::Separator(',')) => false,
                (_, Token::Separator(';')) => false,
                (_, Token::Separator(':')) => false,
                // The shift operators << and >> list as typed
                (Token::Operator('<'), Token::Operator('<')) => false,
                (Token::Operator('>'), Token::Operator('>')) => false,
                // Need space between most tokens
                _ => true,
            };

            if needs_space {
                result.push(' ');
            }
        }

        match token {
            Token::Keyword(byte) => {
                if let Some(keyword) = main_reverse.get(byte) {
                    result.push_str(keyword);
                }
            }
            Token::ExtendedKeyword(prefix, byte) => {
                if let Some(keyword) = extended_reverse.get(&(*prefix, *byte)) {
                    result.push_str(keyword);
                }
            }
            Token::LineNumber(num) => {
                result.push_str(&num.to_string());
            }
            Token::Integer(val) => {
                result.push_str(&val.to_string());
            }
            Token::Real(val) => {
                // A whole real lists with an exponent, so that it tokenizes as a real again
                let plain = val.to_string();
                if plain.contains('.') {
                    result.push_str(&plain);
                } else {
                    result.push_str(&format!("{:E}", val));
                }
            }
            Token::String(s) => {
                result.push('"');
                result.push_str(s);
                result.push('"');
            }
            Token::Identifier(name) => {
                result.push_str(name);
            }
            Token::Operator(op) => {
                result.push(*op);
            }
            Token::Separator(sep) => {
                result.push(*sep);
            }
            Token::EndOfLine => {
                // End of line marker
            }
        }
    }

    Ok(result)
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
const EXTENDED_FUNCTIONS: &[(&str, u8)] = &[("SUM", 0x8E), ("BEAT", 0x8F)];

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
    ("ENDWHILE", 0xA4),
];

/// Split a word such as `PROCgreet` or `FNadd` into its keyword and its name.
/// The keyword must be in upper case, as in BBC BASIC.
fn split_proc_or_fn(word: &str) -> Option<(&'static str, &str)> {
    ["PROC", "FN"].into_iter().find_map(|keyword| {
        word.strip_prefix(keyword)
            .filter(|name| !name.is_empty())
            .map(|name| (keyword, name))
    })
}

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
        assert_eq!(
            extended_reverse.get(&(0xC6, 0x8E)),
            Some(&"SUM".to_string())
        );
        assert_eq!(
            extended_reverse.get(&(0xC7, 0x8F)),
            Some(&"AUTO".to_string())
        );
        assert_eq!(
            extended_reverse.get(&(0xC8, 0x8E)),
            Some(&"CASE".to_string())
        );
    }

    // TDD Tests for tokenization

    #[test]
    fn test_tokenize_integer_literal() {
        // RED: Test tokenizing a simple integer
        let result = tokenize("42").unwrap();
        assert_eq!(result.line_number, None);
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0], Token::Integer(42));
    }

    #[test]
    fn test_tokenize_negative_integer() {
        // RED: Test tokenizing a negative integer
        let result = tokenize("-123").unwrap();
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0], Token::Integer(-123));
    }

    #[test]
    fn test_tokenize_real_number() {
        // RED: Test tokenizing a real number
        let result = tokenize("3.14159").unwrap();
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0], Token::Real(3.14159));
    }

    #[test]
    fn test_tokenize_exponent_literal_is_real() {
        let result = tokenize("1E3").unwrap();
        assert_eq!(result.tokens, vec![Token::Real(1000.0)]);
    }

    #[test]
    fn test_tokenize_exponent_literal_with_sign() {
        let result = tokenize("1.5E-2").unwrap();
        assert_eq!(result.tokens, vec![Token::Real(0.015)]);
        let result = tokenize("2E+2").unwrap();
        assert_eq!(result.tokens, vec![Token::Real(200.0)]);
    }

    #[test]
    fn test_tokenize_e_with_no_digits_is_identifier() {
        let result = tokenize("1E").unwrap();
        assert_eq!(
            result.tokens,
            vec![Token::Integer(1), Token::Identifier("E".to_string())]
        );
        let result = tokenize("1E+").unwrap();
        assert_eq!(
            result.tokens,
            vec![
                Token::Integer(1),
                Token::Identifier("E".to_string()),
                Token::Operator('+')
            ]
        );
    }

    #[test]
    fn test_tokenize_lines_with_e_after_number_are_unchanged() {
        let result = tokenize("IF X = 1 ELSE PRINT 2").unwrap();
        assert_eq!(result.tokens[3], Token::Integer(1));
        assert_eq!(result.tokens[4], Token::Keyword(0x8B)); // ELSE

        let result = tokenize("FOR I = 1 TO 2").unwrap();
        assert_eq!(result.tokens[3], Token::Integer(1));
        assert_eq!(result.tokens[4], Token::Keyword(0xB8)); // TO

        let result = tokenize("A = 2 * E").unwrap();
        assert_eq!(result.tokens[2], Token::Integer(2));
        assert_eq!(result.tokens[4], Token::Identifier("E".to_string()));

        let result = tokenize("PRINT 1;E").unwrap();
        assert_eq!(
            result.tokens[1..],
            [
                Token::Integer(1),
                Token::Separator(';'),
                Token::Identifier("E".to_string())
            ]
        );
        // &1E is a hex number, not 1 and E
        let result = tokenize("PRINT &1E").unwrap();
        assert_eq!(result.tokens[1..], [Token::Integer(0x1E)]);
    }

    #[test]
    fn test_tokenize_hex_literals() {
        let cases = [
            ("&1F", 31),
            ("&FF", 255),
            ("&7FFFFFFF", i32::MAX),
            ("&FFFFFFFF", -1),
            ("&80000000", i32::MIN),
            ("&1E3", 483),
        ];
        for (source, value) in cases {
            let result = tokenize(source).unwrap();
            assert_eq!(result.tokens, vec![Token::Integer(value)], "{source}");
        }
    }

    #[test]
    fn test_tokenize_hex_literal_in_expression() {
        let result = tokenize("A = &10 + 1").unwrap();
        assert_eq!(
            result.tokens,
            vec![
                Token::Identifier("A".to_string()),
                Token::Operator('='),
                Token::Integer(16),
                Token::Operator('+'),
                Token::Integer(1),
            ]
        );
    }

    #[test]
    fn test_tokenize_ampersand_with_no_hex_digit_is_syntax_error() {
        for source in ["&", "PRINT &", "&G"] {
            assert!(
                matches!(tokenize(source), Err(BBCBasicError::SyntaxError { .. })),
                "{source}"
            );
        }
    }

    #[test]
    fn test_tokenize_hex_literal_with_nine_digits_is_too_big() {
        assert_eq!(
            tokenize("&123456789").map(|_| ()),
            Err(BBCBasicError::TooBig)
        );
    }

    #[test]
    fn test_tokenize_integer_too_large_for_i32_is_real() {
        let result = tokenize("3000000000").unwrap();
        assert_eq!(result.tokens, vec![Token::Real(3_000_000_000.0)]);
    }

    #[test]
    fn test_tokenize_print_keyword() {
        // RED: Test tokenizing PRINT keyword
        let result = tokenize("PRINT").unwrap();
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0], Token::Keyword(0xF1));
    }

    #[test]
    fn test_tokenize_print_with_integer() {
        // RED: Test tokenizing "PRINT 42"
        let result = tokenize("PRINT 42").unwrap();
        assert_eq!(result.tokens.len(), 2);
        assert_eq!(result.tokens[0], Token::Keyword(0xF1));
        assert_eq!(result.tokens[1], Token::Integer(42));
    }

    #[test]
    fn test_tokenize_string_literal() {
        // RED: Test tokenizing string literal
        let result = tokenize(r#""Hello, World!""#).unwrap();
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0], Token::String("Hello, World!".to_string()));
    }

    #[test]
    fn test_tokenize_print_with_string() {
        // RED: Test tokenizing PRINT "hello"
        let result = tokenize(r#"PRINT "hello""#).unwrap();
        assert_eq!(result.tokens.len(), 2);
        assert_eq!(result.tokens[0], Token::Keyword(0xF1));
        assert_eq!(result.tokens[1], Token::String("hello".to_string()));
    }

    #[test]
    fn test_tokenize_identifier() {
        // RED: Test tokenizing variable identifier
        let result = tokenize("A%").unwrap();
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0], Token::Identifier("A%".to_string()));
    }

    #[test]
    fn test_tokenize_assignment() {
        // RED: Test tokenizing "A% = 42"
        let result = tokenize("A% = 42").unwrap();
        assert_eq!(result.tokens.len(), 3);
        assert_eq!(result.tokens[0], Token::Identifier("A%".to_string()));
        assert_eq!(result.tokens[1], Token::Operator('='));
        assert_eq!(result.tokens[2], Token::Integer(42));
    }

    #[test]
    fn test_tokenize_line_with_line_number() {
        // RED: Test tokenizing line with line number
        let result = tokenize("10 PRINT 42").unwrap();
        assert_eq!(result.line_number, Some(10));
        assert_eq!(result.tokens.len(), 2);
        assert_eq!(result.tokens[0], Token::Keyword(0xF1));
        assert_eq!(result.tokens[1], Token::Integer(42));
    }

    #[test]
    fn test_tokenize_expression_with_operators() {
        // RED: Test tokenizing "2 + 3 * 4"
        let result = tokenize("2 + 3 * 4").unwrap();
        assert_eq!(result.tokens.len(), 5);
        assert_eq!(result.tokens[0], Token::Integer(2));
        assert_eq!(result.tokens[1], Token::Operator('+'));
        assert_eq!(result.tokens[2], Token::Integer(3));
        assert_eq!(result.tokens[3], Token::Operator('*'));
        assert_eq!(result.tokens[4], Token::Integer(4));
    }

    // Detokenization tests

    #[test]
    fn test_detokenize_simple_integer() {
        let line = TokenizedLine::new(None, vec![Token::Integer(42)]);
        let result = detokenize(&line).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_detokenize_real_tokenizes_to_same_real() {
        for source in ["1E3", "1.5E-2", "1E10", "1E300", "3.14159"] {
            let tokens = tokenize(source).unwrap();
            let listed = detokenize(&tokens).unwrap();
            assert_eq!(tokenize(&listed).unwrap().tokens, tokens.tokens, "{listed}");
        }
    }

    #[test]
    fn test_detokenize_print_statement() {
        let line = TokenizedLine::new(Some(10), vec![Token::Keyword(0xF1), Token::Integer(42)]);
        let result = detokenize(&line).unwrap();
        assert_eq!(result, "10 PRINT 42");
    }

    #[test]
    fn test_detokenize_print_with_string() {
        let line = TokenizedLine::new(
            None,
            vec![Token::Keyword(0xF1), Token::String("Hello".to_string())],
        );
        let result = detokenize(&line).unwrap();
        assert_eq!(result, r#"PRINT "Hello""#);
    }

    #[test]
    fn test_detokenize_assignment() {
        let line = TokenizedLine::new(
            None,
            vec![
                Token::Identifier("A%".to_string()),
                Token::Operator('='),
                Token::Integer(42),
            ],
        );
        let result = detokenize(&line).unwrap();
        assert_eq!(result, "A% = 42");
    }

    #[test]
    fn test_apostrophe_comment() {
        // RED: Test that apostrophe (') is tokenized as REM
        let line = tokenize("10 PRINT 42 ' This is a comment").unwrap();

        // Should tokenize as: LineNumber(10), Keyword(PRINT), Integer(42), Keyword(REM)
        assert_eq!(line.line_number, Some(10));
        assert_eq!(line.tokens.len(), 3); // PRINT, 42, REM

        // Check tokens
        assert!(matches!(line.tokens[0], Token::Keyword(0xF1))); // PRINT
        assert!(matches!(line.tokens[1], Token::Integer(42)));
        assert!(matches!(line.tokens[2], Token::Keyword(0xF4))); // REM
    }

    /// The characters that the tokenizer does not know (issue 62).
    const UNKNOWN_CHARACTERS: [char; 11] = ['~', '?', '!', '@', '|', '\\', '[', ']', '{', '}', '`'];

    #[test]
    fn test_unknown_character_in_statement_gives_syntax_error() {
        for ch in UNKNOWN_CHARACTERS {
            let source = format!("10 PRINT {}5", ch);
            match tokenize(&source) {
                Err(BBCBasicError::SyntaxError { message, line }) => {
                    assert_eq!(message, format!("Unknown character '{}'", ch));
                    assert_eq!(line, Some(10));
                }
                other => panic!("{:?} gave {:?}", source, other),
            }
        }
    }

    #[test]
    fn test_unknown_character_in_string_is_kept() {
        for ch in UNKNOWN_CHARACTERS {
            let source = format!("10 PRINT \"A{}B\"", ch);
            let line = tokenize(&source).expect("string tokenizes");
            assert_eq!(line.tokens[1], Token::String(format!("A{}B", ch)));
        }
    }

    #[test]
    fn test_unknown_character_after_rem_gives_no_error() {
        for ch in UNKNOWN_CHARACTERS {
            let source = format!("10 X = 1 : REM a {} b", ch);
            assert!(tokenize(&source).is_ok(), "{:?} gave an error", source);
        }
    }

    #[test]
    fn test_unknown_character_in_data_line_gives_no_error() {
        for ch in UNKNOWN_CHARACTERS {
            let source = format!("10 DATA 1, {}2", ch);
            assert!(tokenize(&source).is_ok(), "{:?} gave an error", source);
        }
    }

    #[test]
    fn test_hash_is_an_operator_for_file_statements() {
        let line = tokenize("10 PRINT#F%, 42").unwrap();
        assert_eq!(line.tokens[1], Token::Operator('#'));
    }

    #[test]
    fn test_tab_is_white_space() {
        let line = tokenize("10 PRINT\t5").unwrap();
        assert_eq!(line.tokens, vec![Token::Keyword(0xF1), Token::Integer(5)]);
    }
}
