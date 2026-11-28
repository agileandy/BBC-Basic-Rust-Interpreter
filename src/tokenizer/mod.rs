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
    /// Hex integer literal, such as `&1F`. It has the same value as an
    /// integer, but LIST shows it in hex, and RENUMBER does not change it.
    HexInteger(i32),
    /// Real number literal
    Real(f64),
    /// String literal
    String(String),
    /// The text after DATA to the end of the line, as typed
    DataText(String),
    /// The text after the REM keyword to the end of the line, as typed. An
    /// apostrophe comment uses REM and a DataText, so the 2 forms list as typed.
    RemText(String),
    /// Variable or procedure name
    Identifier(String),
    /// A structure member, such as `point.x` or `shape.pos.x`: the structure
    /// name and the member names of the path (issues #191 and #192)
    Member(String, Vec<String>),
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

/// True if the last token ends a value: a number, a string, a name, a
/// closing bracket, or a keyword that is a value with no argument, such as
/// PI. A `-` after a value is the subtract operator.
fn ends_with_value(tokens: &[Token]) -> bool {
    matches!(
        tokens.last(),
        Some(
            Token::Integer(_)
                | Token::HexInteger(_)
                | Token::Real(_)
                | Token::String(_)
                | Token::Identifier(_)
                | Token::Member(..)
                | Token::Separator(')')
                // PAGE TIME LOMEM HIMEM, COUNT ERL ERR FALSE GET PI POS
                // TRUE VPOS, and the PAGE..HIMEM forms at the left of `=`
                | Token::Keyword(
                    0x90..=0x93
                        | 0x9C
                        | 0x9E
                        | 0x9F
                        | 0xA3
                        | 0xA5
                        | 0xAF
                        | 0xB1
                        | 0xB9
                        | 0xBC
                        | 0xD0..=0xD3
                )
        )
    )
}

/// True if an apostrophe after these tokens is a new line item, not a
/// comment. The rule: the statement starts after the last colon, THEN or
/// ELSE, it contains PRINT or INPUT, and the line has no REM.
fn apostrophe_is_new_line(tokens: &[Token]) -> bool {
    if tokens.iter().any(|t| matches!(t, Token::Keyword(0xF4))) {
        return false;
    }
    let start = tokens
        .iter()
        .rposition(|t| matches!(t, Token::Separator(':') | Token::Keyword(0x8B | 0x8C)))
        .map_or(0, |i| i + 1);
    tokens[start..]
        .iter()
        .any(|t| matches!(t, Token::Keyword(0xE8 | 0xF1))) // INPUT, PRINT
}

/// Tokenize a BBC BASIC source line
pub fn tokenize(source_line: &str) -> Result<TokenizedLine> {
    tokenize_text(source_line, true)
}

/// Tokenize the text of one expression, such as the argument of EVAL.
/// Leading digits are a number, not a line number (issue 147).
pub fn tokenize_expression(text: &str) -> Result<Vec<Token>> {
    tokenize_text(text, false).map(|line| line.tokens)
}

/// Tokenize a text. If `line_numbers` is false, leading digits are not a
/// line number.
fn tokenize_text(source_line: &str, line_numbers: bool) -> Result<TokenizedLine> {
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
    if let Some(&ch) = chars.peek().filter(|_| line_numbers) {
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

            // Leading digits are always a line number. A number that does not
            // fit is an error, so that the line does not run at once.
            if num_str.parse::<u16>().is_err() {
                return Err(BBCBasicError::SyntaxError {
                    message: "Line number out of range".to_string(),
                    line: None,
                });
            }

            // A line of only digits is a line number with no statement. The
            // line is trimmed, so trailing spaces are already gone.
            if temp_chars.peek().is_none() {
                if let Ok(num) = num_str.parse::<u16>() {
                    return Ok(TokenizedLine::new(Some(num), Vec::new()));
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
                // operator). A `=expr` line ends a multi-line function. A
                // `?addr=v` or `!addr=v` line writes to memory. A `'` line
                // is a comment (issue 159). A `@%=v` line sets the print
                // format (issue 166).
                let next_is_statement = temp_chars
                    .peek()
                    .map(|c| c.is_alphabetic() || matches!(c, '_' | '=' | '?' | '!' | '\'' | '@'))
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

        // Numbers (integer or real, including negative). A real number
        // can start with a decimal point, such as .5. A `-` starts a
        // negative number only when no value is before it, so 5-3 subtracts.
        if ch.is_ascii_digit()
            || ((ch == '.' || (ch == '-' && !ends_with_value(&tokens)))
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
            tokens.push(Token::HexInteger(value as i32));
            continue;
        }

        // @% is the print format variable (issue 166). No other name starts with @.
        if ch == '@' {
            let mut lookahead = chars.clone();
            lookahead.next();
            if lookahead.peek() == Some(&'%') {
                chars.next(); // consume the '@'
                chars.next(); // consume the '%'
                tokens.push(Token::Identifier("@%".to_string()));
                continue;
            }
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

            // A short form is letters and a full stop, such as P. for PRINT
            if chars.peek() == Some(&'.') {
                if let Some(full) = short_form_keyword(&word) {
                    chars.next(); // consume the '.'
                    word = full.to_string();
                }
            }

            // A name, a full stop and a letter start a structure member,
            // such as point.x (issue #191). A short form wins (above). Each
            // more full stop and letter add a member of a nested structure,
            // such as shape.pos.x (issue #192).
            if chars.peek() == Some(&'.')
                && is_structure_name(&word, &keyword_map, &extended_map)
                && chars.clone().nth(1).is_some_and(starts_member_name)
            {
                let mut members = Vec::new();
                while chars.peek() == Some(&'.')
                    && chars.clone().nth(1).is_some_and(starts_member_name)
                {
                    chars.next(); // consume the '.'
                    members.push(take_name(&mut chars));
                }
                tokens.push(Token::Member(word, members));
                continue;
            }

            // A keyword matches only in upper case, as in BBC BASIC.
            // A word in another letter case is a name.
            if let Some(&token_byte) = keyword_map.get(&word) {
                // DATA after LOCAL or RESTORE is a part of LOCAL DATA or
                // RESTORE DATA, and the line continues (issue #186)
                let data_part = matches!(tokens.last(), Some(Token::Keyword(0xEA | 0xF7)));
                tokens.push(Token::Keyword(token_byte));
                // DATA uses the rest of the line. Keep that text as typed.
                if token_byte == 0xDC && !data_part {
                    let text: String = chars.by_ref().collect();
                    if !text.is_empty() {
                        tokens.push(Token::DataText(text));
                    }
                }
                // REM uses the rest of the line. Keep that text as typed (issue 179).
                if token_byte == 0xF4 {
                    let text: String = chars.by_ref().collect();
                    if !text.is_empty() {
                        tokens.push(Token::RemText(text));
                    }
                }
            } else if let Some(&(prefix, token_byte)) = extended_map.get(&word) {
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
            '\'' if apostrophe_is_new_line(&tokens) => {
                // In PRINT and INPUT, an apostrophe prints a new line (issue 159)
                chars.next();
                tokens.push(Token::Separator(ch));
            }
            // An apostrophe after REM is part of the REM text (issue 178)
            '\'' if tokens.contains(&Token::Keyword(0xF4)) => {
                chars.next();
                tokens.push(Token::Separator(ch));
            }
            '\'' => {
                // An apostrophe comment is REM and the rest of the line as
                // typed. A REM with a DataText after it lists as `'` (issue 178).
                chars.next();
                tokens.push(Token::Keyword(0xF4));
                tokens.push(Token::DataText(chars.by_ref().collect()));
            }
            // # starts the channel of a file statement, such as PRINT#
            // ~ selects hex output, as in PRINT ~X and STR$~(X)
            // ? and ! read and write memory, as in ?A=5 and B!4 (issue 76)
            '+' | '*' | '/' | '^' | '<' | '>' | '=' | '#' | '~' | '?' | '!' => {
                chars.next();
                tokens.push(Token::Operator(ch));
            }
            '-' => {
                // Only treat as operator if not followed by digit (handled above)
                chars.next();
                tokens.push(Token::Operator(ch));
            }
            // A full stop after a closing bracket is the dot operator of a
            // matrix product, A() = B() . C() (issue #189). A full stop that
            // starts a number, a short form or a member is a token before
            // this point.
            '.' if tokens.last() == Some(&Token::Separator(')')) => {
                chars.next();
                tokens.push(Token::Operator(ch));
            }
            // { and } enclose the members of a structure (issue #191)
            ',' | ';' | ':' | '(' | ')' | '{' | '}' => {
                chars.next();
                tokens.push(Token::Separator(ch));
            }
            _ => {
                // A REM or DATA statement uses the rest of the line, so the
                // tokenizer skips a character that it does not know there.
                // In other text, the character is an error. The DATA of
                // LOCAL DATA does not use the line, and has no DataText.
                if !tokens
                    .iter()
                    .any(|t| matches!(t, Token::Keyword(0xF4) | Token::DataText(_)))
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
                // A function, an array or the name after FN or PROC joins
                // its bracket: VAL("1"), C(3), FNa(1)
                (Token::Keyword(byte), Token::Separator('(')) if is_function_keyword(*byte) => {
                    false
                }
                (Token::Identifier(_), Token::Separator('(')) => false,
                // An array member joins its bracket: s.a(1) (issue #192)
                (Token::Member(..), Token::Separator('(')) => false,
                // A structure lists as typed: DIM point{x,y%}
                (Token::Identifier(_), Token::Separator('{')) => false,
                (_, Token::Separator('}')) => false,
                (Token::Keyword(FN | PROC), Token::Identifier(_)) => false,
                // A keyword after a closing bracket: IF (A) THEN
                (Token::Separator(')'), Token::Keyword(_)) => true,
                // An operator after a closing bracket, as after a name:
                // LEN("ABC") - 1, (A) = 1. The memory forms list as typed: A%(1)?2
                (Token::Separator(')'), Token::Operator(op)) if !matches!(op, '?' | '!') => true,
                // The OF of a CASE after a closing bracket: CASE LEN(A$) OF
                (Token::Separator(')'), Token::Identifier(name)) if name == "OF" => true,
                // No space before/after certain operators
                (Token::Separator(_), _) => false,
                // The new line item lists as typed: PRINT ''"X", PRINT "A"'"B"
                (Token::Keyword(0xE8 | 0xF1), Token::Separator('\'')) => true,
                (_, Token::Separator('\'')) => false,
                (_, Token::Separator(',')) => false,
                (_, Token::Separator(';')) => false,
                (_, Token::Separator(':')) => false,
                // The shift operators << and >> list as typed
                (Token::Operator('<'), Token::Operator('<')) => false,
                (Token::Operator('>'), Token::Operator('>')) => false,
                // The comparison operators <=, >= and <> list as typed
                (Token::Operator('<'), Token::Operator('=')) => false,
                (Token::Operator('>'), Token::Operator('=')) => false,
                (Token::Operator('<'), Token::Operator('>')) => false,
                // The DATA text holds its own spaces
                (_, Token::DataText(_) | Token::RemText(_)) => false,
                // The hex forms ~X and STR$~(X) list as typed
                (Token::Operator('~'), _) => false,
                (Token::Keyword(0xC3), Token::Operator('~')) => false,
                // The memory forms ?A, !A, A?1 and A!1 list as typed
                (Token::Operator('?' | '!'), _) => false,
                (Token::Identifier(_) | Token::Member(..), Token::Operator('?' | '!')) => false,
                // A unary minus joins its value: ABS(-X), X = -Y, 2 * -Y
                (Token::Operator('-'), _)
                    if i < 2
                        || matches!(
                            &tokenized_line.tokens[i - 2],
                            Token::Operator(_) | Token::Separator('(' | ',' | ';' | ':')
                        ) =>
                {
                    false
                }
                // Need space between most tokens
                _ => true,
            };

            if needs_space {
                result.push(' ');
            }
        }

        match token {
            // An apostrophe comment lists as typed (issue 178)
            Token::Keyword(0xF4)
                if matches!(tokenized_line.tokens.get(i + 1), Some(Token::DataText(_))) =>
            {
                result.push('\'');
            }
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
            Token::HexInteger(val) => {
                result.push_str(&format!("&{:X}", *val as u32));
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
            Token::Member(structure, members) => {
                result.push_str(structure);
                for member in members {
                    result.push('.');
                    result.push_str(member);
                }
            }
            Token::DataText(text) | Token::RemText(text) => {
                result.push_str(text);
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

const FN: u8 = 0xA4;
const PROC: u8 = 0xF2;

/// True if the keyword is a function that takes its argument in brackets.
/// These are SPC, TAB and the functions in the range 0x8E to 0xC5. NOT and
/// TO are in that range, but they are an operator and a part of FOR.
fn is_function_keyword(byte: u8) -> bool {
    matches!(byte, 0x89 | 0x8A | 0x8E..=0xC5) && !matches!(byte, 0xAC | 0xB8)
}

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
    // INSTALL (0x9A) is not a keyword here. The parser finds it by its place
    // on the line, so that a variable named INSTALL still works (issue #185).
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
    ("ENDIF", 0xA5),
    // OF, WHEN, OTHERWISE and ENDCASE are not keywords here. The parser
    // finds them by their place on the line, so that a variable with one of
    // these names still works (issue #107).
];

/// The minimum short form of each keyword and the keyword. The source is the
/// BBC Microcomputer System User Guide, chapter 47 "Minimum abbreviations",
/// pages 483 to 484. A keyword that the guide shows with no full stop, such
/// as ABS or END, has no short form. The extension words have no short form.
const SHORT_FORMS: &[(&str, &str)] = &[
    ("AD", "ADVAL"),
    ("A", "AND"),
    ("AU", "AUTO"),
    ("B", "BGET"),
    ("BP", "BPUT"),
    ("CA", "CALL"),
    ("CH", "CHAIN"),
    ("CHR", "CHR$"),
    ("CL", "CLEAR"),
    ("CLO", "CLOSE"),
    ("C", "COLOUR"),
    ("COU", "COUNT"),
    ("D", "DATA"),
    ("DEL", "DELETE"),
    ("DR", "DRAW"),
    ("EL", "ELSE"),
    ("E", "ENDPROC"),
    ("ENV", "ENVELOPE"),
    ("ERR", "ERROR"),
    ("EV", "EVAL"),
    ("FA", "FALSE"),
    ("F", "FOR"),
    ("GC", "GCOL"),
    ("GE", "GET$"),
    ("GOS", "GOSUB"),
    ("G", "GOTO"),
    ("H", "HIMEM"),
    ("INK", "INKEY$"),
    ("I", "INPUT"),
    ("INS", "INSTR"),
    ("LE", "LEFT$"),
    ("LIN", "LINE"),
    ("L", "LIST"),
    ("LO", "LOAD"),
    ("LOC", "LOCAL"),
    ("LOM", "LOMEM"),
    ("M", "MID$"),
    ("MO", "MODE"),
    ("MOV", "MOVE"),
    ("N", "NEXT"),
    ("O", "OLD"),
    ("OP", "OPENIN"),
    ("OPENO", "OPENOUT"),
    ("OS", "OSCLI"),
    ("PA", "PAGE"),
    ("PL", "PLOT"),
    ("PO", "POINT"),
    ("P", "PRINT"),
    ("PRO", "PROC"),
    ("PT", "PTR"),
    ("REA", "READ"),
    ("REN", "RENUMBER"),
    ("REP", "REPEAT"),
    ("REPO", "REPORT"),
    ("RES", "RESTORE"),
    ("R", "RETURN"),
    ("RI", "RIGHT$"),
    ("SA", "SAVE"),
    ("SO", "SOUND"),
    ("S", "STEP"),
    ("STO", "STOP"),
    ("STR", "STR$"),
    ("STRI", "STRING$"),
    ("T", "TAN"),
    ("TH", "THEN"),
    ("TI", "TIME"),
    ("TR", "TRACE"),
    ("U", "UNTIL"),
    ("V", "VDU"),
    ("VP", "VPOS"),
    ("W", "WIDTH"),
];

/// Give the keyword of a short form, from the upper-case letters before the
/// full stop. The letters start with the minimum short form, and they are the
/// start of the keyword. A full keyword is not a short form, so `PRINT.5` is
/// PRINT .5, as in BBC BASIC. If 2 keywords match, the shorter minimum wins,
/// as in the keyword order of the BBC BASIC ROM: END. is ENDPROC (issue 167).
fn short_form_keyword(letters: &str) -> Option<&'static str> {
    if letters.is_empty() || !letters.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    SHORT_FORMS
        .iter()
        .filter(|(minimum, keyword)| {
            letters.starts_with(minimum)
                && keyword.starts_with(letters)
                && keyword.len() > letters.len()
        })
        .min_by_key(|(minimum, _)| minimum.len())
        .map(|&(_, keyword)| keyword)
}

/// True if the name and a full stop are a keyword short form, such as `P`
/// in `P.`. A structure with this name cannot use a member (issue #191).
pub fn is_short_form(name: &str) -> bool {
    short_form_keyword(name).is_some()
}

/// True if the word can be the name of a structure before a full stop: it
/// is not a keyword, not PROCname or FNname, and has no `%` or `$` (issue
/// #191). The caller checks the short forms first.
fn is_structure_name(
    word: &str,
    keyword_map: &HashMap<String, u8>,
    extended_map: &HashMap<String, (u8, u8)>,
) -> bool {
    !word.ends_with(['%', '$'])
        && !keyword_map.contains_key(word)
        && !extended_map.contains_key(word)
        && split_proc_or_fn(word).is_none()
}

/// True if the character can start a member name: a letter or `_`, as at
/// the start of a variable name
fn starts_member_name(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

/// Take a name from the characters: letters, digits, `_`, `%` and `$`
fn take_name(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut name = String::new();
    while let Some(ch) = chars.next_if(|c| c.is_alphanumeric() || matches!(c, '_' | '%' | '$')) {
        name.push(ch);
    }
    name
}

/// If a typed line starts with a short form, give the line with the full
/// keyword, so that a command at the prompt, such as `L.`, is the full word.
/// Give other lines unchanged.
pub fn expand_short_form(line: &str) -> String {
    let end = line
        .find(|c: char| !c.is_ascii_uppercase())
        .unwrap_or(line.len());
    match short_form_keyword(&line[..end]) {
        Some(keyword) if line[end..].starts_with('.') => {
            format!("{}{}", keyword, &line[end + 1..])
        }
        _ => line.to_string(),
    }
}

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
        // A bare number is a line number (issue #71), so put it in a statement
        let result = tokenize("PRINT 42").unwrap();
        assert_eq!(result.line_number, None);
        assert_eq!(result.tokens.len(), 2);
        assert_eq!(result.tokens[1], Token::Integer(42));
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
    fn test_tokenize_endif_line() {
        let result = tokenize("40 ENDIF").unwrap();
        assert_eq!(result.line_number, Some(40));
        assert_eq!(result.tokens, vec![Token::ExtendedKeyword(0xC8, 0xA5)]);
        assert_eq!(detokenize(&result).unwrap(), "40 ENDIF");
    }

    #[test]
    fn test_tokenize_case_lines_and_list_them_again() {
        for source in [
            "20 CASE X OF",
            "20 CASE LEN(\"ab\") OF",
            "20 CASE A(1) OF",
            "30 WHEN 1,2:PRINT 1",
            "40 OTHERWISE PRINT 2",
            "50 ENDCASE",
        ] {
            let result = tokenize(source).unwrap();
            assert_eq!(detokenize(&result).unwrap(), source);
        }
        let result = tokenize("20 CASE X OF").unwrap();
        assert_eq!(result.tokens[0], Token::ExtendedKeyword(0xC8, 0x8E));
    }

    #[test]
    fn test_tokenize_of_when_otherwise_and_endcase_are_identifiers() {
        // The parser finds these words by their place on the line, so that
        // a variable with one of these names still works (issue #107)
        for word in [
            "OF",
            "WHEN",
            "OTHERWISE",
            "ENDCASE",
            "of",
            "when",
            "otherwise",
            "endcase",
            "When",
        ] {
            let result = tokenize(&format!("{}=5", word)).unwrap();
            assert_eq!(result.tokens[0], Token::Identifier(word.to_string()));
        }
    }

    #[test]
    fn test_tokenize_lower_case_case_is_identifier() {
        let result = tokenize("case=5").unwrap();
        assert_eq!(result.tokens[0], Token::Identifier("case".to_string()));
    }

    #[test]
    fn test_tokenize_endif_not_in_upper_case_is_identifier() {
        for word in ["endif", "EndIf", "ENDif"] {
            let result = tokenize(&format!("{}=5", word)).unwrap();
            assert_eq!(result.tokens[0], Token::Identifier(word.to_string()));
        }
    }

    #[test]
    fn test_tokenize_keyword_in_upper_case_is_keyword() {
        let result = tokenize("PRINT").unwrap();
        assert_eq!(result.tokens, vec![Token::Keyword(0xF1)]);
    }

    #[test]
    fn test_tokenize_keyword_not_in_upper_case_is_identifier() {
        let words = [
            "print", "Print", "count", "time", "to", "step", "pi", "val", "len", "endwhile",
            "endif",
        ];
        for word in words {
            let result = tokenize(&format!("{}=5", word)).unwrap();
            assert_eq!(
                result.tokens[0],
                Token::Identifier(word.to_string()),
                "word {}",
                word
            );
        }
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
        assert_eq!(result.tokens[1..], [Token::HexInteger(0x1E)]);
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
            assert_eq!(result.tokens, vec![Token::HexInteger(value)], "{source}");
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
                Token::HexInteger(16),
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
        let result = tokenize("PRINT 3000000000").unwrap();
        assert_eq!(
            result.tokens,
            vec![Token::Keyword(0xF1), Token::Real(3_000_000_000.0)]
        );
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
    fn test_tokenize_line_number_alone_has_no_tokens() {
        let result = tokenize("10").unwrap();
        assert_eq!(result.line_number, Some(10));
        assert!(result.tokens.is_empty());
    }

    #[test]
    fn test_tokenize_line_number_alone_with_spaces_has_no_tokens() {
        let result = tokenize(" 10 ").unwrap();
        assert_eq!(result.line_number, Some(10));
        assert!(result.tokens.is_empty());
    }

    #[test]
    fn test_tokenize_line_number_then_statement_is_unchanged() {
        let result = tokenize("10 PRINT 1").unwrap();
        assert_eq!(result.line_number, Some(10));
        assert_eq!(result.tokens, vec![Token::Keyword(0xF1), Token::Integer(1)]);
    }

    #[test]
    fn test_tokenize_number_alone_out_of_line_range_is_not_a_line_number() {
        assert_eq!(
            tokenize("99999").map(|_| ()),
            Err(BBCBasicError::SyntaxError {
                message: "Line number out of range".to_string(),
                line: None,
            })
        );
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
        // An apostrophe outside PRINT and INPUT is tokenized as REM. In
        // PRINT it is a new line item (issue 159).
        let line = tokenize("10 GOTO 42 ' This is a comment").unwrap();

        // GOTO, 42, REM and the comment text as typed (issue 178)
        assert_eq!(line.line_number, Some(10));
        assert_eq!(
            line.tokens,
            vec![
                Token::Keyword(0xE5), // GOTO
                Token::Integer(42),
                Token::Keyword(0xF4), // REM
                Token::DataText(" This is a comment".to_string()),
            ]
        );
    }

    /// The characters that the tokenizer does not know (issue 62).
    // { and } are the characters of a structure (issue #191)
    const UNKNOWN_CHARACTERS: [char; 6] = ['@', '|', '\\', '[', ']', '`'];

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
    fn test_tilde_is_an_operator_for_hex_output() {
        let line = tokenize("10 PRINT ~X").unwrap();
        assert_eq!(
            line.tokens,
            vec![
                Token::Keyword(0xF1),
                Token::Operator('~'),
                Token::Identifier("X".to_string()),
            ]
        );
        let line = tokenize("10 PRINT STR$~(X)").unwrap();
        assert_eq!(line.tokens[1], Token::Keyword(0xC3)); // STR$
        assert_eq!(line.tokens[2], Token::Operator('~'));
    }

    #[test]
    fn test_tilde_in_string_is_part_of_the_string() {
        let line = tokenize("10 PRINT \"~X\"").unwrap();
        assert_eq!(line.tokens[1], Token::String("~X".to_string()));
    }

    #[test]
    fn test_detokenize_shows_tilde_as_typed() {
        for source in ["10 PRINT ~X", "10 PRINT STR$~(X)", "10 PRINT ~-1;~A%"] {
            let line = tokenize(source).unwrap();
            assert_eq!(detokenize(&line).unwrap(), source);
        }
    }

    #[test]
    fn test_query_and_pling_are_memory_operators() {
        let line = tokenize("10 ?A=B!4").unwrap();
        assert_eq!(
            line.tokens,
            vec![
                Token::Operator('?'),
                Token::Identifier("A".to_string()),
                Token::Operator('='),
                Token::Identifier("B".to_string()),
                Token::Operator('!'),
                Token::Integer(4),
            ]
        );
    }

    #[test]
    fn test_minus_after_memory_operator_is_part_of_the_number() {
        let line = tokenize("10 PRINT ?-1").unwrap();
        assert_eq!(line.tokens[2], Token::Integer(-1));
        let line = tokenize("10 PRINT ?A-1").unwrap();
        assert_eq!(line.tokens[3], Token::Operator('-'));
    }

    #[test]
    fn test_detokenize_shows_memory_operators_as_typed() {
        for source in ["10 ?A = 5", "10 A!4 = B?1", "10 PRINT ?A;!B"] {
            let line = tokenize(source).unwrap();
            assert_eq!(detokenize(&line).unwrap(), source);
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

    #[test]
    fn test_number_can_start_with_decimal_point() {
        assert_eq!(tokenize("PRINT .5").unwrap().tokens[1], Token::Real(0.5));
        assert_eq!(
            tokenize("PRINT .25E2").unwrap().tokens[1],
            Token::Real(25.0)
        );
        assert_eq!(
            tokenize("X = .5 + .5").unwrap().tokens,
            vec![
                Token::Identifier("X".to_string()),
                Token::Operator('='),
                Token::Real(0.5),
                Token::Operator('+'),
                Token::Real(0.5),
            ]
        );
    }

    #[test]
    fn test_decimal_point_before_a_letter_is_not_a_number() {
        // No keyword starts with X, so `X.` is not a short form (issue 167).
        // A full stop and a letter after a name start a member (issue #191).
        assert_eq!(
            tokenize("10 PRINT X.B").unwrap().tokens,
            vec![
                Token::Keyword(0xF1),
                Token::Member("X".to_string(), vec!["B".to_string()])
            ]
        );
    }

    #[test]
    fn test_detokenize_shows_comparison_operators_as_typed() {
        for (source, listed) in [
            ("10 IF A% <= B% THEN PRINT 1", "10 IF A% <= B% THEN PRINT 1"),
            ("10 IF A% >= B% THEN PRINT 1", "10 IF A% >= B% THEN PRINT 1"),
            ("10 IF A% <> B% THEN PRINT 1", "10 IF A% <> B% THEN PRINT 1"),
        ] {
            let line = tokenize(source).unwrap();
            assert_eq!(detokenize(&line).unwrap(), listed);
        }
    }

    #[test]
    fn test_detokenized_operators_tokenize_to_the_same_tokens() {
        for op in ["<=", ">=", "<>", "<<", ">>", "<", ">", "="] {
            let source = format!("10 PRINT A% {op} B%");
            let line = tokenize(&source).unwrap();
            let listed = detokenize(&line).unwrap();
            assert!(listed.contains(&format!(" {op} ")), "{listed}");
            assert_eq!(tokenize(&listed).unwrap(), line, "{listed}");
        }
    }

    #[test]
    fn test_data_keeps_rest_of_line_as_one_text_token() {
        let line = tokenize("10 DATA 1 : REM x ELSE ~").unwrap();
        assert_eq!(
            line.tokens,
            vec![
                Token::Keyword(0xDC),
                Token::DataText(" 1 : REM x ELSE ~".to_string())
            ]
        );
    }

    #[test]
    fn test_data_with_no_text_gives_only_the_keyword() {
        let line = tokenize("10 DATA").unwrap();
        assert_eq!(line.tokens, vec![Token::Keyword(0xDC)]);
    }

    #[test]
    fn test_detokenize_data_line_lists_as_typed() {
        let source = "10 DATA  \"A,B\" ,C:D";
        assert_eq!(detokenize(&tokenize(source).unwrap()).unwrap(), source);
    }

    // Issue 87: a `-` starts a negative number only when no value is before it.

    fn tokens_of(source: &str) -> Vec<Token> {
        tokenize(source).unwrap().tokens
    }

    #[test]
    fn test_minus_after_number_is_subtract() {
        assert_eq!(
            tokens_of("PRINT 5-3"),
            vec![
                Token::Keyword(0xF1),
                Token::Integer(5),
                Token::Operator('-'),
                Token::Integer(3)
            ]
        );
    }

    #[test]
    fn test_minus_after_identifier_is_subtract() {
        assert_eq!(
            tokens_of("Y%=X%-1"),
            vec![
                Token::Identifier("Y%".to_string()),
                Token::Operator('='),
                Token::Identifier("X%".to_string()),
                Token::Operator('-'),
                Token::Integer(1)
            ]
        );
        assert_eq!(
            tokens_of("Y=A-B"),
            vec![
                Token::Identifier("Y".to_string()),
                Token::Operator('='),
                Token::Identifier("A".to_string()),
                Token::Operator('-'),
                Token::Identifier("B".to_string())
            ]
        );
    }

    #[test]
    fn test_minus_after_closing_bracket_is_subtract() {
        let tokens = tokens_of("Y=A(1)-1");
        assert_eq!(
            &tokens[tokens.len() - 3..],
            &[
                Token::Separator(')'),
                Token::Operator('-'),
                Token::Integer(1)
            ]
        );
    }

    #[test]
    fn test_minus_after_string_is_subtract() {
        let tokens = tokens_of("Y=\"A\"-1");
        assert_eq!(
            &tokens[tokens.len() - 2..],
            &[Token::Operator('-'), Token::Integer(1)]
        );
    }

    #[test]
    fn test_minus_after_value_keyword_is_subtract() {
        assert_eq!(
            tokens_of("Y=PI-3"),
            vec![
                Token::Identifier("Y".to_string()),
                Token::Operator('='),
                Token::Keyword(0xAF),
                Token::Operator('-'),
                Token::Integer(3)
            ]
        );
    }

    #[test]
    fn test_minus_after_operator_starts_negative_number() {
        assert_eq!(
            tokens_of("Y=2*-3"),
            vec![
                Token::Identifier("Y".to_string()),
                Token::Operator('='),
                Token::Integer(2),
                Token::Operator('*'),
                Token::Integer(-3)
            ]
        );
        assert_eq!(
            tokens_of("A = -1"),
            vec![
                Token::Identifier("A".to_string()),
                Token::Operator('='),
                Token::Integer(-1)
            ]
        );
        assert_eq!(tokens_of("Y=X%+-1")[4..], [Token::Integer(-1)]);
    }

    #[test]
    fn test_minus_after_opening_bracket_starts_negative_number() {
        assert_eq!(
            tokens_of("PRINT (-3)"),
            vec![
                Token::Keyword(0xF1),
                Token::Separator('('),
                Token::Integer(-3),
                Token::Separator(')')
            ]
        );
    }

    #[test]
    fn test_minus_after_keyword_starts_negative_number() {
        assert_eq!(
            tokens_of("PRINT -1"),
            vec![Token::Keyword(0xF1), Token::Integer(-1)]
        );
        let tokens = tokens_of("FOR I% = 10 TO 1 STEP -1");
        assert_eq!(
            &tokens[tokens.len() - 2..],
            &[Token::Keyword(0x88), Token::Integer(-1)]
        );
    }

    #[test]
    fn test_minus_after_comma_starts_negative_number() {
        assert_eq!(
            tokens_of("PRINT 1,-2"),
            vec![
                Token::Keyword(0xF1),
                Token::Integer(1),
                Token::Separator(','),
                Token::Integer(-2)
            ]
        );
    }

    #[test]
    fn test_minus_before_decimal_point_number_is_subtract() {
        assert_eq!(
            tokens_of("Y=X-.5"),
            vec![
                Token::Identifier("Y".to_string()),
                Token::Operator('='),
                Token::Identifier("X".to_string()),
                Token::Operator('-'),
                Token::Real(0.5)
            ]
        );
    }

    #[test]
    fn test_line_number_is_not_changed_by_minus_rule() {
        let line = tokenize("10 PRINT 1").unwrap();
        assert_eq!(line.line_number, Some(10));
        assert_eq!(line.tokens, vec![Token::Keyword(0xF1), Token::Integer(1)]);
    }

    #[test]
    fn test_detokenize_subtract_reads_back_to_same_tokens() {
        let line = tokenize("10 PRINT 5-3").unwrap();
        let listed = detokenize(&line).unwrap();
        assert_eq!(tokenize(&listed).unwrap(), line);
    }

    /// Tokenize the source, list it, and check that the listed line is the
    /// expected text and tokenizes to the same tokens
    fn assert_round_trip(source: &str, listed: &str) {
        let line = tokenize(source).unwrap();
        let text = detokenize(&line).unwrap();
        assert_eq!(text, listed, "{source}");
        assert_eq!(tokenize(&text).unwrap(), line, "{source}");
    }

    #[test]
    fn test_detokenize_shows_hex_number_as_typed() {
        for source in [
            "10 PRINT &1F",
            "10 PRINT &FFFFFFFF",
            "10 A = &10 + 1",
            "10 PRINT &80000000",
        ] {
            assert_round_trip(source, source);
        }
    }

    #[test]
    fn test_detokenize_shows_hex_number_with_no_leading_zeros() {
        assert_round_trip("10 PRINT &001F", "10 PRINT &1F");
        assert_round_trip("10 PRINT &0", "10 PRINT &0");
    }

    #[test]
    fn test_detokenize_puts_no_space_between_function_and_bracket() {
        for source in [
            "10 PRINT VAL(\"1\")",
            "10 PRINT LEN(A$)",
            "10 PRINT LEFT$(A$,1)",
            "10 PRINT TAB(5)",
            "10 PRINT SPC(2)",
            "10 PRINT SIN(X)",
            "10 PRINT FNa(1)",
            "10 PROCb(2)",
            "10 DIM C(3)",
        ] {
            assert_round_trip(source, source);
        }
        assert_round_trip("10 PRINT LEFT$(A$, 1)", "10 PRINT LEFT$(A$,1)");
    }

    #[test]
    fn test_detokenize_keeps_space_after_statement_keyword_and_operator() {
        for source in [
            "10 PRINT (1 + 2)",
            "10 IF (A) THEN PRINT 1",
            "10 FOR I = 1 TO (2)",
            "10 A = NOT (B)",
        ] {
            assert_round_trip(source, source);
        }
    }

    #[test]
    fn test_minus_after_hex_number_is_subtract() {
        assert_eq!(
            tokens_of("PRINT &10-1"),
            vec![
                Token::Keyword(0xF1),
                Token::HexInteger(16),
                Token::Operator('-'),
                Token::Integer(1)
            ]
        );
        assert_round_trip("10 PRINT &10-1", "10 PRINT &10 - 1");
    }

    #[test]
    fn test_detokenize_puts_space_on_both_sides_of_minus_after_bracket() {
        assert_round_trip("10 PRINT LEN(\"ABC\")-1", "10 PRINT LEN(\"ABC\") - 1");
        assert_round_trip("10 Y = A(1)-B", "10 Y = A(1) - B");
    }

    /// The minimum short forms of the BBC Microcomputer System User Guide,
    /// chapter 47 "Minimum abbreviations", pages 483 to 484 (issue 167).
    const GUIDE_SHORT_FORMS: &[(&str, &str)] = &[
        ("AD.", "ADVAL"),
        ("A.", "AND"),
        ("AU.", "AUTO"),
        ("B.", "BGET"),
        ("BP.", "BPUT"),
        ("CA.", "CALL"),
        ("CH.", "CHAIN"),
        ("CHR.", "CHR$"),
        ("CL.", "CLEAR"),
        ("CLO.", "CLOSE"),
        ("C.", "COLOUR"),
        ("COU.", "COUNT"),
        ("D.", "DATA"),
        ("DEL.", "DELETE"),
        ("DR.", "DRAW"),
        ("EL.", "ELSE"),
        ("E.", "ENDPROC"),
        ("ENV.", "ENVELOPE"),
        ("ERR.", "ERROR"),
        ("EV.", "EVAL"),
        ("FA.", "FALSE"),
        ("F.", "FOR"),
        ("GC.", "GCOL"),
        ("GE.", "GET$"),
        ("GOS.", "GOSUB"),
        ("G.", "GOTO"),
        ("H.", "HIMEM"),
        ("INK.", "INKEY$"),
        ("I.", "INPUT"),
        ("INS.", "INSTR"),
        ("LE.", "LEFT$"),
        ("LIN.", "LINE"),
        ("L.", "LIST"),
        ("LO.", "LOAD"),
        ("LOC.", "LOCAL"),
        ("LOM.", "LOMEM"),
        ("M.", "MID$"),
        ("MO.", "MODE"),
        ("MOV.", "MOVE"),
        ("N.", "NEXT"),
        ("O.", "OLD"),
        ("OP.", "OPENIN"),
        ("OPENO.", "OPENOUT"),
        ("OS.", "OSCLI"),
        ("PA.", "PAGE"),
        ("PL.", "PLOT"),
        ("PO.", "POINT"),
        ("P.", "PRINT"),
        ("PRO.", "PROC"),
        ("PT.", "PTR"),
        ("REA.", "READ"),
        ("REN.", "RENUMBER"),
        ("REP.", "REPEAT"),
        ("REPO.", "REPORT"),
        ("RES.", "RESTORE"),
        ("R.", "RETURN"),
        ("RI.", "RIGHT$"),
        ("SA.", "SAVE"),
        ("SO.", "SOUND"),
        ("S.", "STEP"),
        ("STO.", "STOP"),
        ("STR.", "STR$"),
        ("STRI.", "STRING$"),
        ("T.", "TAN"),
        ("TH.", "THEN"),
        ("TI.", "TIME"),
        ("TR.", "TRACE"),
        ("U.", "UNTIL"),
        ("V.", "VDU"),
        ("VP.", "VPOS"),
        ("W.", "WIDTH"),
    ];

    #[test]
    fn test_each_short_form_of_the_guide_gives_its_keyword() {
        for &(short, full) in GUIDE_SHORT_FORMS {
            assert_eq!(
                tokens_of(short),
                tokens_of(full),
                "{short} is the short form of {full}"
            );
        }
    }

    #[test]
    fn test_short_form_longer_than_the_minimum_gives_the_keyword() {
        for (short, full) in [
            ("PR.", "PRINT"),
            ("PRI.", "PRINT"),
            ("PRIN.", "PRINT"),
            ("GO.", "GOTO"),
            ("REPE.", "REPEAT"),
            ("OPEN.", "OPENIN"),
        ] {
            assert_eq!(tokens_of(short), tokens_of(full), "{short} gives {full}");
        }
    }

    #[test]
    fn test_short_form_of_the_shorter_minimum_wins() {
        // ENDPROC has the minimum E., and END has no short form
        assert_eq!(tokens_of("END."), tokens_of("ENDPROC"));
        // ERR has no short form, so ERR. is ERROR
        assert_eq!(tokens_of("ERR."), tokens_of("ERROR"));
        // INKEY and GET have no short form, so INKEY. and GET. are the $ forms
        assert_eq!(tokens_of("INKEY."), tokens_of("INKEY$"));
        assert_eq!(tokens_of("GET."), tokens_of("GET$"));
    }

    #[test]
    fn test_full_keyword_and_full_stop_is_not_a_short_form() {
        // The full stop after a full keyword starts a number, as before
        assert_eq!(tokens_of("PRINT.5"), tokens_of("PRINT .5"));
        assert_eq!(expand_short_form("PRINT.5"), "PRINT.5");
    }

    #[test]
    fn test_short_form_in_a_statement_gives_the_keyword() {
        assert_eq!(
            tokens_of("P.\"A\";1.5"),
            vec![
                Token::Keyword(0xF1),
                Token::String("A".to_string()),
                Token::Separator(';'),
                Token::Real(1.5),
            ]
        );
        assert_eq!(tokens_of("P. .5"), tokens_of("PRINT .5"));
        assert_eq!(
            tokens_of("F.I=1 TO 3:P.I:N."),
            tokens_of("FOR I=1 TO 3:PRINT I:NEXT")
        );
        assert_round_trip("10 P.\"HI\"", "10 PRINT \"HI\"");
    }

    #[test]
    fn test_full_stop_after_a_closing_bracket_is_the_dot_operator() {
        let array = |name: &str| {
            vec![
                Token::Identifier(name.to_string()),
                Token::Separator('('),
                Token::Separator(')'),
            ]
        };
        let mut expected = array("A");
        expected.push(Token::Operator('='));
        expected.extend(array("B"));
        expected.push(Token::Operator('.'));
        expected.extend(array("C"));
        assert_eq!(tokens_of("A() = B() . C()"), expected);
        assert_eq!(tokens_of("A()=B().C()"), expected);
        // A full stop that starts a number is still a number
        assert_eq!(
            tokens_of("(2).5"),
            vec![
                Token::Separator('('),
                Token::Integer(2),
                Token::Separator(')'),
                Token::Real(0.5),
            ]
        );
    }

    #[test]
    fn test_letters_that_are_not_a_short_form_give_the_error_of_today() {
        for text in ["X.", "PRINT X.", "p.", "PRINTX.", "ER.", "CLG."] {
            assert!(
                matches!(
                    tokenize(text),
                    Err(BBCBasicError::SyntaxError { ref message, .. })
                        if message == "Unknown character '.'"
                ),
                "{text} is not a short form"
            );
        }
    }

    #[test]
    fn test_full_stop_in_string_rem_and_data_is_not_a_short_form() {
        assert_eq!(
            tokens_of("PRINT \"P.\""),
            vec![Token::Keyword(0xF1), Token::String("P.".to_string())]
        );
        assert_round_trip("10 REM P. L.", "10 REM P. L.");
        assert_round_trip("10 DATA P.,L.", "10 DATA P.,L.");
    }

    #[test]
    fn test_name_p_is_still_a_variable() {
        assert_eq!(
            tokens_of("P=1.5"),
            vec![
                Token::Identifier("P".to_string()),
                Token::Operator('='),
                Token::Real(1.5),
            ]
        );
    }

    #[test]
    fn test_short_form_at_the_start_of_a_prompt_line_is_expanded() {
        assert_eq!(expand_short_form("L."), "LIST");
        assert_eq!(expand_short_form("L.10,20"), "LIST10,20");
        assert_eq!(expand_short_form("SA. \"FILE\""), "SAVE \"FILE\"");
        assert_eq!(expand_short_form("P.\"A.\""), "PRINT\"A.\"");
        assert_eq!(expand_short_form("LIST"), "LIST");
        assert_eq!(expand_short_form("X."), "X.");
        assert_eq!(expand_short_form("10 P.5"), "10 P.5");
        assert_eq!(expand_short_form("l."), "l.");
    }

    // Structures (issue #191)

    #[test]
    fn test_name_full_stop_and_letter_is_a_member() {
        assert_eq!(
            tokens_of("p.x%=XY.n$"),
            vec![
                Token::Member("p".to_string(), vec!["x%".to_string()]),
                Token::Operator('='),
                Token::Member("XY".to_string(), vec!["n$".to_string()]),
            ]
        );
    }

    #[test]
    fn test_a_member_path_is_one_token() {
        // A nested structure and an array member (issue #192)
        let path = |names: &[&str]| names.iter().map(|name| name.to_string()).collect();
        assert_eq!(
            tokens_of("s.p._q.t$=s.a(1).5"),
            vec![
                Token::Member("s".to_string(), path(&["p", "_q", "t$"])),
                Token::Operator('='),
                Token::Member("s".to_string(), path(&["a"])),
                Token::Separator('('),
                Token::Integer(1),
                Token::Separator(')'),
                Token::Real(0.5),
            ]
        );
        assert_round_trip("10 s.p.q.t$ = s.a(1)", "10 s.p.q.t$ = s.a(1)");
    }

    #[test]
    fn test_short_form_wins_over_a_member() {
        assert_eq!(
            tokens_of("P.x"),
            vec![Token::Keyword(0xF1), Token::Identifier("x".to_string())]
        );
    }

    #[test]
    fn test_keywords_and_typed_names_do_not_start_a_member() {
        for text in ["PRINT.x", "a%.x", "a$.x", "PROCa.x"] {
            assert!(tokenize(text).is_err(), "{text} is not a member");
        }
    }

    #[test]
    fn test_a_member_needs_a_letter_after_the_full_stop() {
        assert_eq!(
            tokens_of("x.5"),
            vec![Token::Identifier("x".to_string()), Token::Real(0.5)]
        );
    }

    #[test]
    fn test_braces_are_separators_and_list_as_typed() {
        assert_eq!(
            tokens_of("DIM p{x}"),
            vec![
                Token::Keyword(0xDE),
                Token::Identifier("p".to_string()),
                Token::Separator('{'),
                Token::Identifier("x".to_string()),
                Token::Separator('}'),
            ]
        );
        assert_round_trip("10 DIM p{x,y%},q{s$}", "10 DIM p{x,y%},q{s$}");
        assert_round_trip("20 p.x?1 = q.s$", "20 p.x?1 = q.s$");
    }

    #[test]
    fn test_is_short_form() {
        assert!(is_short_form("P"));
        assert!(is_short_form("PT"));
        assert!(!is_short_form("XY"));
        assert!(!is_short_form("p"));
    }
}
