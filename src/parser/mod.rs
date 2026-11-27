//! Parser for BBC BASIC statements and expressions
//!
//! Analyzes tokenized BBC BASIC statements and creates abstract syntax trees
//! for execution.

use crate::error::BBCBasicError;
use crate::error::Result;
use crate::tokenizer::{create_reverse_keyword_maps, detokenize, Token, TokenizedLine};

/// The tokens as the user typed them, for an error message. An empty list
/// shows as "end of line". A whole real number shows with no exponent, so
/// `2.0` shows as `2`, not as the LIST form `2E0`.
fn tokens_text(tokens: &[Token]) -> String {
    let tokens = tokens
        .iter()
        .map(|token| match token {
            Token::Real(value)
                if value.fract() == 0.0
                    && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(value) =>
            {
                Token::Integer(*value as i32)
            }
            _ => token.clone(),
        })
        .collect();
    let line = TokenizedLine::new(None, tokens);
    match detokenize(&line) {
        Ok(text) if !text.is_empty() => text,
        _ => "end of line".to_string(),
    }
}

/// Binary operators in BBC BASIC
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    IntegerDivide,
    Modulo,
    Power,

    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Logical
    And,
    Or,
    Eor, // Exclusive OR

    // Bitwise
    LeftShift,  // <<
    RightShift, // >>

    // String
    StringConcat, // String concatenation
}

/// Unary operators in BBC BASIC
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
    /// `?addr` or `!addr`: the value in memory at the address
    Indirect(Indirection),
}

/// The width of a memory operator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Indirection {
    /// `?`: one byte, 0 to 255
    Byte,
    /// `!`: four bytes, low byte first, as a signed integer
    Word,
}

impl Indirection {
    fn from_operator(token: Option<&Token>) -> Option<Self> {
        match token {
            Some(Token::Operator('?')) => Some(Indirection::Byte),
            Some(Token::Operator('!')) => Some(Indirection::Word),
            _ => None,
        }
    }
}

/// BBC BASIC expressions
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// Integer literal
    Integer(i32),
    /// Real number literal
    Real(f64),
    /// String literal
    String(String),
    /// Variable reference
    Variable(String),
    /// Array access with indices
    ArrayAccess {
        name: String,
        indices: Vec<Expression>,
    },
    /// Function call
    FunctionCall { name: String, args: Vec<Expression> },
    /// Binary operation
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    /// Unary operation
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },
}

/// Print item types for PRINT statements
#[derive(Debug, Clone, PartialEq)]
pub enum PrintItem {
    Expression(Expression),
    Tab(Expression),               // TAB(n)
    TabXY(Expression, Expression), // TAB(x, y): the same as VDU 31, x, y
    Spc(Expression),               // SPC(n)
    Hex,                           // ~ : the numeric items up to the next , or ; print in hex
    Semicolon,                     // ;
    Comma,                         // ,
    NewLine,                       // ' : print a new line and set the column to 0
}

/// Item of a VDU statement. A semicolon after the item sends it as 2 bytes,
/// low byte first. A comma or no separator sends it as 1 byte.
#[derive(Debug, Clone, PartialEq)]
pub struct VduItem {
    pub value: Expression,
    pub two_bytes: bool,
}

/// Item of an INPUT or INPUT LINE statement, in the order of the source
#[derive(Debug, Clone, PartialEq)]
pub enum InputItem {
    /// A prompt text. `question_mark` is true if a comma or a semicolon follows
    /// the text, so that the next variable shows `?` after the text.
    Prompt { text: String, question_mark: bool },
    /// An apostrophe: print a new line and set the column to 0
    NewLine,
    /// A variable that gets a reply
    Variable(String),
    /// An array element that gets a reply. The indices are evaluated when
    /// the reply is stored.
    ArrayElement {
        name: String,
        indices: Vec<Expression>,
    },
}

/// The form of a TRACE statement
#[derive(Debug, Clone, PartialEq)]
pub enum TraceMode {
    /// TRACE ON: trace each line
    On,
    /// TRACE OFF: stop the trace
    Off,
    /// TRACE n: trace only the lines with a number below n
    Below(Expression),
}

/// BBC BASIC statements
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Variable assignment (LET A = 5 or A = 5)
    Assignment {
        target: String,
        expression: Expression,
    },
    /// Array element assignment
    ArrayAssignment {
        name: String,
        indices: Vec<Expression>,
        expression: Expression,
    },
    /// Memory write: `?addr = value`, `!addr = value`, `base?offset = value`
    /// or `base!offset = value`. The address includes the offset.
    MemoryAssignment {
        width: Indirection,
        address: Expression,
        expression: Expression,
    },
    /// PRINT statement
    Print { items: Vec<PrintItem> },
    /// INPUT statement
    Input { items: Vec<InputItem> },
    /// INPUT LINE statement: each variable gets one full reply line
    InputLine { items: Vec<InputItem> },
    /// FOR loop
    For {
        variable: String,
        start: Expression,
        end: Expression,
        step: Option<Expression>,
    },
    /// `FOR EACH variable IN collection` (issue #141, a modern extension):
    /// a loop for each element of a list or each key of a map
    ForEach {
        variable: String,
        collection: Expression,
    },
    /// NEXT statement
    Next { variables: Vec<String> },
    /// IF statement
    If {
        condition: Expression,
        then_part: Vec<Statement>,
        else_part: Option<Vec<Statement>>,
    },
    /// `IF cond THEN` with nothing after THEN - starts a block IF (BBC
    /// BASIC V). The lines up to the matching ELSE or ENDIF line are the
    /// THEN block.
    IfBlock { condition: Expression },
    /// ELSE at the start of a line - starts the ELSE block of a block IF
    BlockElse,
    /// ENDIF at the start of a line - ends a block IF
    EndIf,
    /// `CASE expr OF` - the last statement of a line. It starts a CASE
    /// structure (BBC BASIC V). The WHEN, OTHERWISE and ENDCASE lines of the
    /// structure follow it.
    Case { value: Expression },
    /// `WHEN v1, v2` at the start of a line - starts a case of a CASE
    /// structure. The statements after it (after a colon) are in the case.
    When { values: Vec<Expression> },
    /// OTHERWISE at the start of a line - starts the default case
    Otherwise,
    /// ENDCASE - ends a CASE structure. Only an ENDCASE at the start of a
    /// line counts. Another ENDCASE does nothing.
    EndCase,
    /// GOTO statement
    Goto { line_number: u16 },
    /// GOSUB statement
    Gosub { line_number: u16 },
    /// GOTO with a calculated line number, such as `GOTO (100 + X)`. The
    /// runner evaluates the target when the statement runs.
    GotoExpression { target: Expression },
    /// GOSUB with a calculated line number, such as `GOSUB X * 10`
    GosubExpression { target: Expression },
    /// RETURN statement (optionally with value for functions)
    Return { value: Option<Expression> },
    /// DIM statement: arrays, and blocks of memory such as `DIM B% 10`
    Dim { items: Vec<DimItem> },
    /// REM statement (comment)
    Rem { comment: String },
    /// END statement
    End,
    /// STOP statement
    Stop,
    /// QUIT statement (like END but exits immediately)
    Quit,
    /// Procedure call
    ProcCall { name: String, args: Vec<Expression> },
    /// DEF PROC - define a procedure
    DefProc { name: String, params: Vec<String> },
    /// DEF FN - define a function. A single-line function has its return
    /// expression. A multi-line function has no expression: its body lines
    /// follow, and a `=expr` line ends it.
    DefFn {
        name: String,
        params: Vec<String>,
        expression: Option<Expression>,
    },
    /// `=expr` - end a multi-line function and give its result
    FnReturn { value: Expression },
    /// ENDPROC - end procedure definition
    EndProc,
    /// LOCAL statement - declares local variables in a procedure
    Local { variables: Vec<String> },
    /// DATA statement - stores data values
    Data { values: Vec<DataValue> },
    /// READ statement - reads data into variables and array elements. A
    /// target is never an `InputItem::Prompt`.
    Read { variables: Vec<InputItem> },
    /// SWAP statement - exchange the values of 2 variables or array
    /// elements (BBC BASIC V, issue #162). A target is never an
    /// `InputItem::Prompt`.
    Swap { first: InputItem, second: InputItem },
    /// RESTORE statement - resets data pointer (optionally to specific line)
    Restore { line_number: Option<u16> },
    /// REPEAT statement - starts a REPEAT...UNTIL loop
    Repeat,
    /// UNTIL statement - ends a REPEAT...UNTIL loop
    Until { condition: Expression },
    /// WHILE statement - starts a WHILE...ENDWHILE loop
    While { condition: Expression },
    /// ENDWHILE statement - ends a WHILE...ENDWHILE loop
    EndWhile,
    /// CLS statement - clear screen
    Cls,
    /// REPORT statement - print the text of the last error
    Report,
    /// CLEAR statement - remove all variables and close the loops and calls
    Clear,
    /// TRACE statement - show the number of each line that runs
    Trace { mode: TraceMode },
    /// ON GOTO statement - computed GOTO based on expression value. The
    /// ELSE part (the rest of the line) runs if the index is out of range.
    OnGoto {
        expression: Expression,
        targets: Vec<u16>,
        else_part: Option<Vec<Statement>>,
    },
    /// ON GOSUB statement - computed GOSUB based on expression value. The
    /// ELSE part (the rest of the line) runs if the index is out of range.
    OnGosub {
        expression: Expression,
        targets: Vec<u16>,
        else_part: Option<Vec<Statement>>,
    },
    /// ON ERROR GOTO statement - set error handler
    OnError { line_number: u16 },
    /// ON ERROR OFF statement - clear error handler
    OnErrorOff,
    /// ON ERROR followed by statements. The handler is the rest of the line.
    OnErrorStatements { statements: Vec<Statement> },
    /// PRINT# statement - write to file
    PrintFile {
        handle: Expression,
        items: Vec<PrintItem>,
    },
    /// INPUT# statement - read from file. Each target is a variable or an
    /// array element.
    InputFile {
        handle: Expression,
        targets: Vec<InputItem>,
    },
    /// CLOSE# statement - close file
    CloseFile { handle: Expression },
    /// BPUT# statement - write one byte to file
    Bput {
        handle: Expression,
        value: Expression,
    },
    /// PTR# assignment - set the position in a file
    SetPtr {
        handle: Expression,
        position: Expression,
    },
    /// SAVEJSON file$, json$ - write a JSON text to a file (extension, issue #102)
    SaveJson { file: Expression, json: Expression },
    /// PLOT statement - general plotting with mode code
    Plot {
        mode: Expression,
        x: Expression,
        y: Expression,
    },
    /// MOVE statement - move graphics cursor
    Move { x: Expression, y: Expression },
    /// DRAW statement - draw line to coordinates
    Draw { x: Expression, y: Expression },
    /// CIRCLE statement - draw a circle. CIRCLE FILL draws a filled circle
    Circle {
        x: Expression,
        y: Expression,
        radius: Expression,
        filled: bool,
    },
    /// GCOL statement - set graphics color
    Gcol { mode: Expression, color: Expression },
    /// CLG statement - clear graphics screen
    Clg,
    /// MODE statement - select a screen mode from 0 to 7
    Mode { mode: Expression },
    /// COLOUR statement - set the text foreground or background colour
    Colour { colour: Expression },
    /// VDU statement - send bytes to the display
    Vdu { items: Vec<VduItem> },
    /// ELLIPSE statement - draw an ellipse. ELLIPSE FILL draws a filled ellipse
    Ellipse {
        x: Expression,
        y: Expression,
        major: Expression,
        minor: Expression,
        filled: bool,
    },
    /// RECTANGLE statement - draw a rectangle outline. RECTANGLE FILL draws a
    /// filled rectangle
    Rectangle {
        x1: Expression,
        y1: Expression,
        width: Expression,
        height: Expression,
        filled: bool,
    },
    /// FILL statement - flood fill from coordinates
    Fill { x: Expression, y: Expression },
    /// ORIGIN statement - set graphics origin
    Origin { x: Expression, y: Expression },
    /// STEREO statement - set the stereo position of a sound channel.
    /// It has no effect until sound exists (issue #9).
    Stereo {
        channel: Expression,
        position: Expression,
    },
    /// MKDIR statement - make one directory (extension, not BBC BASIC)
    MakeDirectory { path: Expression },
    /// RMDIR statement - remove one empty directory (extension, not BBC BASIC)
    RemoveDirectory { path: Expression },
    /// LISTADD l, value - add an element at the end of a list (extension, issue #139)
    ListAdd { list: Expression, value: Expression },
    /// LISTSET l, i, value - change the element at an index (extension)
    ListSet {
        list: Expression,
        index: Expression,
        value: Expression,
    },
    /// LISTINSERT l, i, value - put an element before an index (extension)
    ListInsert {
        list: Expression,
        index: Expression,
        value: Expression,
    },
    /// LISTREMOVE l, i - remove the element at an index (extension)
    ListRemove { list: Expression, index: Expression },
    /// LISTCLEAR l - remove all elements of a list (extension)
    ListClear { list: Expression },
    /// MAPPUT m, key$, value - put a value for a key of a map (extension, issue #140)
    MapPut {
        map: Expression,
        key: Expression,
        value: Expression,
    },
    /// MAPREMOVE m, key$ - remove a key of a map (extension)
    MapRemove { map: Expression, key: Expression },
    /// MAPCLEAR m - remove all keys of a map (extension)
    MapClear { map: Expression },
    /// LISTADDCOL l, handle - add a nested collection at the end of a list
    /// (extension, issue #142)
    ListAddCollection {
        list: Expression,
        handle: Expression,
    },
    /// MAPPUTCOL m, key$, handle - put a nested collection for a key of a
    /// map (extension, issue #142)
    MapPutCollection {
        map: Expression,
        key: Expression,
        handle: Expression,
    },
    /// Empty statement
    Empty,
}

/// One item of a DIM statement
#[derive(Debug, Clone, PartialEq)]
pub enum DimItem {
    /// An array, such as `A%(10)` or `B(5, 5)`
    Array {
        name: String,
        dimensions: Vec<Expression>,
    },
    /// A block of `size + 1` bytes, such as `B% 10`. The numeric variable
    /// gets the address of the first byte.
    Block { name: String, size: Expression },
}

/// Data value types for DATA statement
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    Integer(i32),
    Real(f64),
    String(String),
}

impl Statement {
    /// Check if this statement is a control flow statement
    pub fn is_control_flow(&self) -> bool {
        matches!(
            self,
            Statement::For { .. }
                | Statement::ForEach { .. }
                | Statement::Next { .. }
                | Statement::If { .. }
                | Statement::Goto { .. }
                | Statement::Gosub { .. }
                | Statement::Return { .. }
        )
    }

    /// Check if this statement ends program execution
    pub fn is_terminating(&self) -> bool {
        matches!(self, Statement::End | Statement::Stop | Statement::Quit)
    }
}

impl Expression {
    /// Get the type of expression for type checking
    pub fn expression_type(&self) -> ExpressionType {
        match self {
            Expression::Integer(_) => ExpressionType::Integer,
            Expression::Real(_) => ExpressionType::Real,
            Expression::String(_) => ExpressionType::String,
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    ExpressionType::Integer
                } else if name.ends_with('$') {
                    ExpressionType::String
                } else {
                    ExpressionType::Real
                }
            }
            Expression::ArrayAccess { name, .. } => {
                if name.ends_with('%') {
                    ExpressionType::Integer
                } else if name.ends_with('$') {
                    ExpressionType::String
                } else {
                    ExpressionType::Real
                }
            }
            Expression::FunctionCall { .. } => ExpressionType::Unknown, // Depends on function
            Expression::BinaryOp { op, .. } => match op {
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Power => ExpressionType::Numeric,
                BinaryOperator::IntegerDivide | BinaryOperator::Modulo => ExpressionType::Integer,
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::LessThan
                | BinaryOperator::LessThanOrEqual
                | BinaryOperator::GreaterThan
                | BinaryOperator::GreaterThanOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Eor
                | BinaryOperator::LeftShift
                | BinaryOperator::RightShift => ExpressionType::Integer,
                BinaryOperator::StringConcat => ExpressionType::String,
            },
            Expression::UnaryOp { op, .. } => match op {
                UnaryOperator::Plus | UnaryOperator::Minus => ExpressionType::Numeric,
                UnaryOperator::Not | UnaryOperator::Indirect(_) => ExpressionType::Integer,
            },
        }
    }
}

/// Expression type for type checking
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionType {
    Integer,
    Real,
    String,
    Numeric, // Either integer or real
    Unknown,
}

/// Parse all the statements of a line, in order.
///
/// A colon separates the statements. A REM, DATA or IF statement uses the
/// rest of the line, so a colon after it is not a separator. A string is
/// one token, so a colon in a string is never a separator.
///
/// An ELSE or ENDIF at the start of a line is a line of a block IF. The
/// statements after it (after an optional colon) follow it.
///
/// A WHEN, OTHERWISE or ENDCASE at the start of a line is a line of a CASE
/// structure. A colon follows the values of a WHEN and an ENDCASE. A
/// `CASE expr OF` must be the last statement of its line.
pub fn parse_line(line: &TokenizedLine) -> Result<Vec<Statement>> {
    let line_number = line.line_number;
    let (marker, rest) = match (line.tokens.split_first(), case_word(&line.tokens)) {
        (Some((Token::Keyword(0x8B), rest)), _) => (Statement::BlockElse, rest),
        (Some((Token::ExtendedKeyword(0xC8, 0xA5), rest)), _) => (Statement::EndIf, rest),
        (Some((_, rest)), Some("OTHERWISE")) => (Statement::Otherwise, rest),
        (Some((_, rest)), Some("ENDCASE")) => {
            expect_colon_after("ENDCASE", rest, line_number)?;
            (Statement::EndCase, rest)
        }
        (Some((_, rest)), Some(_)) => {
            let end = rest
                .iter()
                .position(|t| matches!(t, Token::Separator(':')))
                .unwrap_or(rest.len());
            let values = parse_when_values(&rest[..end], line_number)?;
            (Statement::When { values }, &rest[end..])
        }
        _ => {
            let statements = parse_statements(&line.tokens, line_number)?;
            check_case_is_last(&statements, line_number)?;
            return Ok(statements);
        }
    };
    let rest = rest.strip_prefix(&[Token::Separator(':')]).unwrap_or(rest);
    let mut statements = vec![marker];
    if !rest.is_empty() {
        statements.extend(parse_statements(rest, line_number)?);
    }
    check_case_is_last(&statements, line_number)?;
    Ok(statements)
}

/// The word WHEN, OTHERWISE or ENDCASE if it starts the tokens as a word of
/// a CASE structure. The tokenizer gives these words as names. Before `=`,
/// or before brackets and `=`, the word is a variable or an array, so that
/// an old program with a variable of that name still works.
fn case_word(tokens: &[Token]) -> Option<&'static str> {
    let word = match tokens.first() {
        Some(Token::Identifier(name)) => ["WHEN", "OTHERWISE", "ENDCASE"]
            .into_iter()
            .find(|word| name == word)?,
        _ => return None,
    };
    (!is_assignment_target(tokens)).then_some(word)
}

/// True if `=` follows the name at the start of the tokens, or follows the
/// brackets after the name, as in `WHEN = 5` and `WHEN(3) = 5`.
fn is_assignment_target(tokens: &[Token]) -> bool {
    match tokens.get(1) {
        Some(Token::Operator('=')) => true,
        Some(Token::Separator('(')) => {
            let mut depth = 0;
            for (index, token) in tokens.iter().enumerate().skip(1) {
                match token {
                    Token::Separator('(') => depth += 1,
                    Token::Separator(')') => {
                        depth -= 1;
                        if depth == 0 {
                            return matches!(tokens.get(index + 1), Some(Token::Operator('=')));
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        _ => false,
    }
}

/// A `CASE expr OF` must be the last statement of its line, because the
/// WHEN lines of the structure follow the line. An empty statement or a REM
/// can follow it.
fn check_case_is_last(statements: &[Statement], line_number: Option<u16>) -> Result<()> {
    let end = statements
        .iter()
        .rposition(|s| !matches!(s, Statement::Empty | Statement::Rem { .. }))
        .unwrap_or(0);
    if statements[..end]
        .iter()
        .any(|s| matches!(s, Statement::Case { .. }))
    {
        return Err(BBCBasicError::SyntaxError {
            message: "CASE must be the last statement of a line".to_string(),
            line: line_number,
        });
    }
    Ok(())
}

/// Only the end of the statement or a colon can follow the word.
fn expect_colon_after(word: &str, rest: &[Token], line_number: Option<u16>) -> Result<()> {
    match rest.first() {
        None | Some(Token::Separator(':')) => Ok(()),
        Some(token) => Err(BBCBasicError::SyntaxError {
            message: format!(
                "Unexpected {} after {}",
                tokens_text(std::slice::from_ref(token)),
                word
            ),
            line: line_number,
        }),
    }
}

/// Parse the values of a WHEN: one expression or more, with a comma between
/// two expressions. A comma in the brackets of a function call is part of
/// the call.
fn parse_when_values(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<Expression>> {
    let mut values = Vec::new();
    let mut pos = 0;
    loop {
        if pos == tokens.len() {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected expression".to_string(),
                line: line_number,
            });
        }
        values.push(parse_expr_precedence(tokens, &mut pos, 0)?);
        match tokens.get(pos) {
            None => return Ok(values),
            Some(Token::Separator(',')) => pos += 1,
            Some(token) => {
                return Err(BBCBasicError::SyntaxError {
                    message: format!(
                        "Unexpected {} after expression",
                        tokens_text(std::slice::from_ref(token))
                    ),
                    line: line_number,
                })
            }
        }
    }
}

/// Parse `CASE expr OF`. The tokens are the tokens after CASE. The last word
/// is OF, or the word before a REM. The tokenizer gives OF as a name, so a
/// variable named OF still works.
fn parse_case_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let end = tokens
        .iter()
        .position(|t| matches!(t, Token::Keyword(0xF4)))
        .unwrap_or(tokens.len());
    let expression = match tokens[..end].split_last() {
        Some((Token::Identifier(word), expression)) if word == "OF" => expression,
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected OF after CASE expression".to_string(),
                line: line_number,
            })
        }
    };
    Ok(Statement::Case {
        value: parse_expression(expression)?,
    })
}

/// Split tokens at each statement separator and parse each part.
fn parse_statements(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<Statement>> {
    let mut statements = Vec::new();
    let mut rest = tokens;
    loop {
        // A statement can follow REPEAT with no colon, as in REPEAT UNTIL X
        if let [Token::Keyword(0xF5), next, ..] = rest {
            if !matches!(next, Token::Separator(':')) {
                statements.push(Statement::Repeat);
                rest = &rest[1..];
                continue;
            }
        }
        let uses_rest_of_line = matches!(
            rest.first(),
            Some(Token::Keyword(0xF4 | 0xDC | 0xE7)) // REM, DATA, IF
        ) || is_on_with_else(rest)
            || is_on_error_with_statements(rest);
        // An apostrophe comment (REM and its DataText) also ends a statement,
        // as in `X = 1 ' note` (issue 178). The comment is the next statement.
        let end = if uses_rest_of_line {
            rest.len()
        } else {
            rest.windows(2)
                .position(|pair| matches!(pair, [Token::Keyword(0xF4), Token::DataText(_)]))
                .into_iter()
                .chain(rest.iter().position(|t| matches!(t, Token::Separator(':'))))
                .min()
                .unwrap_or(rest.len())
        };
        let part = TokenizedLine::new(line_number, rest[..end].to_vec());
        statements.push(parse_statement(&part)?);
        if end == rest.len() {
            return Ok(statements);
        }
        let separator = usize::from(matches!(rest[end], Token::Separator(':')));
        rest = &rest[end + separator..];
    }
}

/// True if the tokens start with an ON GOTO or ON GOSUB statement that has
/// an ELSE before the next colon. Its ELSE part is the rest of the line.
/// ON ERROR has no ELSE part.
fn is_on_with_else(tokens: &[Token]) -> bool {
    let Some((Token::Keyword(0xEE), after_on)) = tokens.split_first() else {
        return false;
    };
    if matches!(after_on.first(), Some(Token::Keyword(0x85))) {
        return false;
    }
    after_on
        .iter()
        .take_while(|t| !matches!(t, Token::Separator(':')))
        .any(|t| matches!(t, Token::Keyword(0x8B)))
}

/// True if the tokens start with ON ERROR and a statement that is not GOTO,
/// OFF or LOCAL. The handler statements are the rest of the line.
fn is_on_error_with_statements(tokens: &[Token]) -> bool {
    matches!(
        tokens,
        [Token::Keyword(0xEE), Token::Keyword(0x85), next, ..]
            if !matches!(next, Token::Keyword(0xE5 | 0x87 | 0xEA))
    )
}

/// Parse a tokenized line into a statement
pub fn parse_statement(line: &TokenizedLine) -> Result<Statement> {
    let tokens = &line.tokens;

    if tokens.is_empty() {
        return Ok(Statement::Empty);
    }

    // END, STOP, CLS, CLEAR and REPORT take no argument
    if let [Token::Keyword(0xE0 | 0xFA | 0xDB | 0xD8 | 0xF6), unexpected, ..] = tokens.as_slice() {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "Unexpected {} after statement",
                tokens_text(std::slice::from_ref(unexpected))
            ),
            line: line.line_number,
        });
    }

    // A word of a CASE structure that is not at the start of a line. An
    // ENDCASE does nothing there, and the CASE scan does not count it.
    // WHEN and OTHERWISE are only at the start of a line (parse_line).
    match case_word(tokens) {
        Some("ENDCASE") => {
            expect_colon_after("ENDCASE", &tokens[1..], line.line_number)?;
            return Ok(Statement::EndCase);
        }
        Some(word) => {
            return Err(BBCBasicError::SyntaxError {
                message: format!("{} must be the first statement of a line", word),
                line: line.line_number,
            })
        }
        None => {}
    }

    // Check first token to determine statement type
    match &tokens[0] {
        // PRINT statement
        Token::Keyword(0xF1) => {
            // Check if it's PRINT# (file I/O) or regular PRINT
            if tokens.len() > 1 && matches!(tokens[1], Token::Operator('#')) {
                parse_print_file_statement(&tokens[2..], line.line_number)
            } else {
                parse_print_statement(&tokens[1..])
            }
        }

        // LET statement (optional keyword)
        Token::Keyword(0xE9) => {
            // LET is optional, parse assignment from token 1 onward
            if tokens.len() < 2 {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected variable after LET".to_string(),
                    line: line.line_number,
                });
            }
            parse_assignment(&tokens[1..], line.line_number)
        }

        // SAVEJSON file$, json$ (issue #102). With '=' or '(' after it, the
        // name is a variable or an array, so that old programs do not change.
        Token::Identifier(name)
            if name == "SAVEJSON"
                && !matches!(
                    tokens.get(1),
                    Some(Token::Operator('=') | Token::Separator('('))
                ) =>
        {
            parse_savejson_statement(&tokens[1..], line.line_number)
        }

        // Variable assignment (without LET keyword), or a memory write
        // such as ?A = 5
        Token::Identifier(_) | Token::Operator('?' | '!') => {
            match parse_assignment(tokens, line.line_number) {
                // MKDIR path$ and RMDIR path$ are extension statements. A line
                // that is a valid assignment, such as MKDIR = 3, keeps that meaning.
                Err(_) if matches!(&tokens[0], Token::Identifier(name) if name == "MKDIR") => {
                    Ok(Statement::MakeDirectory {
                        path: parse_expression(&tokens[1..])?,
                    })
                }
                Err(_) if matches!(&tokens[0], Token::Identifier(name) if name == "RMDIR") => {
                    Ok(Statement::RemoveDirectory {
                        path: parse_expression(&tokens[1..])?,
                    })
                }
                // The list statements (issue #139) and the map statements
                // (issue #140) use the same rule as MKDIR.
                Err(error) => match &tokens[0] {
                    Token::Identifier(name) if is_list_statement(name) => {
                        parse_list_statement(name, &tokens[1..], line.line_number)
                    }
                    _ => Err(error),
                },
                parsed => parsed,
            }
        }

        // FOR loop
        Token::Keyword(0xE3) => parse_for_statement(&tokens[1..], line.line_number),

        // NEXT statement
        Token::Keyword(0xED) => parse_next_statement(&tokens[1..]),

        // GOTO statement
        Token::Keyword(0xE5) => parse_goto_statement(&tokens[1..], line.line_number),

        // GOSUB statement
        Token::Keyword(0xE4) => parse_gosub_statement(&tokens[1..], line.line_number),

        // ON statement (ON GOTO / ON GOSUB)
        Token::Keyword(0xEE) => parse_on_statement(&tokens[1..], line.line_number),

        // RETURN statement (with optional expression for functions)
        Token::Keyword(0xF8) => {
            if tokens.len() > 1 {
                // Parse the expression after RETURN
                let expr = parse_expression(&tokens[1..])?;
                Ok(Statement::Return { value: Some(expr) })
            } else {
                // Just RETURN with no expression
                Ok(Statement::Return { value: None })
            }
        }

        // INPUT statement
        Token::Keyword(0xE8) => {
            // Check if it's INPUT# (file I/O) or regular INPUT
            if tokens.len() > 1 && matches!(tokens[1], Token::Operator('#')) {
                parse_input_file_statement(&tokens[2..], line.line_number)
            } else if tokens.len() > 1 && matches!(tokens[1], Token::Keyword(0x86)) {
                // INPUT LINE: 0x86 is the LINE token
                Ok(Statement::InputLine {
                    items: parse_input_items(&tokens[2..], line.line_number)?,
                })
            } else {
                parse_input_statement(&tokens[1..], line.line_number)
            }
        }

        // DIM statement
        Token::Keyword(0xDE) => parse_dim_statement(&tokens[1..], line.line_number),

        // IF statement
        Token::Keyword(0xE7) => parse_if_statement(&tokens[1..], line.line_number),

        // ELSE: parse_line reads it at the start of a line, and an IF reads
        // it in its line
        Token::Keyword(0x8B) => Err(BBCBasicError::SyntaxError {
            message: "ELSE must be the first statement of a line".to_string(),
            line: line.line_number,
        }),

        // END statement
        Token::Keyword(0xE0) => Ok(Statement::End),

        // STOP statement
        Token::Keyword(0xFA) => Ok(Statement::Stop),

        // REM statement (comment)
        Token::Keyword(0xF4) => {
            // Everything after REM is a comment
            let comment = tokens[1..]
                .iter()
                .map(|t| format!("{:?}", t))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(Statement::Rem { comment })
        }

        // DATA statement
        Token::Keyword(0xDC) => parse_data_statement(&tokens[1..], line.line_number),

        // READ statement
        Token::Keyword(0xF3) => parse_read_statement(&tokens[1..], line.line_number),

        // RESTORE statement
        Token::Keyword(0xF7) => parse_restore_statement(&tokens[1..], line.line_number),

        // REPEAT statement
        Token::Keyword(0xF5) => Ok(Statement::Repeat),

        // UNTIL statement
        Token::Keyword(0xFD) => parse_until_statement(&tokens[1..], line.line_number),

        // CLS statement
        Token::Keyword(0xDB) => Ok(Statement::Cls),

        // REPORT statement
        Token::Keyword(0xF6) => Ok(Statement::Report),

        // CLEAR statement
        Token::Keyword(0xD8) => Ok(Statement::Clear),

        // TRACE statement
        Token::Keyword(0xFC) => parse_trace_statement(&tokens[1..], line.line_number),

        // DEF statement (DEF PROC or DEF FN)
        Token::Keyword(0xDD) => parse_def_statement(&tokens[1..], line.line_number),

        // =expr: the end of a multi-line function and its result
        Token::Operator('=') => Ok(Statement::FnReturn {
            value: parse_expression(&tokens[1..])?,
        }),

        // ENDPROC statement
        Token::Keyword(0xE1) => Ok(Statement::EndProc),

        // LOCAL statement
        Token::Keyword(0xEA) => parse_local_statement(&tokens[1..], line.line_number),

        // PROC call (PROC followed by identifier)
        Token::Keyword(0xF2) => parse_proc_call(&tokens[1..], line.line_number),

        // CLOSE# statement (file I/O)
        Token::Keyword(0xD9) => {
            // CLOSE requires # after it for file I/O
            if tokens.len() > 1 && matches!(tokens[1], Token::Operator('#')) {
                parse_close_file_statement(&tokens[2..], line.line_number)
            } else {
                Err(BBCBasicError::SyntaxError {
                    message: "CLOSE requires # (use CLOSE#)".to_string(),
                    line: line.line_number,
                })
            }
        }

        // BPUT# statement (file I/O)
        Token::Keyword(0xD5) if matches!(tokens.get(1), Some(Token::Operator('#'))) => {
            parse_bput_statement(&tokens[2..], line.line_number)
        }

        // PTR# assignment (file I/O). The tokenizer gives PTR as 0x8F.
        Token::Keyword(0x8F | 0xCF) if matches!(tokens.get(1), Some(Token::Operator('#'))) => {
            parse_set_ptr_statement(&tokens[2..], line.line_number)
        }

        // Graphics statements
        // PLOT statement
        Token::Keyword(0xF0) => parse_plot_statement(&tokens[1..], line.line_number),

        // DRAW statement
        Token::Keyword(0xDF) => parse_draw_statement(&tokens[1..], line.line_number),

        // MOVE statement
        Token::Keyword(0xEC) => parse_move_statement(&tokens[1..], line.line_number),

        // GCOL statement
        Token::Keyword(0xE6) => parse_gcol_statement(&tokens[1..], line.line_number),

        // CLG statement
        Token::Keyword(0xDA) => Ok(Statement::Clg),

        // MODE statement
        Token::Keyword(0xEB) => parse_mode_statement(&tokens[1..], line.line_number),

        // COLOUR statement
        Token::Keyword(0xFB) => parse_colour_statement(&tokens[1..], line.line_number),

        // VDU statement
        Token::Keyword(0xEF) => parse_vdu_statement(&tokens[1..], line.line_number),

        // Extended statements (0xC8 prefix)
        Token::ExtendedKeyword(0xC8, extended_token) => match extended_token {
            // WHILE statement
            0x95 => parse_while_statement(&tokens[1..], line.line_number),
            // ENDWHILE statement
            0xA4 => Ok(Statement::EndWhile),
            // ENDIF that is not at the start of a line: it does nothing, and
            // the block IF scan does not count it (BBC BASIC V)
            0xA5 => Ok(Statement::EndIf),
            // CASE expr OF
            0x8E => parse_case_statement(&tokens[1..], line.line_number),
            // CIRCLE statement
            0x8F => parse_circle_statement(&tokens[1..], line.line_number),
            // FILL statement
            0x90 => parse_fill_statement(&tokens[1..], line.line_number),
            // ORIGIN statement
            0x91 => parse_origin_statement(&tokens[1..], line.line_number),
            // RECTANGLE statement
            0x93 => parse_rectangle_statement(&tokens[1..], line.line_number),
            // STEREO statement
            0xA2 => parse_stereo_statement(&tokens[1..], line.line_number),
            // ELLIPSE statement
            0x9D => parse_ellipse_statement(&tokens[1..], line.line_number),
            // QUIT statement
            0x98 => Ok(Statement::Quit),
            // SWAP statement
            0x94 => parse_swap_statement(&tokens[1..], line.line_number),
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown extended statement: {}", tokens_text(&tokens[..1])),
                line: line.line_number,
            }),
        },

        _ => Err(BBCBasicError::SyntaxError {
            message: format!("Unknown statement: {}", tokens_text(&tokens[..1])),
            line: line.line_number,
        }),
    }
}

/// Parse PRINT statement
fn parse_print_statement(tokens: &[Token]) -> Result<Statement> {
    Ok(Statement::Print {
        items: parse_print_items(tokens)?,
    })
}

/// Read the items of a PRINT or PRINT# statement
fn parse_print_items(tokens: &[Token]) -> Result<Vec<PrintItem>> {
    let mut items = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        match &tokens[pos] {
            Token::Separator(';') => {
                items.push(PrintItem::Semicolon);
                pos += 1;
            }
            Token::Separator(',') => {
                items.push(PrintItem::Comma);
                pos += 1;
            }
            Token::Separator('\'') => {
                items.push(PrintItem::NewLine);
                pos += 1;
            }
            Token::Operator('~') => {
                items.push(PrintItem::Hex);
                pos += 1;
            }
            // Handle TAB(expr)
            Token::Keyword(0x8A) => {
                pos += 1; // skip TAB keyword
                if pos >= tokens.len() || !matches!(tokens[pos], Token::Separator('(')) {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected '(' after TAB".to_string(),
                        line: None,
                    });
                }
                pos += 1; // skip '('

                // Find matching ')'
                let start_pos = pos;
                let mut paren_depth = 1;
                while pos < tokens.len() && paren_depth > 0 {
                    match &tokens[pos] {
                        Token::Separator('(') => paren_depth += 1,
                        Token::Separator(')') => paren_depth -= 1,
                        _ => {}
                    }
                    pos += 1;
                }

                if paren_depth != 0 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Unmatched parentheses in TAB".to_string(),
                        line: None,
                    });
                }

                // TAB(x) or TAB(x, y): split the values at the commas
                // outside the inner brackets
                let values = split_at_commas(&tokens[start_pos..pos - 1]);
                match values.as_slice() {
                    [x] => items.push(PrintItem::Tab(parse_expression(x)?)),
                    [x, y] => {
                        items.push(PrintItem::TabXY(parse_expression(x)?, parse_expression(y)?))
                    }
                    _ => {
                        return Err(BBCBasicError::SyntaxError {
                            message: "TAB takes 1 or 2 values".to_string(),
                            line: None,
                        })
                    }
                }
            }
            // Handle SPC(expr)
            Token::Keyword(0x89) => {
                pos += 1; // skip SPC keyword
                if pos >= tokens.len() || !matches!(tokens[pos], Token::Separator('(')) {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected '(' after SPC".to_string(),
                        line: None,
                    });
                }
                pos += 1; // skip '('

                // Find matching ')'
                let start_pos = pos;
                let mut paren_depth = 1;
                while pos < tokens.len() && paren_depth > 0 {
                    match &tokens[pos] {
                        Token::Separator('(') => paren_depth += 1,
                        Token::Separator(')') => paren_depth -= 1,
                        _ => {}
                    }
                    pos += 1;
                }

                if paren_depth != 0 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Unmatched parentheses in SPC".to_string(),
                        line: None,
                    });
                }

                let expr = parse_expression(&tokens[start_pos..pos - 1])?;
                items.push(PrintItem::Spc(expr));
            }
            _ => {
                // Parse an expression
                let start_pos = pos;
                let mut end_pos = pos;
                let mut paren_depth = 0;

                // Find end of expression (stop at separator or end, but respect parentheses)
                while end_pos < tokens.len() {
                    match &tokens[end_pos] {
                        Token::Separator('(') => {
                            paren_depth += 1;
                            end_pos += 1;
                        }
                        Token::Separator(')') => {
                            paren_depth -= 1;
                            end_pos += 1;
                        }
                        Token::Separator(';' | ',' | '\'') if paren_depth == 0 => {
                            break;
                        }
                        _ => {
                            end_pos += 1;
                        }
                    }
                }

                if end_pos > start_pos {
                    let item_tokens = &tokens[start_pos..end_pos];
                    let mut used = 0;
                    let expr = parse_expr_precedence(item_tokens, &mut used, 0)?;
                    items.push(PrintItem::Expression(expr));
                    // A new value with no separator before it is the next item (BBC rule)
                    pos = if used < item_tokens.len() && starts_print_item(&item_tokens[used]) {
                        start_pos + used
                    } else {
                        end_pos
                    };
                } else {
                    break;
                }
            }
        }
    }

    Ok(items)
}

/// True if this token can start a PRINT item after a complete expression.
/// Binary operator keywords (AND, DIV, EOR, MOD, OR) continue the expression.
fn starts_print_item(token: &Token) -> bool {
    match token {
        Token::Integer(_)
        | Token::Real(_)
        | Token::String(_)
        | Token::Identifier(_)
        | Token::ExtendedKeyword(_, _)
        | Token::Separator('(')
        | Token::Operator('~') => true,
        Token::Keyword(code) => get_keyword_precedence(*code).is_none(),
        _ => false,
    }
}

/// Parse assignment statement (A% = 42 or LET A% = 42, or array assignment like arr(i) = 5)
fn parse_assignment(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if Indirection::from_operator(tokens.first()).is_some()
        || Indirection::from_operator(tokens.get(1)).is_some()
    {
        return parse_memory_assignment(tokens, line_number);
    }

    if tokens.len() < 3 {
        return Err(BBCBasicError::SyntaxError {
            message: "Invalid assignment".to_string(),
            line: line_number,
        });
    }

    let target = match &tokens[0] {
        Token::Identifier(name) => name.clone(),
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected variable name".to_string(),
                line: line_number,
            })
        }
    };

    // Check if this is an array element assignment: array_name(index) = value
    if tokens.len() > 1 && matches!(tokens[1], Token::Separator('(')) {
        // Find the closing parenthesis
        let mut paren_depth = 0;
        let mut close_paren_pos = None;
        for (i, token) in tokens.iter().enumerate() {
            if matches!(token, Token::Separator('(')) {
                paren_depth += 1;
            } else if matches!(token, Token::Separator(')')) {
                paren_depth -= 1;
                if paren_depth == 0 {
                    close_paren_pos = Some(i);
                    break;
                }
            }
        }

        let close_paren_pos = close_paren_pos.ok_or(BBCBasicError::SyntaxError {
            message: "Expected ')' in array assignment".to_string(),
            line: line_number,
        })?;

        // Parse indices between parentheses
        let indices_tokens = &tokens[2..close_paren_pos];
        let indices = if indices_tokens.is_empty() {
            Vec::new()
        } else {
            parse_comma_separated_expressions(indices_tokens, line_number)?
        };

        // An array element as the base of a memory write: C%(1)?2 = 5
        if Indirection::from_operator(tokens.get(close_paren_pos + 1)).is_some() {
            return parse_memory_assignment(tokens, line_number);
        }

        // After closing paren, expect '='
        if close_paren_pos + 1 >= tokens.len()
            || !matches!(tokens[close_paren_pos + 1], Token::Operator('='))
        {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected '=' after array indices".to_string(),
                line: line_number,
            });
        }

        // Parse the value expression
        let expression = parse_expression(&tokens[close_paren_pos + 2..])?;

        return Ok(Statement::ArrayAssignment {
            name: target,
            indices,
            expression,
        });
    }

    // Simple variable assignment
    if !matches!(tokens[1], Token::Operator('=')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected '='".to_string(),
            line: line_number,
        });
    }

    let expression = parse_expression(&tokens[2..])?;

    Ok(Statement::Assignment { target, expression })
}

/// Parse a memory write such as `?A = 5` or `A!4 = 5`. The left side is one
/// primary that must be a memory read, and `=` must follow it.
fn parse_memory_assignment(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let mut pos = 0;
    let target = parse_primary(tokens, &mut pos)?;
    let Expression::UnaryOp {
        op: UnaryOperator::Indirect(width),
        operand,
    } = target
    else {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected '?' or '!'".to_string(),
            line: line_number,
        });
    };
    if !matches!(tokens.get(pos), Some(Token::Operator('='))) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected '='".to_string(),
            line: line_number,
        });
    }
    let expression = parse_expression(&tokens[pos + 1..])?;
    Ok(Statement::MemoryAssignment {
        width,
        address: *operand,
        expression,
    })
}

/// Parse FOR statement
fn parse_for_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // FOR EACH variable IN collection. EACH with `=` after it is the name
    // of the variable of a normal FOR.
    if matches!(tokens.first(), Some(Token::Identifier(word)) if word == "EACH")
        && !matches!(tokens.get(1), Some(Token::Operator('=')))
    {
        return parse_for_each_statement(&tokens[1..], line_number);
    }

    // FOR variable = start TO end [STEP step]
    if tokens.len() < 5 {
        return Err(BBCBasicError::SyntaxError {
            message: "Invalid FOR statement".to_string(),
            line: line_number,
        });
    }

    let variable = match &tokens[0] {
        Token::Identifier(name) => name.clone(),
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected variable name after FOR".to_string(),
                line: line_number,
            })
        }
    };

    if !matches!(tokens[1], Token::Operator('=')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected '=' in FOR statement".to_string(),
            line: line_number,
        });
    }

    // Find TO keyword
    let to_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Keyword(0xB8)))
        .ok_or(BBCBasicError::SyntaxError {
            message: "Expected TO in FOR statement".to_string(),
            line: line_number,
        })?;

    let start = parse_expression(&tokens[2..to_pos])?;

    // Check for STEP keyword
    let step_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Keyword(0x88)));

    let (end, step) = if let Some(step_pos) = step_pos {
        let end = parse_expression(&tokens[to_pos + 1..step_pos])?;
        let step = parse_expression(&tokens[step_pos + 1..])?;
        (end, Some(step))
    } else {
        let end = parse_expression(&tokens[to_pos + 1..])?;
        (end, None)
    };

    Ok(Statement::For {
        variable,
        start,
        end,
        step,
    })
}

/// Parse the tokens after `FOR EACH`: `variable IN collection` (issue #141).
/// EACH and IN are words of this statement only, not keywords.
fn parse_for_each_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let syntax_error = |message: &str| BBCBasicError::SyntaxError {
        message: message.to_string(),
        line: line_number,
    };
    let Some(Token::Identifier(variable)) = tokens.first() else {
        return Err(syntax_error("Expected variable name after FOR EACH"));
    };
    if !matches!(tokens.get(1), Some(Token::Identifier(word)) if word == "IN") {
        return Err(syntax_error("Expected IN in FOR EACH statement"));
    }
    if tokens.len() < 3 {
        return Err(syntax_error("Expected a collection after IN"));
    }
    Ok(Statement::ForEach {
        variable: variable.clone(),
        collection: parse_expression(&tokens[2..])?,
    })
}

/// Parse NEXT statement
fn parse_next_statement(tokens: &[Token]) -> Result<Statement> {
    let mut variables = Vec::new();

    if tokens.is_empty() {
        // NEXT without variable
        return Ok(Statement::Next { variables });
    }

    // Parse variable list (comma-separated)
    let mut pos = 0;
    while pos < tokens.len() {
        match &tokens[pos] {
            Token::Identifier(name) => {
                variables.push(name.clone());
                pos += 1;

                if pos < tokens.len() && matches!(tokens[pos], Token::Separator(',')) {
                    pos += 1; // skip comma
                }
            }
            _ => break,
        }
    }

    Ok(Statement::Next { variables })
}

/// The target of a GOTO or a GOSUB
enum JumpTarget {
    Line(u16),
    Calculated(Expression),
}

/// Parse the target after GOTO or GOSUB. A number constant must be the
/// only token. A target that starts with a different token is an
/// expression, as in BBC BASIC: `GOTO (100 + X)` or `GOSUB X * 10`.
fn parse_jump_target(
    tokens: &[Token],
    keyword: &str,
    line_number: Option<u16>,
) -> Result<JumpTarget> {
    let syntax_error = |message: String| BBCBasicError::SyntaxError {
        message,
        line: line_number,
    };
    match tokens {
        [] => Err(syntax_error(format!(
            "Expected line number after {}",
            keyword
        ))),
        [Token::Integer(n)] => Ok(JumpTarget::Line(literal_line_number(*n, line_number)?)),
        [Token::Integer(_), ..] => Err(syntax_error(format!(
            "Expected end of statement after {} line number",
            keyword
        ))),
        _ => {
            let mut pos = 0;
            let target = parse_expr_precedence(tokens, &mut pos, 0)
                .map_err(|_| syntax_error(format!("Expected line number after {}", keyword)))?;
            if pos != tokens.len() {
                return Err(syntax_error(format!(
                    "Expected end of statement after {} target",
                    keyword
                )));
            }
            Ok(JumpTarget::Calculated(target))
        }
    }
}

/// Parse GOTO statement
fn parse_goto_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    Ok(match parse_jump_target(tokens, "GOTO", line_number)? {
        JumpTarget::Line(line_number) => Statement::Goto { line_number },
        JumpTarget::Calculated(target) => Statement::GotoExpression { target },
    })
}

/// Parse GOSUB statement
fn parse_gosub_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    Ok(match parse_jump_target(tokens, "GOSUB", line_number)? {
        JumpTarget::Line(line_number) => Statement::Gosub { line_number },
        JumpTarget::Calculated(target) => Statement::GosubExpression { target },
    })
}

/// The error for ON ERROR with nothing to run after it
const NO_ON_ERROR_STATEMENT: &str = "Expected GOTO, OFF or a statement after ON ERROR";

/// A syntax error with this message at this line
fn syntax_error_at(message: &str, line_number: Option<u16>) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: message.to_string(),
        line: line_number,
    }
}

/// A literal line number after a keyword. It must be from 0 to 65535: a
/// larger number must not change to a different line.
fn literal_line_number(value: i32, line_number: Option<u16>) -> Result<u16> {
    u16::try_from(value).map_err(|_| syntax_error_at("Line number out of range", line_number))
}

/// Parse the handler statements after ON ERROR. The handler runs from one
/// line only, so it must have a statement and cannot open a block IF or
/// define a PROC or FN.
fn parse_handler_statements(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<Statement>> {
    let statements = parse_statements(tokens, line_number)?;
    if statements.iter().all(|s| matches!(s, Statement::Empty)) {
        return Err(syntax_error_at(NO_ON_ERROR_STATEMENT, line_number));
    }
    if let Some(Statement::IfBlock { .. }) = statements.last() {
        return Err(syntax_error_at(
            "Block IF cannot be in an ON ERROR handler",
            line_number,
        ));
    }
    if has_def(&statements) {
        return Err(syntax_error_at(
            "DEF cannot be in an ON ERROR handler",
            line_number,
        ));
    }
    Ok(statements)
}

/// True if a statement or a statement in an IF branch is a DEF PROC or DEF FN
fn has_def(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::DefProc { .. } | Statement::DefFn { .. } => true,
        Statement::If {
            then_part,
            else_part,
            ..
        } => has_def(then_part) || else_part.as_deref().is_some_and(has_def),
        _ => false,
    })
}

/// Parse ON statement (ON GOTO or ON GOSUB)
fn parse_on_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Syntax: ON <expression> GOTO|GOSUB <line1>, <line2>, ...
    // or: ON ERROR GOTO <line>
    // or: ON ERROR OFF
    // or: ON ERROR <statements>

    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected expression after ON".to_string(),
            line: line_number,
        });
    }

    // Check for ON ERROR variant
    if matches!(tokens[0], Token::Keyword(0x85)) {
        // ERROR keyword (0x85)
        if tokens.len() < 2 {
            return Err(syntax_error_at(NO_ON_ERROR_STATEMENT, line_number));
        }

        match tokens[1] {
            Token::Keyword(0x87) => {
                // OFF keyword (0x87)
                return Ok(Statement::OnErrorOff);
            }
            Token::Keyword(0xE5) => {
                // GOTO keyword (0xE5)
                if tokens.len() < 3 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected line number after ON ERROR GOTO".to_string(),
                        line: line_number,
                    });
                }
                match tokens[2] {
                    Token::Integer(n) | Token::HexInteger(n) => {
                        return Ok(Statement::OnError {
                            line_number: literal_line_number(n, line_number)?,
                        });
                    }
                    _ => {
                        return Err(BBCBasicError::SyntaxError {
                            message: "Expected line number after ON ERROR GOTO".to_string(),
                            line: line_number,
                        });
                    }
                }
            }
            Token::Keyword(0xEA) => {
                // LOCAL keyword (0xEA)
                return Err(BBCBasicError::SyntaxError {
                    message: "ON ERROR LOCAL is not supported".to_string(),
                    line: line_number,
                });
            }
            _ => {
                // The handler is the rest of the line
                return Ok(Statement::OnErrorStatements {
                    statements: parse_handler_statements(&tokens[1..], line_number)?,
                });
            }
        }
    }

    // Find GOTO or GOSUB keyword
    let mut goto_pos = None;
    let mut gosub_pos = None;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::Keyword(0xE5) => {
                // GOTO
                goto_pos = Some(i);
                break;
            }
            Token::Keyword(0xE4) => {
                // GOSUB
                gosub_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    let (keyword_pos, is_goto) = if let Some(pos) = goto_pos {
        (pos, true)
    } else if let Some(pos) = gosub_pos {
        (pos, false)
    } else {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected GOTO or GOSUB after ON expression".to_string(),
            line: line_number,
        });
    };

    // Parse expression before GOTO/GOSUB
    let expression = parse_expression(&tokens[..keyword_pos])?;

    // Parse line numbers after GOTO/GOSUB, up to an optional ELSE. The ELSE
    // part is the rest of the line (parse_statements gives it to the ON).
    let after_keyword = &tokens[keyword_pos + 1..];
    let (line_tokens, else_part) = match after_keyword
        .iter()
        .position(|t| matches!(t, Token::Keyword(0x8B)))
    {
        Some(else_pos) => {
            let else_tokens = &after_keyword[else_pos + 1..];
            if else_tokens.is_empty() {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected statement after ELSE".to_string(),
                    line: line_number,
                });
            }
            (
                &after_keyword[..else_pos],
                Some(parse_if_branch(else_tokens, line_number)?),
            )
        }
        None => (after_keyword, None),
    };
    if line_tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected line numbers after ON GOTO/GOSUB".to_string(),
            line: line_number,
        });
    }

    let mut targets = Vec::new();
    let mut pos = 0;

    while pos < line_tokens.len() {
        match &line_tokens[pos] {
            Token::Integer(n) | Token::HexInteger(n) => {
                targets.push(literal_line_number(*n, line_number)?);
                pos += 1;

                // Skip comma if present
                if pos < line_tokens.len() && matches!(line_tokens[pos], Token::Separator(',')) {
                    pos += 1;
                }
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected line number in ON GOTO/GOSUB list".to_string(),
                    line: line_number,
                });
            }
        }
    }

    if targets.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected at least one line number in ON GOTO/GOSUB".to_string(),
            line: line_number,
        });
    }

    if is_goto {
        Ok(Statement::OnGoto {
            expression,
            targets,
            else_part,
        })
    } else {
        Ok(Statement::OnGosub {
            expression,
            targets,
            else_part,
        })
    }
}

/// Parse INPUT statement
fn parse_input_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    Ok(Statement::Input {
        items: parse_input_items(tokens, line_number)?,
    })
}

/// Parse the item list of INPUT or INPUT LINE: prompt texts, variables and
/// array elements. A comma or a semicolon after a prompt text gives `?` after
/// the text.
fn parse_input_items(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<InputItem>> {
    let mut items = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        let item = match &tokens[pos] {
            Token::String(text) => InputItem::Prompt {
                text: text.clone(),
                question_mark: matches!(tokens.get(pos + 1), Some(Token::Separator(',' | ';'))),
            },
            Token::Identifier(name)
                if matches!(tokens.get(pos + 1), Some(Token::Separator('('))) =>
            {
                let close =
                    find_closing_bracket(tokens, pos + 1).ok_or(BBCBasicError::SyntaxError {
                        message: "Expected ')' in INPUT".to_string(),
                        line: line_number,
                    })?;
                let indices =
                    parse_comma_separated_expressions(&tokens[pos + 2..close], line_number)?;
                pos = close;
                InputItem::ArrayElement {
                    name: name.clone(),
                    indices,
                }
            }
            Token::Identifier(name) => InputItem::Variable(name.clone()),
            Token::Separator('\'') => InputItem::NewLine,
            _ => break,
        };
        items.push(item);
        pos += 1;

        if matches!(tokens.get(pos), Some(Token::Separator(',' | ';'))) {
            pos += 1; // skip the separator
        }
    }

    Ok(items)
}

/// Find the bracket that closes the bracket at `open`
fn find_closing_bracket(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate().skip(open) {
        match token {
            Token::Separator('(') => depth += 1,
            Token::Separator(')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse PRINT# statement (file I/O)
fn parse_print_file_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // First token should be the file handle expression
    // Format: PRINT# handle, items...

    // Find the comma that separates handle from print items
    let comma_pos = first_comma(tokens);

    if comma_pos.is_none() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected comma after file handle in PRINT#".to_string(),
            line: line_number,
        });
    }

    let comma_pos = comma_pos.unwrap();

    // Parse handle expression
    let handle = parse_expression(&tokens[..comma_pos])?;

    // Parse print items after the comma
    let items = if comma_pos + 1 < tokens.len() {
        parse_print_items(&tokens[comma_pos + 1..])?
    } else {
        Vec::new()
    };

    // BBC BASIC writes only values to a file, and has no column for TAB or SPC
    // there, so it does not permit them in PRINT#
    if items.iter().any(|item| {
        matches!(
            item,
            PrintItem::Tab(_) | PrintItem::TabXY(..) | PrintItem::Spc(_)
        )
    }) {
        return Err(BBCBasicError::SyntaxError {
            message: "TAB and SPC are not permitted in PRINT#".to_string(),
            line: line_number,
        });
    }
    // A file has no new line item either (issue 159)
    if items.contains(&PrintItem::NewLine) {
        return Err(BBCBasicError::SyntaxError {
            message: "' is not permitted in PRINT#".to_string(),
            line: line_number,
        });
    }

    Ok(Statement::PrintFile { handle, items })
}

/// Parse INPUT# statement (file I/O)
fn parse_input_file_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Format: INPUT# handle, var1, var2, ...

    // Find the comma that separates handle from variables
    let comma_pos = first_comma(tokens);

    if comma_pos.is_none() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected comma after file handle in INPUT#".to_string(),
            line: line_number,
        });
    }

    let comma_pos = comma_pos.unwrap();

    // Parse handle expression
    let handle = parse_expression(&tokens[..comma_pos])?;

    if comma_pos + 1 == tokens.len() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected at least one variable in INPUT#".to_string(),
            line: line_number,
        });
    }

    // Each part between the commas is one target (issue #154)
    let targets = split_at_commas(&tokens[comma_pos + 1..])
        .into_iter()
        .map(|part| parse_input_file_target(part, line_number))
        .collect::<Result<Vec<_>>>()?;

    Ok(Statement::InputFile { handle, targets })
}

/// Parse one INPUT# target: a variable or an array element, in the target
/// form of INPUT. Other tokens give a syntax error (issue #154).
fn parse_input_file_target(part: &[Token], line_number: Option<u16>) -> Result<InputItem> {
    let is_target = match part {
        [Token::Identifier(_)] => true,
        [Token::Identifier(_), Token::Separator('('), ..] => {
            find_closing_bracket(part, 1) == Some(part.len() - 1)
        }
        _ => false,
    };
    if !is_target {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected a variable or an array element in INPUT#".to_string(),
            line: line_number,
        });
    }
    let mut items = parse_input_items(part, line_number)?;
    Ok(items.remove(0))
}

/// Parse CLOSE# statement (file I/O)
fn parse_close_file_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Format: CLOSE# handle

    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected file handle after CLOSE#".to_string(),
            line: line_number,
        });
    }

    // Parse handle expression
    let handle = parse_expression(tokens)?;

    Ok(Statement::CloseFile { handle })
}

/// Parse BPUT# statement (file I/O)
fn parse_bput_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Format: BPUT# handle, value
    let comma_pos = first_comma(tokens).ok_or_else(|| BBCBasicError::SyntaxError {
        message: "Expected comma after file handle in BPUT#".to_string(),
        line: line_number,
    })?;

    let handle = parse_expression(&tokens[..comma_pos])?;
    let value = parse_expression(&tokens[comma_pos + 1..])?;

    Ok(Statement::Bput { handle, value })
}

/// Parse SAVEJSON file$, json$ (extension, issue #102)
fn parse_savejson_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // The comma between the two texts is the first comma outside brackets
    let comma_pos = first_comma(tokens).ok_or_else(|| BBCBasicError::SyntaxError {
        message: "Expected comma after file name in SAVEJSON".to_string(),
        line: line_number,
    })?;

    let file = parse_expression(&tokens[..comma_pos])?;
    let json = parse_expression(&tokens[comma_pos + 1..])?;

    Ok(Statement::SaveJson { file, json })
}

/// Tell if a name is a list statement (extension, issue #139) or a map
/// statement (extension, issue #140). LISTADDCOL and MAPPUTCOL are from
/// issue #142.
fn is_list_statement(name: &str) -> bool {
    matches!(
        name,
        "LISTADD"
            | "LISTSET"
            | "LISTINSERT"
            | "LISTREMOVE"
            | "LISTCLEAR"
            | "MAPPUT"
            | "MAPREMOVE"
            | "MAPCLEAR"
            | "LISTADDCOL"
            | "MAPPUTCOL"
    )
}

/// Parse a list statement: LISTADD l, value, LISTSET l, i, value,
/// LISTINSERT l, i, value, LISTREMOVE l, i or LISTCLEAR l (issue #139).
/// Parse a map statement: MAPPUT m, key$, value, MAPREMOVE m, key$ or
/// MAPCLEAR m (issue #140). A comma in brackets does not divide the arguments.
fn parse_list_statement(
    name: &str,
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Statement> {
    let args = split_at_commas(tokens)
        .into_iter()
        .map(parse_expression)
        .collect::<Result<Vec<_>>>()?;

    let count = match name {
        "LISTCLEAR" | "MAPCLEAR" => 1,
        "LISTADD" | "LISTREMOVE" | "MAPREMOVE" | "LISTADDCOL" => 2,
        _ => 3,
    };
    if args.len() != count {
        return Err(BBCBasicError::SyntaxError {
            message: format!("{} requires {} arguments, got {}", name, count, args.len()),
            line: line_number,
        });
    }

    let mut args = args.into_iter();
    let list = args.next().expect("count is checked");
    let mut next = || args.next().expect("count is checked");
    Ok(match name {
        "LISTADD" => Statement::ListAdd {
            list,
            value: next(),
        },
        "LISTSET" => Statement::ListSet {
            list,
            index: next(),
            value: next(),
        },
        "LISTINSERT" => Statement::ListInsert {
            list,
            index: next(),
            value: next(),
        },
        "LISTREMOVE" => Statement::ListRemove {
            list,
            index: next(),
        },
        "LISTCLEAR" => Statement::ListClear { list },
        "MAPPUT" => Statement::MapPut {
            map: list,
            key: next(),
            value: next(),
        },
        "MAPREMOVE" => Statement::MapRemove {
            map: list,
            key: next(),
        },
        "LISTADDCOL" => Statement::ListAddCollection {
            list,
            handle: next(),
        },
        "MAPPUTCOL" => Statement::MapPutCollection {
            map: list,
            key: next(),
            handle: next(),
        },
        _ => Statement::MapClear { map: list },
    })
}

/// Parse PTR# assignment (file I/O)
fn parse_set_ptr_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Format: PTR# handle = position
    let equals_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Operator('=')))
        .ok_or_else(|| BBCBasicError::SyntaxError {
            message: "Expected = after file handle in PTR#".to_string(),
            line: line_number,
        })?;

    let handle = parse_expression(&tokens[..equals_pos])?;
    let position = parse_expression(&tokens[equals_pos + 1..])?;

    Ok(Statement::SetPtr { handle, position })
}

/// Parse PLOT statement: PLOT mode, x, y
fn parse_plot_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "PLOT requires mode, x, y parameters".to_string(),
            line: line_number,
        });
    }

    // Parse comma-separated arguments: mode, x, y
    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 3 {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "PLOT requires 3 parameters (mode, x, y), got {}",
                args.len()
            ),
            line: line_number,
        });
    }

    Ok(Statement::Plot {
        mode: args[0].clone(),
        x: args[1].clone(),
        y: args[2].clone(),
    })
}

/// Parse MOVE statement: MOVE x, y
fn parse_move_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "MOVE requires x, y parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 2 {
        return Err(BBCBasicError::SyntaxError {
            message: format!("MOVE requires 2 parameters (x, y), got {}", args.len()),
            line: line_number,
        });
    }

    Ok(Statement::Move {
        x: args[0].clone(),
        y: args[1].clone(),
    })
}

/// Parse DRAW statement: DRAW x, y
fn parse_draw_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "DRAW requires x, y parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 2 {
        return Err(BBCBasicError::SyntaxError {
            message: format!("DRAW requires 2 parameters (x, y), got {}", args.len()),
            line: line_number,
        });
    }

    Ok(Statement::Draw {
        x: args[0].clone(),
        y: args[1].clone(),
    })
}

/// Remove the word FILL after CIRCLE, ELLIPSE or RECTANGLE. Give true and the
/// tokens after FILL, or false and all the tokens.
fn split_fill_word(tokens: &[Token]) -> (bool, &[Token]) {
    match tokens.first() {
        Some(Token::ExtendedKeyword(0xC8, 0x90)) => (true, &tokens[1..]),
        _ => (false, tokens),
    }
}

/// Parse CIRCLE statement: CIRCLE [FILL] x, y, radius
fn parse_circle_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let (filled, tokens) = split_fill_word(tokens);
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "CIRCLE requires x, y, radius parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 3 {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "CIRCLE requires 3 parameters (x, y, radius), got {}",
                args.len()
            ),
            line: line_number,
        });
    }

    Ok(Statement::Circle {
        x: args[0].clone(),
        y: args[1].clone(),
        radius: args[2].clone(),
        filled,
    })
}

/// Parse GCOL statement: GCOL mode, color
fn parse_gcol_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "GCOL requires mode, color parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 2 {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "GCOL requires 2 parameters (mode, color), got {}",
                args.len()
            ),
            line: line_number,
        });
    }

    Ok(Statement::Gcol {
        mode: args[0].clone(),
        color: args[1].clone(),
    })
}

/// Parse ELLIPSE statement: ELLIPSE [FILL] x, y, major, minor
fn parse_ellipse_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let (filled, tokens) = split_fill_word(tokens);
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "ELLIPSE requires x, y, major, minor parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 4 {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "ELLIPSE requires 4 parameters (x, y, major, minor), got {}",
                args.len()
            ),
            line: line_number,
        });
    }

    Ok(Statement::Ellipse {
        x: args[0].clone(),
        y: args[1].clone(),
        major: args[2].clone(),
        minor: args[3].clone(),
        filled,
    })
}

/// Parse RECTANGLE statement: RECTANGLE [FILL] x1, y1, width [, height]. The
/// BBC BASIC for Windows manual gives: "To draw a square you can omit the
/// fourth parameter; the height is then assumed to be the same as the width"
/// (https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin7.html).
fn parse_rectangle_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let (filled, tokens) = split_fill_word(tokens);
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "RECTANGLE requires x1, y1, width, height parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 3 && args.len() != 4 {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "RECTANGLE requires 3 or 4 parameters (x1, y1, width [, height]), got {}",
                args.len()
            ),
            line: line_number,
        });
    }

    Ok(Statement::Rectangle {
        x1: args[0].clone(),
        y1: args[1].clone(),
        width: args[2].clone(),
        height: args.get(3).unwrap_or(&args[2]).clone(),
        filled,
    })
}

/// Parse FILL statement: FILL x, y
fn parse_fill_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "FILL requires x, y parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 2 {
        return Err(BBCBasicError::SyntaxError {
            message: format!("FILL requires 2 parameters (x, y), got {}", args.len()),
            line: line_number,
        });
    }

    Ok(Statement::Fill {
        x: args[0].clone(),
        y: args[1].clone(),
    })
}

/// Parse ORIGIN statement
/// ORIGIN x, y - Set graphics origin
fn parse_origin_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "ORIGIN requires x, y parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 2 {
        return Err(BBCBasicError::SyntaxError {
            message: format!("ORIGIN requires 2 parameters (x, y), got {}", args.len()),
            line: line_number,
        });
    }

    Ok(Statement::Origin {
        x: args[0].clone(),
        y: args[1].clone(),
    })
}

/// Parse STEREO channel, position
fn parse_stereo_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let args = if tokens.is_empty() {
        Vec::new()
    } else {
        parse_comma_separated_expressions(tokens, line_number)?
    };

    match <[Expression; 2]>::try_from(args) {
        Ok([channel, position]) => Ok(Statement::Stereo { channel, position }),
        Err(args) => Err(BBCBasicError::SyntaxError {
            message: format!(
                "STEREO requires 2 parameters (channel, position), got {}",
                args.len()
            ),
            line: line_number,
        }),
    }
}

/// Split the tokens at each comma that is not in brackets (issue #150). A
/// comma in a string literal is in the string token, so it does not split.
fn split_at_commas(tokens: &[Token]) -> Vec<&[Token]> {
    let mut depth = 0;
    tokens
        .split(|token| {
            match token {
                Token::Separator('(') => depth += 1,
                Token::Separator(')') => depth -= 1,
                _ => {}
            }
            depth == 0 && matches!(token, Token::Separator(','))
        })
        .collect()
}

/// The position of the first comma that is not in brackets (issue #150).
fn first_comma(tokens: &[Token]) -> Option<usize> {
    let first = split_at_commas(tokens)[0].len();
    (first < tokens.len()).then_some(first)
}

/// Parse the expressions between the commas that are not in brackets. An
/// empty part (2 commas together, or a comma at the start or at the end)
/// gives a syntax error (issue #155).
fn parse_comma_separated_expressions(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Vec<Expression>> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected at least one expression".to_string(),
            line: line_number,
        });
    }

    split_at_commas(tokens)
        .into_iter()
        .map(|part| {
            if part.is_empty() {
                Err(BBCBasicError::SyntaxError {
                    message: "Missing value at a comma".to_string(),
                    line: line_number,
                })
            } else {
                parse_expression(part)
            }
        })
        .collect()
}

/// The error for an empty part in a list of values (issue #163).
fn missing_value_at_a_comma(line_number: Option<u16>) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: "Missing value at a comma".to_string(),
        line: line_number,
    }
}

/// Parse DIM statement
fn parse_dim_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let mut items = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        // Get array name (without the opening paren suffix if present)
        let name = match &tokens[pos] {
            Token::Identifier(n) => {
                // Remove trailing '(' if present (some tokenizers include it)
                n.trim_end_matches('(').to_string()
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected array name in DIM".to_string(),
                    line: line_number,
                })
            }
        };
        pos += 1;

        if pos >= tokens.len() {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected '(' after array name".to_string(),
                line: line_number,
            });
        }

        // A name with no bracket reserves a block: the size runs to the
        // next comma outside brackets
        if !matches!(tokens[pos], Token::Separator('(')) {
            let start = pos;
            let mut depth = 0;
            while pos < tokens.len() {
                match &tokens[pos] {
                    Token::Separator('(') => depth += 1,
                    Token::Separator(')') => depth -= 1,
                    Token::Separator(',') if depth == 0 => break,
                    _ => {}
                }
                pos += 1;
            }
            let size = parse_expression(&tokens[start..pos])?;
            items.push(DimItem::Block { name, size });
            if pos < tokens.len() {
                pos += 1; // the comma before the next item
                if pos >= tokens.len() || matches!(tokens[pos], Token::Separator(',')) {
                    return Err(missing_value_at_a_comma(line_number));
                }
                continue;
            }
            break;
        }
        pos += 1;

        // The sizes run to the closing bracket of this array
        let start = pos;
        let mut depth = 0;
        while pos < tokens.len() {
            match &tokens[pos] {
                Token::Separator('(') => depth += 1,
                Token::Separator(')') if depth == 0 => break,
                Token::Separator(')') => depth -= 1,
                _ => {}
            }
            pos += 1;
        }

        // An empty size gives a syntax error (issue #163)
        let mut dimensions = Vec::new();
        if pos > start {
            for part in split_at_commas(&tokens[start..pos]) {
                if part.is_empty() {
                    return Err(missing_value_at_a_comma(line_number));
                }
                dimensions.push(parse_expression(part)?);
            }
        }
        if pos < tokens.len() {
            pos += 1; // end of this array
        }

        items.push(DimItem::Array { name, dimensions });

        // Only a comma or the end of the statement can follow an array
        // (issue #177)
        match tokens.get(pos) {
            None => break,
            Some(Token::Separator(',')) => {
                pos += 1;
                if pos >= tokens.len() || matches!(tokens[pos], Token::Separator(',')) {
                    return Err(missing_value_at_a_comma(line_number));
                }
            }
            Some(token) => {
                return Err(BBCBasicError::SyntaxError {
                    message: format!(
                        "Unexpected {} after DIM item",
                        tokens_text(std::slice::from_ref(token))
                    ),
                    line: line_number,
                })
            }
        }
    }

    Ok(Statement::Dim { items })
}

/// Parse DATA statement
///
/// DATA uses the rest of its line, so the tokenizer gives that text as one
/// token. A comma separates the items. An item in double quotes is the text
/// in the quotes, and a doubled quote is one quote. An item with no quotes is
/// the text with leading spaces removed. Each item is kept as text: a READ
/// into a numeric variable converts it.
fn parse_data_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let text = match tokens {
        [] => return Ok(Statement::Data { values: Vec::new() }),
        [Token::DataText(text)] => text,
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: format!("Invalid DATA value: {}", tokens_text(tokens)),
                line: line_number,
            });
        }
    };

    let mut values = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.next_if_eq(&' ').is_some() {}
        let mut item = String::new();
        if chars.next_if_eq(&'"').is_some() {
            while let Some(ch) = chars.next() {
                if ch == '"' && chars.next_if_eq(&'"').is_none() {
                    break;
                }
                item.push(ch);
            }
            // Text after the closing quote is not part of the item
            while chars.next_if(|c| *c != ',').is_some() {}
        } else {
            while let Some(ch) = chars.next_if(|c| *c != ',') {
                item.push(ch);
            }
        }
        values.push(DataValue::String(item));
        // Consume the comma. If there is no comma, the items are complete.
        if chars.next().is_none() {
            break;
        }
    }

    Ok(Statement::Data { values })
}

/// Parse READ statement
/// Supports: READ var1, A(I), N$(1, 2), ... (issue #153). The targets are
/// split at the commas that are not in brackets. An empty part is a syntax
/// error, as in a list of expressions (issue #155).
fn parse_read_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "READ requires at least one variable".to_string(),
            line: line_number,
        });
    }

    let variables = split_at_commas(tokens)
        .into_iter()
        .map(|part| {
            if part.is_empty() {
                Err(BBCBasicError::SyntaxError {
                    message: "Missing value at a comma".to_string(),
                    line: line_number,
                })
            } else {
                parse_read_target(part, line_number)
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Statement::Read { variables })
}

/// Parse one READ target: a variable, or an array element with the index
/// form of INPUT. Another form gives a syntax error.
fn parse_read_target(tokens: &[Token], line_number: Option<u16>) -> Result<InputItem> {
    match tokens {
        [Token::Identifier(name)] => Ok(InputItem::Variable(name.clone())),
        [Token::Identifier(name), Token::Separator('('), ..]
            if find_closing_bracket(tokens, 1) == Some(tokens.len() - 1) =>
        {
            Ok(InputItem::ArrayElement {
                name: name.clone(),
                indices: parse_comma_separated_expressions(
                    &tokens[2..tokens.len() - 1],
                    line_number,
                )?,
            })
        }
        _ => Err(BBCBasicError::SyntaxError {
            message: "Expected variable name in READ".to_string(),
            line: line_number,
        }),
    }
}

/// Parse SWAP statement (issue #162)
/// Supports: SWAP A, B and SWAP A(I), A(J). The 2 targets have the form of
/// a READ target. A whole array, `A()`, gives a syntax error.
fn parse_swap_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let parts = split_at_commas(tokens);
    let [first, second] = parts.as_slice() else {
        return Err(BBCBasicError::SyntaxError {
            message: "SWAP requires 2 variables".to_string(),
            line: line_number,
        });
    };
    Ok(Statement::Swap {
        first: parse_swap_target(first, line_number)?,
        second: parse_swap_target(second, line_number)?,
    })
}

/// Parse one SWAP target: a variable, or an array element with one index or
/// more. Another form gives a syntax error.
fn parse_swap_target(tokens: &[Token], line_number: Option<u16>) -> Result<InputItem> {
    let is_variable = matches!(tokens, [Token::Identifier(_)]);
    let is_element = matches!(tokens, [Token::Identifier(_), Token::Separator('('), ..])
        && tokens.len() > 3
        && find_closing_bracket(tokens, 1) == Some(tokens.len() - 1);
    if is_variable || is_element {
        parse_read_target(tokens, line_number)
    } else {
        Err(BBCBasicError::SyntaxError {
            message: "Expected variable name in SWAP".to_string(),
            line: line_number,
        })
    }
}

/// Parse RESTORE statement
/// Supports: RESTORE [line_number]
fn parse_restore_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        // RESTORE with no line number - reset to beginning
        Ok(Statement::Restore { line_number: None })
    } else if tokens.len() == 1 {
        // RESTORE with line number
        match &tokens[0] {
            Token::Integer(num) | Token::HexInteger(num) => Ok(Statement::Restore {
                line_number: Some(literal_line_number(*num, line_number)?),
            }),
            Token::LineNumber(num) => Ok(Statement::Restore {
                line_number: Some(*num),
            }),
            _ => Err(BBCBasicError::SyntaxError {
                message: "RESTORE expects line number".to_string(),
                line: None,
            }),
        }
    } else {
        Err(BBCBasicError::SyntaxError {
            message: "RESTORE expects at most one line number".to_string(),
            line: None,
        })
    }
}

/// Parse UNTIL statement
/// Supports: UNTIL condition
fn parse_until_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "UNTIL requires a condition".to_string(),
            line: line_number,
        });
    }

    // Parse the condition expression
    let condition = parse_expression(tokens)?;
    Ok(Statement::Until { condition })
}

/// Parse MODE statement
/// Supports: MODE n, where n is an expression
fn parse_mode_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "MODE requires a mode number".to_string(),
            line: line_number,
        });
    }

    let mode = parse_expression(tokens)?;
    Ok(Statement::Mode { mode })
}

/// Parse COLOUR statement: COLOUR n, where n is an expression. The palette
/// forms with 2 or 4 values are not supported, so they are a syntax error.
fn parse_colour_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "COLOUR requires a colour number".to_string(),
            line: line_number,
        });
    }

    let colour = parse_expression(tokens)?;
    Ok(Statement::Colour { colour })
}

/// Parse VDU statement: VDU a, b; c
/// A comma or no separator after an item sends 1 byte. A semicolon sends 2.
/// A comma in brackets is part of the item.
fn parse_vdu_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let syntax_error = |message: &str| BBCBasicError::SyntaxError {
        message: message.to_string(),
        line: line_number,
    };
    if tokens.is_empty() {
        return Err(syntax_error("VDU requires a value"));
    }

    let mut items = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    for (pos, token) in tokens.iter().enumerate() {
        match token {
            Token::Separator('(') => depth += 1,
            Token::Separator(')') => depth -= 1,
            Token::Separator(separator @ (',' | ';')) if depth == 0 => {
                if start == pos {
                    return Err(syntax_error("VDU item is missing"));
                }
                items.push(VduItem {
                    value: parse_expression(&tokens[start..pos])?,
                    two_bytes: *separator == ';',
                });
                start = pos + 1;
            }
            _ => {}
        }
    }
    if start < tokens.len() {
        items.push(VduItem {
            value: parse_expression(&tokens[start..])?,
            two_bytes: false,
        });
    }
    Ok(Statement::Vdu { items })
}

/// Parse TRACE statement
/// Supports: TRACE ON, TRACE OFF, TRACE n
fn parse_trace_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let mode = match tokens {
        [Token::Keyword(0xEE)] => TraceMode::On,
        [Token::Keyword(0x87)] => TraceMode::Off,
        [] => {
            return Err(BBCBasicError::SyntaxError {
                message: "TRACE requires ON, OFF or a line number".to_string(),
                line: line_number,
            })
        }
        _ => TraceMode::Below(parse_expression(tokens)?),
    };
    Ok(Statement::Trace { mode })
}

/// Parse WHILE statement
/// WHILE condition
fn parse_while_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "WHILE requires a condition".to_string(),
            line: line_number,
        });
    }

    // Parse the condition expression
    let condition = parse_expression(tokens)?;
    Ok(Statement::While { condition })
}

/// Parse DEF statement (DEF PROC or DEF FN)
/// Supports: DEF PROCname(param1, param2, ...)
/// Supports: DEF FNname(param1, param2, ...)
fn parse_def_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "DEF requires PROC or FN".to_string(),
            line: line_number,
        });
    }

    // Check if it's DEF PROC or DEF FN
    match &tokens[0] {
        Token::Keyword(0xF2) => {
            // DEF PROC
            parse_def_proc(&tokens[1..], line_number)
        }
        Token::Keyword(0xA4) => {
            // DEF FN
            parse_def_fn(&tokens[1..], line_number)
        }
        Token::Keyword(_) => {
            // Unknown keyword after DEF
            Err(BBCBasicError::SyntaxError {
                message: "Expected PROC or FN after DEF".to_string(),
                line: line_number,
            })
        }
        _ => Err(BBCBasicError::SyntaxError {
            message: "Expected PROC or FN after DEF".to_string(),
            line: line_number,
        }),
    }
}

/// Parse DEF PROCname(params)
fn parse_def_proc(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected procedure name after DEF PROC".to_string(),
            line: line_number,
        });
    }

    // Extract procedure name (identifier)
    let name = match &tokens[0] {
        Token::Identifier(n) => n.clone(),
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected identifier for procedure name".to_string(),
                line: line_number,
            })
        }
    };

    // Parse parameters if present
    let params = if tokens.len() > 1 {
        // Should be ( param1, param2, ... )
        parse_parameter_list(&tokens[1..], line_number)?
    } else {
        Vec::new()
    };

    Ok(Statement::DefProc { name, params })
}

/// Parse DEF FNname(params) = expression
fn parse_def_fn(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected function name after DEF FN".to_string(),
            line: line_number,
        });
    }

    // Extract function name (identifier). The name keeps its FN prefix, as
    // in a call, so that the executor can tell a user function from a
    // built-in function.
    let name = match &tokens[0] {
        Token::Identifier(n) => format!("FN{}", n),
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected identifier for function name".to_string(),
                line: line_number,
            })
        }
    };

    // Parse parameters if present
    let (params, rest_start) = if tokens.len() > 1 && is_paren(&tokens[1], '(') {
        // Find closing parenthesis
        let close_pos = tokens.iter().skip(1).position(|t| is_paren(t, ')')).ok_or(
            BBCBasicError::SyntaxError {
                message: "Expected ) after parameter list".to_string(),
                line: line_number,
            },
        )? + 1;

        // Parse parameters (comma-separated identifiers between parentheses)
        let mut params = Vec::new();
        let mut pos = 2; // Skip name and (
        while pos < close_pos {
            // Skip commas
            if matches!(tokens[pos], Token::Separator(',')) {
                pos += 1;
                continue;
            }

            // Expect identifier
            match &tokens[pos] {
                Token::Identifier(param) => {
                    params.push(param.clone());
                    pos += 1;
                }
                _ => {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected parameter name".to_string(),
                        line: line_number,
                    });
                }
            }
        }

        (params, close_pos + 1) // Skip past the )
    } else {
        (Vec::new(), 1)
    };

    // No more tokens: a multi-line function. Its body follows on the next lines.
    if rest_start >= tokens.len() {
        return Ok(Statement::DefFn {
            name,
            params,
            expression: None,
        });
    }

    // Expect = after parameters
    if !matches!(tokens[rest_start], Token::Operator('=')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected = after function parameters".to_string(),
            line: line_number,
        });
    }

    // Parse the expression after =
    let expression = Some(parse_expression(&tokens[rest_start + 1..])?);

    Ok(Statement::DefFn {
        name,
        params,
        expression,
    })
}

/// Parse PROC call: PROCname or PROCname(args)
fn parse_proc_call(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected procedure name after PROC".to_string(),
            line: line_number,
        });
    }

    // Extract procedure name
    let name = match &tokens[0] {
        Token::Identifier(n) => n.clone(),
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected identifier for procedure name".to_string(),
                line: line_number,
            })
        }
    };

    // Parse arguments if present
    let args = if tokens.len() > 1 {
        parse_argument_list(&tokens[1..], line_number)?
    } else {
        Vec::new()
    };

    Ok(Statement::ProcCall { name, args })
}

/// Parse LOCAL statement: LOCAL var1, var2, var3
fn parse_local_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let mut variables = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        // Skip commas
        if matches!(tokens[pos], Token::Separator(',')) {
            pos += 1;
            continue;
        }

        // Expect variable name
        match &tokens[pos] {
            Token::Identifier(name) => {
                pos += 1;
                // An array is named NAME(). Keep the brackets so that LOCAL A%()
                // and LOCAL A% stay different variables.
                if pos + 1 < tokens.len()
                    && is_paren(&tokens[pos], '(')
                    && is_paren(&tokens[pos + 1], ')')
                {
                    variables.push(format!("{name}()"));
                    pos += 2;
                } else {
                    variables.push(name.clone());
                }
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected variable name in LOCAL".to_string(),
                    line: line_number,
                });
            }
        }
    }

    if variables.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "LOCAL requires at least one variable".to_string(),
            line: line_number,
        });
    }

    Ok(Statement::Local { variables })
}

/// Check for a parenthesis token. The tokenizer emits parentheses as separators.
fn is_paren(token: &Token, paren: char) -> bool {
    matches!(token, Token::Operator(c) | Token::Separator(c) if *c == paren)
}

/// Parse argument list: (expr1, expr2, ...)
fn parse_argument_list(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<Expression>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Expect opening parenthesis
    if !is_paren(&tokens[0], '(') {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected ( after procedure name".to_string(),
            line: line_number,
        });
    }

    // Find closing parenthesis
    let close_pos =
        tokens
            .iter()
            .rposition(|t| is_paren(t, ')'))
            .ok_or(BBCBasicError::SyntaxError {
                message: "Expected ) after argument list".to_string(),
                line: line_number,
            })?;

    if close_pos == 1 {
        // Empty argument list: ()
        return Ok(Vec::new());
    }

    // Parse comma-separated expressions between parentheses
    let mut args = Vec::new();
    let mut start = 1;
    let mut depth = 0;

    for i in 1..close_pos {
        match &tokens[i] {
            Token::Operator('(') | Token::Separator('(') => depth += 1,
            Token::Operator(')') | Token::Separator(')') => depth -= 1,
            Token::Separator(',') if depth == 0 => {
                // Parse expression from start to i
                let expr = parse_expression(&tokens[start..i])?;
                args.push(expr);
                start = i + 1;
            }
            _ => {}
        }
    }

    // Parse final expression
    if start < close_pos {
        let expr = parse_expression(&tokens[start..close_pos])?;
        args.push(expr);
    }

    Ok(args)
}

/// Parse parameter list: (param1, param2, ...)
fn parse_parameter_list(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<String>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Expect opening parenthesis
    if !is_paren(&tokens[0], '(') {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected ( after procedure name".to_string(),
            line: line_number,
        });
    }

    // Find closing parenthesis
    let close_pos =
        tokens
            .iter()
            .rposition(|t| is_paren(t, ')'))
            .ok_or(BBCBasicError::SyntaxError {
                message: "Expected ) after parameter list".to_string(),
                line: line_number,
            })?;

    // Extract parameter names between parentheses
    let mut params = Vec::new();
    let mut i = 1;
    while i < close_pos {
        match &tokens[i] {
            Token::Identifier(name) => {
                params.push(name.clone());
                i += 1;

                // Check for comma or end
                if i < close_pos {
                    if matches!(tokens[i], Token::Separator(',')) {
                        i += 1; // Skip comma
                    } else {
                        return Err(BBCBasicError::SyntaxError {
                            message: "Expected , between parameters".to_string(),
                            line: line_number,
                        });
                    }
                }
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected identifier in parameter list".to_string(),
                    line: line_number,
                })
            }
        }
    }

    Ok(params)
}

/// Parse IF statement
/// Supports: IF condition [THEN] statement [ELSE statement]
fn parse_if_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let expected_then = || BBCBasicError::SyntaxError {
        message: "Expected THEN after IF condition".to_string(),
        line: line_number,
    };
    if tokens.is_empty() {
        return Err(expected_then());
    }

    // The condition is one expression: it ends at the first token that
    // cannot continue it (issue 160). THEN after it is optional. With no
    // THEN, a statement or ELSE must follow, and a line number cannot.
    let mut condition_end = 0;
    let condition = parse_expr_precedence(tokens, &mut condition_end, 0)?;
    let has_then = matches!(tokens.get(condition_end), Some(Token::Keyword(0x8C)));
    if !has_then
        && matches!(
            tokens.get(condition_end),
            None | Some(
                Token::Separator(':')
                    | Token::Integer(_)
                    | Token::HexInteger(_)
                    | Token::LineNumber(_)
            )
        )
    {
        return Err(expected_then());
    }
    let branch_start = condition_end + usize::from(has_then);

    // Find ELSE keyword (if present). A REM or DATA statement uses the rest
    // of the line, so an ELSE after it is part of it. A statement starts
    // after a colon or after the THEN of an inner IF.
    let after_then = &tokens[branch_start..];
    let mut else_pos = None;
    for (index, token) in after_then.iter().enumerate() {
        let statement_start = index == 0
            || matches!(
                after_then[index - 1],
                Token::Separator(':') | Token::Keyword(0x8C)
            );
        if statement_start && matches!(token, Token::Keyword(0xF4 | 0xDC)) {
            break;
        }
        if matches!(token, Token::Keyword(0x8B)) {
            else_pos = Some(index);
            break;
        }
    }

    let (then_tokens, else_tokens) = if let Some(else_idx) = else_pos {
        // ELSE found: split then_part and else_part
        let absolute_else_pos = branch_start + else_idx;
        (
            &tokens[branch_start..absolute_else_pos],
            Some(&tokens[absolute_else_pos + 1..]),
        )
    } else {
        // No ELSE: only then_part
        (after_then, None)
    };

    // Nothing after THEN: a block IF. Its THEN block is on the next lines.
    if then_tokens.is_empty() && else_tokens.is_none() {
        return Ok(Statement::IfBlock { condition });
    }

    // Parse THEN part: all the statements up to ELSE or the end of the line.
    // With no THEN, ELSE can follow the condition directly.
    let then_part = if then_tokens.is_empty() && !has_then {
        Vec::new()
    } else if then_tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected statement after THEN".to_string(),
            line: line_number,
        });
    } else {
        parse_if_branch(then_tokens, line_number)?
    };

    // Parse ELSE part if present
    let else_part = if let Some(else_toks) = else_tokens {
        if else_toks.is_empty() {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected statement after ELSE".to_string(),
                line: line_number,
            });
        }
        Some(parse_if_branch(else_toks, line_number)?)
    } else {
        None
    };

    Ok(Statement::If {
        condition,
        then_part,
        else_part,
    })
}

/// Parse a THEN or an ELSE branch. A line number at the start of the branch
/// is a short form of GOTO. The statements after it can never run. Only the
/// end of the branch or a colon can follow the line number.
///
/// A block IF or a CASE cannot be in a branch, because the branch ends with
/// its line.
fn parse_if_branch(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<Statement>> {
    let target = match tokens.first() {
        Some(Token::Integer(n) | Token::HexInteger(n)) => literal_line_number(*n, line_number)?,
        Some(Token::LineNumber(n)) => *n,
        _ => {
            let statements = parse_statements(tokens, line_number)?;
            if let Some(Statement::IfBlock { .. }) = statements.last() {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected statement after THEN".to_string(),
                    line: line_number,
                });
            }
            if statements
                .iter()
                .any(|s| matches!(s, Statement::Case { .. }))
            {
                return Err(BBCBasicError::SyntaxError {
                    message: "CASE cannot be in an IF branch".to_string(),
                    line: line_number,
                });
            }
            return Ok(statements);
        }
    };
    match tokens.get(1) {
        None | Some(Token::Separator(':')) => Ok(vec![Statement::Goto {
            line_number: target,
        }]),
        Some(_) => Err(BBCBasicError::SyntaxError {
            message: "Expected end of statement after line number".to_string(),
            line: line_number,
        }),
    }
}

/// Parse a sequence of tokens into an expression
pub fn parse_expression(tokens: &[Token]) -> Result<Expression> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected expression".to_string(),
            line: None,
        });
    }

    let mut pos = 0;
    let expression = parse_expr_precedence(tokens, &mut pos, 0)?;
    if let Some(token) = tokens.get(pos) {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "Unexpected {} after expression",
                tokens_text(std::slice::from_ref(token))
            ),
            line: None,
        });
    }
    Ok(expression)
}

/// Get operator precedence (higher number = higher precedence)
fn get_precedence(op: char) -> Option<u8> {
    match op {
        '^' => Some(60),             // Power (highest)
        '*' | '/' => Some(50),       // Multiplication, Division
        '+' | '-' => Some(40),       // Addition, Subtraction
        '=' | '<' | '>' => Some(30), // Comparison
        _ => None,
    }
}

/// Get keyword operator precedence
fn get_keyword_precedence(keyword_code: u8) -> Option<u8> {
    match keyword_code {
        0x81 => Some(50), // DIV - same as / (integer division)
        0x83 => Some(50), // MOD - same as / (modulo)
        0x80 => Some(20), // AND - lower than comparison
        0x82 => Some(15), // EOR - lower than AND, same as OR
        0x84 => Some(15), // OR - lower than AND
        _ => None,
    }
}

/// Convert operator character to BinaryOperator
fn char_to_binary_op(op: char) -> Option<BinaryOperator> {
    match op {
        '+' => Some(BinaryOperator::Add),
        '-' => Some(BinaryOperator::Subtract),
        '*' => Some(BinaryOperator::Multiply),
        '/' => Some(BinaryOperator::Divide),
        '^' => Some(BinaryOperator::Power),
        '=' => Some(BinaryOperator::Equal),
        '<' => Some(BinaryOperator::LessThan),
        '>' => Some(BinaryOperator::GreaterThan),
        _ => None,
    }
}

/// Convert keyword code to BinaryOperator
fn keyword_to_binary_op(keyword_code: u8) -> Option<BinaryOperator> {
    match keyword_code {
        0x81 => Some(BinaryOperator::IntegerDivide), // DIV
        0x83 => Some(BinaryOperator::Modulo),        // MOD
        0x80 => Some(BinaryOperator::And),           // AND
        0x82 => Some(BinaryOperator::Eor),           // EOR
        0x84 => Some(BinaryOperator::Or),            // OR
        _ => None,
    }
}

/// Parse expression with precedence climbing algorithm
fn parse_expr_precedence(tokens: &[Token], pos: &mut usize, min_prec: u8) -> Result<Expression> {
    // Parse the left-hand side (primary expression)
    let mut left = parse_primary(tokens, pos)?;

    // Parse binary operators with precedence
    while *pos < tokens.len() {
        // Check if current token is a binary operator (either operator or keyword)
        let (prec, op, consumed) = match &tokens[*pos] {
            Token::Operator(ch) => {
                // Check for >= and <= (two-character operators)
                if (*ch == '>' || *ch == '<') && *pos + 1 < tokens.len() {
                    if let Token::Operator('=') = tokens[*pos + 1] {
                        // This is >= or <=
                        let op = if *ch == '>' {
                            BinaryOperator::GreaterThanOrEqual
                        } else {
                            BinaryOperator::LessThanOrEqual
                        };
                        (30, op, 2) // Consume 2 tokens
                    } else if *ch == '<' && matches!(tokens[*pos + 1], Token::Operator('>')) {
                        (30, BinaryOperator::NotEqual, 2)
                    } else if tokens[*pos + 1] == Token::Operator(*ch) {
                        // << and >> (BBC BASIC V) have the precedence of a comparison
                        let op = if *ch == '<' {
                            BinaryOperator::LeftShift
                        } else {
                            BinaryOperator::RightShift
                        };
                        (30, op, 2)
                    } else if let Some(p) = get_precedence(*ch) {
                        if let Some(binary_op) = char_to_binary_op(*ch) {
                            (p, binary_op, 1)
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else if let Some(p) = get_precedence(*ch) {
                    if let Some(binary_op) = char_to_binary_op(*ch) {
                        (p, binary_op, 1)
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            Token::Keyword(code) => {
                if let Some(p) = get_keyword_precedence(*code) {
                    if let Some(binary_op) = keyword_to_binary_op(*code) {
                        (p, binary_op, 1)
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            _ => break,
        };

        // Check precedence
        if prec < min_prec {
            break;
        }

        *pos += consumed; // consume operator(s)

        // Parse right-hand side with higher precedence
        let right = parse_expr_precedence(tokens, pos, prec + 1)?;

        // Create binary operation
        left = Expression::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse a primary expression: an operand, then each binary memory form
/// `base?offset` or `base!offset`. A memory form binds more than every
/// arithmetic operator, and a chain reads from left to right (issue 76).
fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<Expression> {
    let mut expr = parse_operand(tokens, pos)?;
    while let Some(width) = Indirection::from_operator(tokens.get(*pos)) {
        *pos += 1;
        let offset = parse_operand(tokens, pos)?;
        expr = Expression::UnaryOp {
            op: UnaryOperator::Indirect(width),
            operand: Box::new(Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::Add,
                right: Box::new(offset),
            }),
        };
    }
    Ok(expr)
}

/// The built-in functions of one value. With no bracket, each one takes the
/// next factor as its value: `SIN X + 1` is `(SIN X) + 1` (issue 143).
const ONE_VALUE_FUNCTIONS: &[&str] = &[
    "ABS", "ACS", "ADVAL", "ASC", "ASN", "ATN", "CHR$", "COS", "DEG", "EVAL", "EXP", "INKEY",
    "INKEY$", "INT", "LEN", "LN", "LOG", "OPENIN", "OPENOUT", "OPENUP", "RAD", "SGN", "SIN", "SQR",
    "STR$", "STR$~", "TAN", "VAL",
];

/// The built-in functions of 2 or more values. Each one needs a bracket.
const TWO_OR_MORE_VALUE_FUNCTIONS: &[&str] =
    &["INSTR", "LEFT$", "MID$", "POINT", "RIGHT$", "STRING$"];

/// The maximum nesting depth of an expression: brackets, unary operators
/// and function arguments. A deeper nest gives No room, so that the parse
/// cannot overflow the native stack (issue 151).
///
/// On the 64 MiB interpreter thread, a debug build overflows at about 5900
/// levels. EVAL can parse when the evaluation levels already use 42% of the
/// stack, so this limit keeps the total below half of the stack.
const EXPRESSION_NESTING_LIMIT: usize = 500;

thread_local! {
    /// The number of `parse_operand` calls that are open on this thread
    static OPERAND_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One open level of `parse_operand`. The level closes when the guard
/// drops, on each exit path.
struct NestingLevel;

impl NestingLevel {
    /// Open one more level, or give No room at the limit
    fn enter() -> Result<Self> {
        OPERAND_DEPTH.with(|depth| {
            if depth.get() >= EXPRESSION_NESTING_LIMIT {
                return Err(BBCBasicError::NoRoom);
            }
            depth.set(depth.get() + 1);
            Ok(NestingLevel)
        })
    }
}

impl Drop for NestingLevel {
    fn drop(&mut self) {
        OPERAND_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

/// Parse an operand (literal, variable, function call, or parenthesized
/// expression), as one level of the nesting limit
fn parse_operand(tokens: &[Token], pos: &mut usize) -> Result<Expression> {
    let _level = NestingLevel::enter()?;
    parse_operand_level(tokens, pos)
}

/// The body of `parse_operand`, inside its nesting level
fn parse_operand_level(tokens: &[Token], pos: &mut usize) -> Result<Expression> {
    if *pos >= tokens.len() {
        return Err(BBCBasicError::SyntaxError {
            message: "Unexpected end of expression".to_string(),
            line: None,
        });
    }

    let token = &tokens[*pos];

    match token {
        // Literals
        Token::Integer(val) | Token::HexInteger(val) => {
            *pos += 1;
            Ok(Expression::Integer(*val))
        }
        Token::Real(val) => {
            *pos += 1;
            Ok(Expression::Real(*val))
        }
        Token::String(s) => {
            *pos += 1;
            Ok(Expression::String(s.clone()))
        }

        // Variables and array access
        Token::Identifier(name) => {
            *pos += 1;
            // Check if this is followed by '(' for array access
            if *pos < tokens.len() && matches!(tokens[*pos], Token::Separator('(')) {
                // This is array access: name(index1, index2, ...)
                *pos += 1; // consume '('

                let mut indices = Vec::new();
                // Parse indices
                if *pos < tokens.len() && !matches!(tokens[*pos], Token::Separator(')')) {
                    loop {
                        let idx = parse_expr_precedence(tokens, pos, 0)?;
                        indices.push(idx);

                        if *pos >= tokens.len() {
                            break;
                        }

                        match &tokens[*pos] {
                            Token::Separator(',') => {
                                *pos += 1;
                                continue;
                            }
                            Token::Separator(')') => break,
                            _ => break,
                        }
                    }
                }

                // Expect closing parenthesis
                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Separator(')')) {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected ')' in array access".to_string(),
                        line: None,
                    });
                }
                *pos += 1; // consume ')'

                Ok(Expression::ArrayAccess {
                    name: name.clone(),
                    indices,
                })
            } else {
                Ok(Expression::Variable(name.clone()))
            }
        }

        // Unary operators
        Token::Operator('-') => {
            *pos += 1;
            let operand = parse_primary(tokens, pos)?;
            Ok(Expression::UnaryOp {
                op: UnaryOperator::Minus,
                operand: Box::new(operand),
            })
        }
        Token::Operator('+') => {
            *pos += 1;
            let operand = parse_primary(tokens, pos)?;
            Ok(Expression::UnaryOp {
                op: UnaryOperator::Plus,
                operand: Box::new(operand),
            })
        }
        // ?addr and !addr read memory
        Token::Operator(ch @ ('?' | '!')) => {
            let width = if *ch == '?' {
                Indirection::Byte
            } else {
                Indirection::Word
            };
            *pos += 1;
            let address = parse_operand(tokens, pos)?;
            Ok(Expression::UnaryOp {
                op: UnaryOperator::Indirect(width),
                operand: Box::new(address),
            })
        }

        // Parenthesized expressions
        Token::Separator('(') => {
            *pos += 1;
            let expr = parse_expr_precedence(tokens, pos, 0)?;

            // Expect closing parenthesis
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Separator(')')) {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected ')'".to_string(),
                    line: None,
                });
            }
            *pos += 1;
            Ok(expr)
        }

        // Keywords (functions and constants)
        Token::Keyword(byte) => {
            let (main_reverse, _) = create_reverse_keyword_maps();
            let keyword = main_reverse
                .get(byte)
                .cloned()
                .unwrap_or_else(|| "UNKNOWN".to_string());

            *pos += 1;

            // NOT is a unary operator with the priority of unary minus.
            if keyword == "NOT" {
                let operand = parse_primary(tokens, pos)?;
                return Ok(Expression::UnaryOp {
                    op: UnaryOperator::Not,
                    operand: Box::new(operand),
                });
            }

            // BGET#, EOF#, PTR# and EXT#: the channel after '#' is the one argument.
            if matches!(keyword.as_str(), "BGET" | "EOF" | "PTR" | "EXT")
                && matches!(tokens.get(*pos), Some(Token::Operator('#')))
            {
                *pos += 1; // consume '#'
                let channel = parse_primary(tokens, pos)?;
                return Ok(Expression::FunctionCall {
                    name: keyword,
                    args: vec![channel],
                });
            }

            // FN name: a call to a user-defined function. The name follows the
            // keyword, and the call keeps the FN prefix.
            let mut is_user_function = false;
            let keyword = match tokens.get(*pos) {
                Some(Token::Identifier(name)) if keyword == "FN" => {
                    *pos += 1;
                    is_user_function = true;
                    format!("FN{}", name)
                }
                // STR$~(X) gives the hex text of X
                Some(Token::Operator('~')) if keyword == "STR$" => {
                    *pos += 1;
                    "STR$~".to_string()
                }
                _ => keyword,
            };

            // Check if this is a function call (followed by opening paren)
            if *pos < tokens.len() && matches!(tokens[*pos], Token::Separator('(')) {
                *pos += 1; // consume '('

                let mut args = Vec::new();

                // Parse arguments
                if *pos < tokens.len() && !matches!(tokens[*pos], Token::Separator(')')) {
                    loop {
                        let arg = parse_expr_precedence(tokens, pos, 0)?;
                        args.push(arg);

                        if *pos >= tokens.len() {
                            break;
                        }

                        match &tokens[*pos] {
                            Token::Separator(',') => {
                                *pos += 1; // consume comma
                                continue;
                            }
                            Token::Separator(')') => break,
                            _ => break,
                        }
                    }
                }

                // Expect closing parenthesis
                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Separator(')')) {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected ')'".to_string(),
                        line: None,
                    });
                }
                *pos += 1;

                Ok(Expression::FunctionCall {
                    name: keyword,
                    args,
                })
            } else if is_user_function
                || matches!(
                    keyword.as_str(),
                    "TRUE" | "FALSE" | "RND" | "COUNT" | "POS" | "VPOS" | "GET" | "GET$"
                )
            {
                // A user-defined function with no parameters, or a keyword
                // that gives a value with no bracket
                Ok(Expression::FunctionCall {
                    name: keyword,
                    args: Vec::new(),
                })
            } else if ONE_VALUE_FUNCTIONS.contains(&keyword.as_str()) {
                // A function of one value with no bracket takes the next factor
                let argument = parse_primary(tokens, pos)?;
                Ok(Expression::FunctionCall {
                    name: keyword,
                    args: vec![argument],
                })
            } else if TWO_OR_MORE_VALUE_FUNCTIONS.contains(&keyword.as_str()) {
                Err(BBCBasicError::SyntaxError {
                    message: format!("Expected '(' after {}", keyword),
                    line: None,
                })
            } else {
                // It's a constant or keyword used as value
                Ok(Expression::Variable(keyword))
            }
        }

        _ => Err(BBCBasicError::SyntaxError {
            message: format!(
                "Unexpected token in expression: {}",
                tokens_text(std::slice::from_ref(token))
            ),
            line: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statement_types() {
        let assignment = Statement::Assignment {
            target: "A".to_string(),
            expression: Expression::Integer(42),
        };
        assert!(!assignment.is_control_flow());
        assert!(!assignment.is_terminating());

        let end_stmt = Statement::End;
        assert!(end_stmt.is_terminating());

        let for_stmt = Statement::For {
            variable: "I".to_string(),
            start: Expression::Integer(1),
            end: Expression::Integer(10),
            step: None,
        };
        assert!(for_stmt.is_control_flow());
    }

    #[test]
    fn test_expression_types() {
        let int_expr = Expression::Integer(42);
        assert_eq!(int_expr.expression_type(), ExpressionType::Integer);

        let real_expr = Expression::Real(3.14);
        assert_eq!(real_expr.expression_type(), ExpressionType::Real);

        let string_expr = Expression::String("hello".to_string());
        assert_eq!(string_expr.expression_type(), ExpressionType::String);

        let int_var = Expression::Variable("A%".to_string());
        assert_eq!(int_var.expression_type(), ExpressionType::Integer);

        let string_var = Expression::Variable("B$".to_string());
        assert_eq!(string_var.expression_type(), ExpressionType::String);

        let real_var = Expression::Variable("C".to_string());
        assert_eq!(real_var.expression_type(), ExpressionType::Real);
    }

    #[test]
    fn test_binary_operators() {
        let add_expr = Expression::BinaryOp {
            left: Box::new(Expression::Integer(1)),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Integer(2)),
        };
        assert_eq!(add_expr.expression_type(), ExpressionType::Numeric);
    }

    // TDD Tests for expression parsing

    #[test]
    fn test_parse_integer_literal() {
        // RED: Parse simple integer
        let tokens = vec![Token::Integer(42)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(expr, Expression::Integer(42));
    }

    #[test]
    fn test_parse_real_literal() {
        // RED: Parse real number
        let tokens = vec![Token::Real(3.14)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(expr, Expression::Real(3.14));
    }

    #[test]
    fn test_parse_string_literal() {
        // RED: Parse string
        let tokens = vec![Token::String("hello".to_string())];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(expr, Expression::String("hello".to_string()));
    }

    #[test]
    fn test_parse_variable() {
        // RED: Parse variable reference
        let tokens = vec![Token::Identifier("A%".to_string())];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(expr, Expression::Variable("A%".to_string()));
    }

    #[test]
    fn test_parse_simple_addition() {
        // RED: Parse "2 + 3"
        let tokens = vec![Token::Integer(2), Token::Operator('+'), Token::Integer(3)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(Expression::Integer(2)),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Integer(3)),
            }
        );
    }

    /// The expression for TRUE or FALSE: a function call with no arguments.
    fn constant(name: &str) -> Expression {
        Expression::FunctionCall {
            name: name.to_string(),
            args: Vec::new(),
        }
    }

    #[test]
    fn test_parse_true_and_false_without_bracket() {
        let true_expr = parse_expression(&[Token::Keyword(0xB9)]).unwrap();
        let false_expr = parse_expression(&[Token::Keyword(0xA3)]).unwrap();

        assert_eq!(true_expr, constant("TRUE"));
        assert_eq!(false_expr, constant("FALSE"));
    }

    #[test]
    fn test_parse_rnd_count_pos_vpos_without_bracket() {
        // RND 0xB3, COUNT 0x9C, POS 0xB1, VPOS 0xBC
        for (byte, name) in [
            (0xB3, "RND"),
            (0x9C, "COUNT"),
            (0xB1, "POS"),
            (0xBC, "VPOS"),
        ] {
            let expr = parse_expression(&[Token::Keyword(byte)]).unwrap();
            assert_eq!(expr, constant(name), "{name} with no bracket");
        }
    }

    #[test]
    fn test_parse_get_and_get_string_without_bracket() {
        // GET 0xA5, GET$ 0xBE
        for (byte, name) in [(0xA5, "GET"), (0xBE, "GET$")] {
            let expr = parse_expression(&[Token::Keyword(byte)]).unwrap();
            assert_eq!(expr, constant(name), "{name} with no bracket");
        }
    }

    #[test]
    fn test_parse_assignment_from_get_and_get_string() {
        use crate::tokenizer::tokenize;

        for (source, target, name) in [("A% = GET", "A%", "GET"), ("A$ = GET$", "A$", "GET$")] {
            match parse_statement(&tokenize(source).unwrap()).unwrap() {
                Statement::Assignment {
                    target: t,
                    expression,
                } => {
                    assert_eq!(t, target, "{source}");
                    assert_eq!(expression, constant(name), "{source}");
                }
                other => panic!("{source} parsed as {other:?}"),
            }
        }
    }

    /// A call of a built-in function with one value.
    fn call(name: &str, argument: Expression) -> Expression {
        Expression::FunctionCall {
            name: name.to_string(),
            args: vec![argument],
        }
    }

    #[test]
    fn test_parse_function_with_no_bracket_takes_next_factor() {
        use crate::tokenizer::tokenize;

        // SIN X + 1 is (SIN X) + 1
        let expr = parse_expression(&tokenize("SIN X + 1").unwrap().tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(call("SIN", Expression::Variable("X".to_string()))),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Integer(1)),
            }
        );
    }

    #[test]
    fn test_parse_function_with_no_bracket_takes_unary_minus() {
        use crate::tokenizer::tokenize;

        let expr = parse_expression(&tokenize("ABS -X").unwrap().tokens).unwrap();
        assert_eq!(
            expr,
            call(
                "ABS",
                Expression::UnaryOp {
                    op: UnaryOperator::Minus,
                    operand: Box::new(Expression::Variable("X".to_string())),
                }
            )
        );
    }

    #[test]
    fn test_parse_function_with_and_with_no_bracket_are_the_same() {
        use crate::tokenizer::tokenize;

        for (plain, bracket) in [
            ("LEN A$", "LEN(A$)"),
            ("CHR$ 65", "CHR$(65)"),
            ("STR$~ 255", "STR$~(255)"),
            ("LEN STR$ 12", "LEN(STR$(12))"),
        ] {
            let plain_expr = parse_expression(&tokenize(plain).unwrap().tokens).unwrap();
            let bracket_expr = parse_expression(&tokenize(bracket).unwrap().tokens).unwrap();
            assert_eq!(plain_expr, bracket_expr, "{plain}");
        }
    }

    #[test]
    fn test_parse_function_of_two_values_with_no_bracket_is_error() {
        use crate::tokenizer::tokenize;

        let result = parse_expression(&tokenize("LEFT$ A$").unwrap().tokens);
        assert!(
            matches!(&result, Err(BBCBasicError::SyntaxError { message, .. })
                if message == "Expected '(' after LEFT$"),
            "{result:?}"
        );
    }

    #[test]
    fn test_parse_rnd_with_bracket_keeps_argument() {
        let tokens = [
            Token::Keyword(0xB3),
            Token::Separator('('),
            Token::Integer(1),
            Token::Separator(')'),
        ];
        let expr = parse_expression(&tokens).unwrap();

        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "RND".to_string(),
                args: vec![Expression::Integer(1)],
            }
        );
    }

    #[test]
    fn test_parse_true_and_integer() {
        // TRUE AND 5
        let tokens = [
            Token::Keyword(0xB9),
            Token::Keyword(0x80),
            Token::Integer(5),
        ];
        let expr = parse_expression(&tokens).unwrap();

        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(constant("TRUE")),
                op: BinaryOperator::And,
                right: Box::new(Expression::Integer(5)),
            }
        );
    }

    #[test]
    fn test_parse_not_false() {
        let tokens = [Token::Keyword(0xAC), Token::Keyword(0xA3)];
        let expr = parse_expression(&tokens).unwrap();

        assert_eq!(
            expr,
            Expression::UnaryOp {
                op: UnaryOperator::Not,
                operand: Box::new(constant("FALSE")),
            }
        );
    }

    #[test]
    fn test_keyword_codes_give_their_operators() {
        // The tokenizer gives AND 0x80, DIV 0x81, EOR 0x82, MOD 0x83 and OR 0x84.
        let cases = [
            (0x80, BinaryOperator::And),
            (0x81, BinaryOperator::IntegerDivide),
            (0x82, BinaryOperator::Eor),
            (0x83, BinaryOperator::Modulo),
            (0x84, BinaryOperator::Or),
        ];
        for (code, op) in cases {
            let tokens = [Token::Integer(6), Token::Keyword(code), Token::Integer(3)];
            let expr = parse_expression(&tokens).unwrap();

            assert_eq!(
                expr,
                binary(Expression::Integer(6), op, Expression::Integer(3)),
                "keyword code {code:#04X}"
            );
        }
    }

    #[test]
    fn test_and_binds_more_than_or() {
        // 1 OR 2 AND 4 is 1 OR (2 AND 4)
        let tokens = [
            Token::Integer(1),
            Token::Keyword(0x84),
            Token::Integer(2),
            Token::Keyword(0x80),
            Token::Integer(4),
        ];
        let expr = parse_expression(&tokens).unwrap();

        assert_eq!(
            expr,
            binary(
                Expression::Integer(1),
                BinaryOperator::Or,
                binary(
                    Expression::Integer(2),
                    BinaryOperator::And,
                    Expression::Integer(4)
                ),
            )
        );
    }

    #[test]
    fn test_or_and_eor_have_equal_precedence() {
        // 1 EOR 2 OR 3 is (1 EOR 2) OR 3, and 1 OR 2 EOR 3 is (1 OR 2) EOR 3
        let eor_first = [
            Token::Integer(1),
            Token::Keyword(0x82),
            Token::Integer(2),
            Token::Keyword(0x84),
            Token::Integer(3),
        ];
        let or_first = [
            Token::Integer(1),
            Token::Keyword(0x84),
            Token::Integer(2),
            Token::Keyword(0x82),
            Token::Integer(3),
        ];

        assert_eq!(
            parse_expression(&eor_first).unwrap(),
            binary(
                binary(
                    Expression::Integer(1),
                    BinaryOperator::Eor,
                    Expression::Integer(2)
                ),
                BinaryOperator::Or,
                Expression::Integer(3),
            )
        );
        assert_eq!(
            parse_expression(&or_first).unwrap(),
            binary(
                binary(
                    Expression::Integer(1),
                    BinaryOperator::Or,
                    Expression::Integer(2)
                ),
                BinaryOperator::Eor,
                Expression::Integer(3),
            )
        );
    }

    #[test]
    fn test_comparison_binds_more_than_or() {
        // 1 = 1 OR 2 = 3 is (1 = 1) OR (2 = 3)
        let tokens = [
            Token::Integer(1),
            Token::Operator('='),
            Token::Integer(1),
            Token::Keyword(0x84),
            Token::Integer(2),
            Token::Operator('='),
            Token::Integer(3),
        ];
        let expr = parse_expression(&tokens).unwrap();

        assert_eq!(
            expr,
            binary(
                binary(
                    Expression::Integer(1),
                    BinaryOperator::Equal,
                    Expression::Integer(1)
                ),
                BinaryOperator::Or,
                binary(
                    Expression::Integer(2),
                    BinaryOperator::Equal,
                    Expression::Integer(3)
                ),
            )
        );
    }

    #[test]
    fn test_parse_power_operator() {
        // RED: Parse "2 ^ 3" (power/exponentiation)
        let tokens = vec![Token::Integer(2), Token::Operator('^'), Token::Integer(3)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(Expression::Integer(2)),
                op: BinaryOperator::Power,
                right: Box::new(Expression::Integer(3)),
            }
        );
    }

    #[test]
    fn test_parse_mod_operator() {
        // RED: Parse "10 MOD 3" (modulo)
        let tokens = vec![Token::Integer(10), Token::Keyword(0x83), Token::Integer(3)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(Expression::Integer(10)),
                op: BinaryOperator::Modulo,
                right: Box::new(Expression::Integer(3)),
            }
        );
    }

    #[test]
    fn test_parse_div_operator() {
        // RED: Parse "10 DIV 3" (integer division)
        let tokens = vec![Token::Integer(10), Token::Keyword(0x81), Token::Integer(3)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(Expression::Integer(10)),
                op: BinaryOperator::IntegerDivide,
                right: Box::new(Expression::Integer(3)),
            }
        );
    }

    #[test]
    fn test_parse_operator_precedence() {
        // RED: Parse "2 + 3 * 4" - should be 2 + (3 * 4)
        let tokens = vec![
            Token::Integer(2),
            Token::Operator('+'),
            Token::Integer(3),
            Token::Operator('*'),
            Token::Integer(4),
        ];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(Expression::Integer(2)),
                op: BinaryOperator::Add,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Integer(3)),
                    op: BinaryOperator::Multiply,
                    right: Box::new(Expression::Integer(4)),
                }),
            }
        );
    }

    #[test]
    fn test_parse_parenthesized_expression() {
        // RED: Parse "(2 + 3)"
        let tokens = vec![
            Token::Separator('('),
            Token::Integer(2),
            Token::Operator('+'),
            Token::Integer(3),
            Token::Separator(')'),
        ];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::BinaryOp {
                left: Box::new(Expression::Integer(2)),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Integer(3)),
            }
        );
    }

    #[test]
    fn test_parse_function_call() {
        // RED: Parse "SIN(45)"
        let tokens = vec![
            Token::Keyword(0xB5), // SIN
            Token::Separator('('),
            Token::Integer(45),
            Token::Separator(')'),
        ];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "SIN".to_string(),
                args: vec![Expression::Integer(45)],
            }
        );
    }

    #[test]
    fn test_parse_unary_minus() {
        // RED: Parse "-5"
        let tokens = vec![Token::Operator('-'), Token::Integer(5)];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(
            expr,
            Expression::UnaryOp {
                op: UnaryOperator::Minus,
                operand: Box::new(Expression::Integer(5)),
            }
        );
    }

    // TDD Tests for statement parsing

    #[test]
    fn test_parse_print_simple() {
        // RED: Parse "PRINT 42"
        use crate::tokenizer::tokenize;
        let line = tokenize("PRINT 42").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Print {
                items: vec![PrintItem::Expression(Expression::Integer(42))],
            }
        );
    }

    #[test]
    fn test_parse_print_tilde_gives_hex_item() {
        use crate::tokenizer::tokenize;
        let line = tokenize("PRINT ~X;~5").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Print {
                items: vec![
                    PrintItem::Hex,
                    PrintItem::Expression(Expression::Variable("X".to_string())),
                    PrintItem::Semicolon,
                    PrintItem::Hex,
                    PrintItem::Expression(Expression::Integer(5)),
                ],
            }
        );
    }

    fn parse_print_text(source: &str) -> Vec<PrintItem> {
        use crate::tokenizer::tokenize;
        match parse_statement(&tokenize(source).unwrap()).unwrap() {
            Statement::Print { items } => items,
            other => panic!("expected PRINT, got {:?}", other),
        }
    }

    fn parse_print_file_text(source: &str) -> Vec<PrintItem> {
        use crate::tokenizer::tokenize;
        match parse_statement(&tokenize(source).unwrap()).unwrap() {
            Statement::PrintFile { items, .. } => items,
            other => panic!("expected PRINT#, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_print_file_two_numbers_with_no_separator_gives_two_items() {
        assert_eq!(
            parse_print_file_text("PRINT#F%, 10 58"),
            vec![
                PrintItem::Expression(Expression::Integer(10)),
                PrintItem::Expression(Expression::Integer(58)),
            ]
        );
    }

    #[test]
    fn test_parse_print_file_hex_form() {
        assert_eq!(
            parse_print_file_text("PRINT#F%, ~255;255"),
            vec![
                PrintItem::Hex,
                PrintItem::Expression(Expression::Integer(255)),
                PrintItem::Semicolon,
                PrintItem::Expression(Expression::Integer(255)),
            ]
        );
    }

    #[test]
    fn test_parse_print_file_tan_is_a_function_call() {
        assert!(matches!(
            parse_print_file_text("PRINT#F%, TAN(0)").as_slice(),
            [PrintItem::Expression(Expression::FunctionCall { .. })]
        ));
    }

    #[test]
    fn test_parse_print_tan_is_a_function_call() {
        assert!(matches!(
            parse_print_text("PRINT TAN(0)").as_slice(),
            [PrintItem::Expression(Expression::FunctionCall { .. })]
        ));
    }

    #[test]
    fn test_parse_print_two_numbers_with_no_separator_gives_two_items() {
        assert_eq!(
            parse_print_text("PRINT 10 58"),
            vec![
                PrintItem::Expression(Expression::Integer(10)),
                PrintItem::Expression(Expression::Integer(58)),
            ]
        );
    }

    #[test]
    fn test_parse_print_two_strings_with_no_separator_gives_two_items() {
        assert_eq!(
            parse_print_text("PRINT \"A\" \"B\""),
            vec![
                PrintItem::Expression(Expression::String("A".to_string())),
                PrintItem::Expression(Expression::String("B".to_string())),
            ]
        );
    }

    #[test]
    fn test_parse_print_two_variables_with_no_separator_gives_two_items() {
        assert_eq!(
            parse_print_text("PRINT A$ B$"),
            vec![
                PrintItem::Expression(Expression::Variable("A$".to_string())),
                PrintItem::Expression(Expression::Variable("B$".to_string())),
            ]
        );
    }

    #[test]
    fn test_parse_print_subtract_is_one_item() {
        assert_eq!(
            parse_print_text("PRINT 1 - 2"),
            vec![PrintItem::Expression(Expression::BinaryOp {
                left: Box::new(Expression::Integer(1)),
                op: BinaryOperator::Subtract,
                right: Box::new(Expression::Integer(2)),
            })]
        );
    }

    #[test]
    fn test_parse_print_hex_then_two_items_with_no_separator() {
        assert_eq!(
            parse_print_text("PRINT ~10 58,58"),
            vec![
                PrintItem::Hex,
                PrintItem::Expression(Expression::Integer(10)),
                PrintItem::Expression(Expression::Integer(58)),
                PrintItem::Comma,
                PrintItem::Expression(Expression::Integer(58)),
            ]
        );
    }

    #[test]
    fn test_parse_print_tab_then_string_with_no_separator() {
        assert_eq!(
            parse_print_text("PRINT TAB(5) \"X\""),
            vec![
                PrintItem::Tab(Expression::Integer(5)),
                PrintItem::Expression(Expression::String("X".to_string())),
            ]
        );
    }

    #[test]
    fn test_parse_print_tab_with_2_values() {
        assert_eq!(
            parse_print_text("PRINT TAB(5, 3); \"X\""),
            vec![
                PrintItem::TabXY(Expression::Integer(5), Expression::Integer(3)),
                PrintItem::Semicolon,
                PrintItem::Expression(Expression::String("X".to_string())),
            ]
        );
    }

    #[test]
    fn test_parse_print_tab_with_2_values_with_brackets_in_a_value() {
        let items = parse_print_text("PRINT TAB(FNa(1, 2), (3))");
        assert!(
            matches!(
                items.as_slice(),
                [PrintItem::TabXY(
                    Expression::FunctionCall { .. },
                    Expression::Integer(3)
                )]
            ),
            "items: {:?}",
            items
        );
    }

    #[test]
    fn test_parse_print_file_rejects_tab_and_spc() {
        use crate::tokenizer::tokenize;
        for source in ["PRINT#1, TAB(2)", "PRINT#1, TAB(2, 3)", "PRINT#1, SPC(2)"] {
            let error = parse_statement(&tokenize(source).unwrap()).unwrap_err();
            assert!(
                matches!(
                    &error,
                    BBCBasicError::SyntaxError { message, .. }
                        if message == "TAB and SPC are not permitted in PRINT#"
                ),
                "{source}: {error:?}"
            );
        }
    }

    #[test]
    fn test_parse_str_tilde_gives_hex_function_call() {
        use crate::tokenizer::tokenize;
        let line = tokenize("PRINT STR$~(X)").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Print {
                items: vec![PrintItem::Expression(Expression::FunctionCall {
                    name: "STR$~".to_string(),
                    args: vec![Expression::Variable("X".to_string())],
                })],
            }
        );
    }

    #[test]
    fn test_parse_print_tilde_in_string_is_a_string() {
        use crate::tokenizer::tokenize;
        let line = tokenize("PRINT \"~X\"").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Print {
                items: vec![PrintItem::Expression(Expression::String("~X".to_string()))],
            }
        );
    }

    #[test]
    fn test_parse_print_string() {
        // RED: Parse "PRINT \"Hello\""
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"PRINT "Hello""#).unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Print {
                items: vec![PrintItem::Expression(Expression::String(
                    "Hello".to_string()
                ))],
            }
        );
    }

    #[test]
    fn test_parse_assignment() {
        // RED: Parse "A% = 42"
        use crate::tokenizer::tokenize;
        let line = tokenize("A% = 42").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Assignment {
                target: "A%".to_string(),
                expression: Expression::Integer(42),
            }
        );
    }

    #[test]
    fn test_parse_let_assignment() {
        // RED: Parse "LET B = 3.14"
        use crate::tokenizer::tokenize;
        let line = tokenize("LET B = 3.14").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Assignment {
                target: "B".to_string(),
                expression: Expression::Real(3.14),
            }
        );
    }

    #[test]
    fn test_parse_for_loop() {
        // RED: Parse "FOR I% = 1 TO 10"
        use crate::tokenizer::tokenize;
        let line = tokenize("FOR I% = 1 TO 10").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::For {
                variable: "I%".to_string(),
                start: Expression::Integer(1),
                end: Expression::Integer(10),
                step: None,
            }
        );
    }

    #[test]
    fn test_parse_for_loop_with_step() {
        // RED: Parse "FOR I% = 10 TO 1 STEP -1"
        use crate::tokenizer::tokenize;
        let line = tokenize("FOR I% = 10 TO 1 STEP -1").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::For {
                variable: "I%".to_string(),
                start: Expression::Integer(10),
                end: Expression::Integer(1),
                step: Some(Expression::Integer(-1)),
            }
        );
    }

    #[test]
    fn test_parse_next() {
        // RED: Parse "NEXT I%"
        use crate::tokenizer::tokenize;
        let line = tokenize("NEXT I%").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Next {
                variables: vec!["I%".to_string()],
            }
        );
    }

    #[test]
    fn test_parse_goto() {
        // RED: Parse "GOTO 100"
        use crate::tokenizer::tokenize;
        let line = tokenize("GOTO 100").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(stmt, Statement::Goto { line_number: 100 });
    }

    #[test]
    fn test_parse_gosub() {
        // RED: Parse "GOSUB 1000"
        use crate::tokenizer::tokenize;
        let line = tokenize("GOSUB 1000").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(stmt, Statement::Gosub { line_number: 1000 });
    }

    #[test]
    fn test_parse_goto_with_calculated_target_in_brackets() {
        use crate::tokenizer::tokenize;
        let line = tokenize("GOTO (100 + X)").unwrap();
        let stmt = parse_statement(&line).unwrap();

        let target = parse_expression(&line.tokens[1..]).unwrap();
        assert_eq!(stmt, Statement::GotoExpression { target });
    }

    #[test]
    fn test_parse_gosub_with_calculated_target() {
        use crate::tokenizer::tokenize;
        let line = tokenize("GOSUB X * 10").unwrap();
        let stmt = parse_statement(&line).unwrap();

        let target = parse_expression(&line.tokens[1..]).unwrap();
        assert_eq!(stmt, Statement::GosubExpression { target });
    }

    #[test]
    fn test_parse_goto_number_then_more_tokens_is_syntax_error() {
        use crate::tokenizer::tokenize;
        for source in ["10 GOTO 100 + X", "10 GOSUB 100 + X"] {
            let line = tokenize(source).unwrap();
            match parse_statement(&line) {
                Err(BBCBasicError::SyntaxError { line, .. }) => {
                    assert_eq!(line, Some(10), "{}", source)
                }
                other => panic!("{:?} gave {:?}", source, other),
            }
        }
    }

    #[test]
    fn test_parse_goto_expression_with_extra_tokens_is_syntax_error() {
        use crate::tokenizer::tokenize;
        let line = tokenize("10 GOTO (100) X").unwrap();
        match parse_statement(&line) {
            Err(BBCBasicError::SyntaxError { line, .. }) => assert_eq!(line, Some(10)),
            other => panic!("gave {:?}", other),
        }
    }

    #[test]
    fn test_parse_return() {
        // RED: Parse "RETURN"
        use crate::tokenizer::tokenize;
        let line = tokenize("RETURN").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(stmt, Statement::Return { value: None });
    }

    #[test]
    fn test_parse_return_with_expression() {
        // RED: Parse "RETURN X + 1"
        use crate::tokenizer::tokenize;
        let line = tokenize("RETURN X + 1").unwrap();
        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::Return { value: Some(expr) } => {
                // Verify it's a binary expression
                match expr {
                    Expression::BinaryOp { op, .. } => {
                        assert_eq!(op, BinaryOperator::Add);
                    }
                    _ => panic!("Expected binary expression, got {:?}", expr),
                }
            }
            _ => panic!("Expected Return statement with expression, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_on_goto() {
        // RED: Parse "ON X GOTO 100, 200, 300"
        use crate::tokenizer::tokenize;
        let line = tokenize("ON X GOTO 100, 200, 300").unwrap();
        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::OnGoto {
                expression,
                targets,
                else_part,
            } => {
                assert_eq!(expression, Expression::Variable("X".to_string()));
                assert_eq!(targets, vec![100, 200, 300]);
                assert_eq!(else_part, None);
            }
            _ => panic!("Expected OnGoto statement"),
        }
    }

    #[test]
    fn test_parse_on_gosub() {
        // RED: Parse "ON Y% GOSUB 1000, 2000"
        use crate::tokenizer::tokenize;
        let line = tokenize("ON Y% GOSUB 1000, 2000").unwrap();
        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::OnGosub {
                expression,
                targets,
                else_part,
            } => {
                assert_eq!(expression, Expression::Variable("Y%".to_string()));
                assert_eq!(targets, vec![1000, 2000]);
                assert_eq!(else_part, None);
            }
            _ => panic!("Expected OnGosub statement"),
        }
    }

    fn print_string(text: &str) -> Statement {
        Statement::Print {
            items: vec![PrintItem::Expression(Expression::String(text.to_string()))],
        }
    }

    #[test]
    fn test_parse_on_goto_with_else() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON X% GOTO 100, 200 ELSE PRINT \"bad\"").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![Statement::OnGoto {
                expression: Expression::Variable("X%".to_string()),
                targets: vec![100, 200],
                else_part: Some(vec![print_string("bad")]),
            }]
        );
    }

    #[test]
    fn test_parse_on_gosub_else_part_is_rest_of_line() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON X% GOSUB 100 ELSE PRINT \"a\" : PRINT \"b\"").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![Statement::OnGosub {
                expression: Expression::Variable("X%".to_string()),
                targets: vec![100],
                else_part: Some(vec![print_string("a"), print_string("b")]),
            }]
        );
    }

    #[test]
    fn test_parse_on_goto_with_no_else_ends_at_colon() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON X% GOTO 100 : PRINT \"a\"").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![
                Statement::OnGoto {
                    expression: Expression::Variable("X%".to_string()),
                    targets: vec![100],
                    else_part: None,
                },
                print_string("a"),
            ]
        );
    }

    #[test]
    fn test_parse_on_goto_with_empty_else_is_error() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON X% GOTO 100 ELSE").unwrap();

        assert!(parse_line(&line).is_err());
    }

    #[test]
    fn test_parse_if_then_on_goto_else_keeps_else_in_if() {
        // The run list gives an ON with an index out of range the ELSE of
        // the IF, so the parser keeps the first ELSE after THEN in the IF.
        use crate::tokenizer::tokenize;
        let line = tokenize("IF A% THEN ON X% GOTO 100 ELSE PRINT \"bad\"").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![Statement::If {
                condition: Expression::Variable("A%".to_string()),
                then_part: vec![Statement::OnGoto {
                    expression: Expression::Variable("X%".to_string()),
                    targets: vec![100],
                    else_part: None,
                }],
                else_part: Some(vec![print_string("bad")]),
            }]
        );
    }

    #[test]
    fn test_parse_on_error_statements_use_rest_of_line() {
        use crate::tokenizer::tokenize;
        let line = tokenize("PRINT \"a\" : ON ERROR PRINT \"b\" : END").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![
                print_string("a"),
                Statement::OnErrorStatements {
                    statements: vec![print_string("b"), Statement::End],
                },
            ]
        );
    }

    #[test]
    fn test_parse_on_error_statements_with_if() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON ERROR IF ERR = 18 THEN END ELSE REPORT").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![Statement::OnErrorStatements {
                statements: vec![Statement::If {
                    condition: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("ERR".to_string())),
                        op: BinaryOperator::Equal,
                        right: Box::new(Expression::Integer(18)),
                    },
                    then_part: vec![Statement::End],
                    else_part: Some(vec![Statement::Report]),
                }],
            }]
        );
    }

    #[test]
    fn test_parse_on_error_goto_ends_at_colon() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON ERROR GOTO 100 : PRINT \"a\"").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![Statement::OnError { line_number: 100 }, print_string("a")]
        );
    }

    #[test]
    fn test_parse_on_error_off_ends_at_colon() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON ERROR OFF : PRINT \"a\"").unwrap();

        assert_eq!(
            parse_line(&line).unwrap(),
            vec![Statement::OnErrorOff, print_string("a")]
        );
    }

    #[test]
    fn test_parse_on_error_local_is_error() {
        use crate::tokenizer::tokenize;
        let line = tokenize("ON ERROR LOCAL PRINT \"a\"").unwrap();

        assert!(matches!(
            parse_line(&line),
            Err(BBCBasicError::SyntaxError { message, .. })
                if message == "ON ERROR LOCAL is not supported"
        ));
    }

    fn input_var(name: &str) -> InputItem {
        InputItem::Variable(name.to_string())
    }

    fn input_prompt(text: &str, question_mark: bool) -> InputItem {
        InputItem::Prompt {
            text: text.to_string(),
            question_mark,
        }
    }

    fn parse_input_text(source: &str) -> Statement {
        use crate::tokenizer::tokenize;
        parse_statement(&tokenize(source).unwrap()).unwrap()
    }

    #[test]
    fn test_parse_input() {
        assert_eq!(
            parse_input_text("INPUT A%, B$"),
            Statement::Input {
                items: vec![input_var("A%"), input_var("B$")],
            }
        );
    }

    #[test]
    fn test_parse_input_prompt_with_semicolon_has_question_mark() {
        assert_eq!(
            parse_input_text("INPUT \"Name\";A$"),
            Statement::Input {
                items: vec![input_prompt("Name", true), input_var("A$")],
            }
        );
    }

    #[test]
    fn test_parse_input_prompt_with_comma_has_question_mark() {
        assert_eq!(
            parse_input_text("INPUT \"Name\",A$"),
            Statement::Input {
                items: vec![input_prompt("Name", true), input_var("A$")],
            }
        );
    }

    #[test]
    fn test_parse_input_prompt_with_no_separator_has_no_question_mark() {
        assert_eq!(
            parse_input_text("INPUT \"Name\" A$"),
            Statement::Input {
                items: vec![input_prompt("Name", false), input_var("A$")],
            }
        );
    }

    #[test]
    fn test_parse_input_prompts_can_repeat() {
        assert_eq!(
            parse_input_text("INPUT \"X\", X, \"Y\" Y"),
            Statement::Input {
                items: vec![
                    input_prompt("X", true),
                    input_var("X"),
                    input_prompt("Y", false),
                    input_var("Y"),
                ],
            }
        );
    }

    #[test]
    fn test_parse_input_line_one_variable() {
        assert_eq!(
            parse_input_text("INPUT LINE A$"),
            Statement::InputLine {
                items: vec![input_var("A$")],
            }
        );
    }

    #[test]
    fn test_parse_input_line_two_variables() {
        assert_eq!(
            parse_input_text("INPUT LINE A$, B$"),
            Statement::InputLine {
                items: vec![input_var("A$"), input_var("B$")],
            }
        );
    }

    #[test]
    fn test_parse_input_line_with_prompt() {
        assert_eq!(
            parse_input_text("INPUT LINE \"Text\" A$"),
            Statement::InputLine {
                items: vec![input_prompt("Text", false), input_var("A$")],
            }
        );
    }

    fn input_element(name: &str, indices: Vec<Expression>) -> InputItem {
        InputItem::ArrayElement {
            name: name.to_string(),
            indices,
        }
    }

    #[test]
    fn test_parse_input_numeric_array_element() {
        assert_eq!(
            parse_input_text("INPUT A(1)"),
            Statement::Input {
                items: vec![input_element("A", vec![Expression::Integer(1)])],
            }
        );
    }

    #[test]
    fn test_parse_input_string_array_element_with_variable_index() {
        assert_eq!(
            parse_input_text("INPUT A$(I%), B"),
            Statement::Input {
                items: vec![
                    input_element("A$", vec![Expression::Variable("I%".to_string())]),
                    input_var("B"),
                ],
            }
        );
    }

    #[test]
    fn test_parse_input_two_dimension_element_after_prompt() {
        assert_eq!(
            parse_input_text("INPUT \"Cell\";M(1, 2)"),
            Statement::Input {
                items: vec![
                    input_prompt("Cell", true),
                    input_element("M", vec![Expression::Integer(1), Expression::Integer(2)]),
                ],
            }
        );
    }

    #[test]
    fn test_parse_input_line_array_element() {
        assert_eq!(
            parse_input_text("INPUT LINE L$(2)"),
            Statement::InputLine {
                items: vec![input_element("L$", vec![Expression::Integer(2)])],
            }
        );
    }

    #[test]
    fn test_parse_dim() {
        // RED: Parse "DIM A%(10), B(5, 5)"
        use crate::tokenizer::tokenize;
        let line = tokenize("DIM A%(10), B(5, 5)").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Dim {
                items: vec![
                    DimItem::Array {
                        name: "A%".to_string(),
                        dimensions: vec![Expression::Integer(10)],
                    },
                    DimItem::Array {
                        name: "B".to_string(),
                        dimensions: vec![Expression::Integer(5), Expression::Integer(5)],
                    },
                ],
            }
        );
    }

    fn block(name: &str, size: Expression) -> DimItem {
        DimItem::Block {
            name: name.to_string(),
            size,
        }
    }

    #[test]
    fn test_parse_dim_block() {
        assert_eq!(
            parse_one("DIM B% 10"),
            Statement::Dim {
                items: vec![block("B%", Expression::Integer(10))],
            }
        );
    }

    #[test]
    fn test_parse_dim_two_blocks() {
        assert_eq!(
            parse_one("DIM A% 10, B% N% + 1"),
            Statement::Dim {
                items: vec![
                    block("A%", Expression::Integer(10)),
                    block(
                        "B%",
                        binary(var("N%"), BinaryOperator::Add, Expression::Integer(1))
                    ),
                ],
            }
        );
    }

    #[test]
    fn test_parse_dim_array_and_block() {
        assert_eq!(
            parse_one("DIM A(5), B% 10"),
            Statement::Dim {
                items: vec![
                    DimItem::Array {
                        name: "A".to_string(),
                        dimensions: vec![Expression::Integer(5)],
                    },
                    block("B%", Expression::Integer(10)),
                ],
            }
        );
    }

    #[test]
    fn test_parse_memory_write_with_array_element_base() {
        let element = Expression::ArrayAccess {
            name: "C%".to_string(),
            indices: vec![Expression::Integer(1)],
        };
        let cases = [
            ("C%(1)?2 = 5", Indirection::Byte, Expression::Integer(2)),
            ("C%(1)!4 = 5", Indirection::Word, Expression::Integer(4)),
        ];
        for (source, width, offset) in cases {
            assert_eq!(
                parse_one(source),
                Statement::MemoryAssignment {
                    width,
                    address: binary(element.clone(), BinaryOperator::Add, offset),
                    expression: Expression::Integer(5),
                },
                "{source}"
            );
        }
    }

    #[test]
    fn test_parse_local_array_keeps_brackets() {
        use crate::tokenizer::tokenize;
        let line = tokenize("LOCAL A%(), B, C$()").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Local {
                variables: vec!["A%()".to_string(), "B".to_string(), "C$()".to_string()],
            }
        );
    }

    #[test]
    fn test_parse_end() {
        // RED: Parse "END"
        use crate::tokenizer::tokenize;
        let line = tokenize("END").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(stmt, Statement::End);
    }

    #[test]
    fn test_parse_assignment_with_function() {
        // Test parsing "X% = ABS(-5)"
        use crate::tokenizer::tokenize;
        let line = tokenize("X% = ABS(-5)").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Assignment {
                target: "X%".to_string(),
                expression: Expression::FunctionCall {
                    name: "ABS".to_string(),
                    args: vec![Expression::Integer(-5)],
                },
            }
        );
    }

    #[test]
    fn test_parse_if_then_simple() {
        // RED: Parse "IF X% > 10 THEN PRINT \"Big\""
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"IF X% > 10 THEN PRINT "Big""#).unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::If {
                condition: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Integer(10)),
                },
                then_part: vec![Statement::Print {
                    items: vec![PrintItem::Expression(Expression::String("Big".to_string()))],
                }],
                else_part: None,
            }
        );
    }

    #[test]
    fn test_parse_if_then_goto() {
        // RED: Parse "IF A% = 0 THEN GOTO 100"
        use crate::tokenizer::tokenize;
        let line = tokenize("IF A% = 0 THEN GOTO 100").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::If {
                condition: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("A%".to_string())),
                    op: BinaryOperator::Equal,
                    right: Box::new(Expression::Integer(0)),
                },
                then_part: vec![Statement::Goto { line_number: 100 }],
                else_part: None,
            }
        );
    }

    /// Parse all the statements of one line.
    fn line_statements(source: &str) -> Result<Vec<Statement>> {
        use crate::tokenizer::tokenize;
        parse_line(&tokenize(source).unwrap())
    }

    fn print_integer(value: i32) -> Statement {
        Statement::Print {
            items: vec![PrintItem::Expression(Expression::Integer(value))],
        }
    }

    #[test]
    fn test_parse_if_with_nothing_after_then_is_block_if() {
        assert_eq!(
            line_statements("20 IF X THEN"),
            Ok(vec![Statement::IfBlock {
                condition: Expression::Variable("X".to_string()),
            }])
        );
        assert_eq!(
            line_statements("20 A=1 : IF X THEN"),
            Ok(vec![
                Statement::Assignment {
                    target: "A".to_string(),
                    expression: Expression::Integer(1),
                },
                Statement::IfBlock {
                    condition: Expression::Variable("X".to_string()),
                },
            ])
        );
    }

    #[test]
    fn test_parse_else_line_with_and_without_statements() {
        assert_eq!(line_statements("30 ELSE"), Ok(vec![Statement::BlockElse]));
        assert_eq!(
            line_statements("30 ELSE PRINT 1"),
            Ok(vec![Statement::BlockElse, print_integer(1)])
        );
        assert_eq!(
            line_statements("30 ELSE : PRINT 1 : PRINT 2"),
            Ok(vec![
                Statement::BlockElse,
                print_integer(1),
                print_integer(2)
            ])
        );
    }

    #[test]
    fn test_parse_endif_line_with_and_without_statements() {
        assert_eq!(line_statements("40 ENDIF"), Ok(vec![Statement::EndIf]));
        assert_eq!(
            line_statements("40 ENDIF : PRINT 1"),
            Ok(vec![Statement::EndIf, print_integer(1)])
        );
    }

    #[test]
    fn test_parse_else_not_at_line_start_is_error() {
        assert!(line_statements("40 PRINT 1 : ELSE").is_err());
    }

    #[test]
    fn test_parse_endif_not_at_line_start_is_endif_statement() {
        assert_eq!(
            line_statements("40 PRINT 1 : ENDIF"),
            Ok(vec![print_integer(1), Statement::EndIf])
        );
        assert_eq!(
            line_statements("40 IF X THEN ENDIF"),
            Ok(vec![Statement::If {
                condition: Expression::Variable("X".to_string()),
                then_part: vec![Statement::EndIf],
                else_part: None,
            }])
        );
    }

    #[test]
    fn test_parse_block_if_inside_single_line_if_is_error() {
        assert!(line_statements("20 IF A THEN IF B THEN").is_err());
        assert!(line_statements("20 IF A THEN PRINT 1 ELSE IF B THEN").is_err());
        assert!(line_statements("20 IF A THEN ELSE PRINT 1").is_err());
    }

    /// The message of a syntax error, or panic.
    fn syntax_message(result: Result<Vec<Statement>>) -> String {
        match result {
            Err(BBCBasicError::SyntaxError { message, .. }) => message,
            other => panic!("Expected a syntax error, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_case_of_at_the_end_of_a_line() {
        assert_eq!(
            line_statements("20 CASE X OF"),
            Ok(vec![Statement::Case {
                value: Expression::Variable("X".to_string()),
            }])
        );
        assert_eq!(
            line_statements("20 PRINT 1 : CASE A$ OF"),
            Ok(vec![
                print_integer(1),
                Statement::Case {
                    value: Expression::Variable("A$".to_string()),
                },
            ])
        );
    }

    #[test]
    fn test_parse_case_errors() {
        assert_eq!(
            syntax_message(line_statements("20 CASE X OF : PRINT 1")),
            "CASE must be the last statement of a line"
        );
        assert_eq!(
            syntax_message(line_statements("20 CASE X")),
            "Expected OF after CASE expression"
        );
        assert_eq!(
            syntax_message(line_statements("20 CASE OF")),
            "Expected expression"
        );
        assert_eq!(
            syntax_message(line_statements("20 IF A THEN CASE X OF")),
            "CASE cannot be in an IF branch"
        );
        assert_eq!(
            syntax_message(line_statements("20 IF A THEN PRINT 1 ELSE CASE X OF")),
            "CASE cannot be in an IF branch"
        );
        assert_eq!(
            syntax_message(line_statements("20 CASE X OF PRINT 1")),
            "Expected OF after CASE expression"
        );
    }

    #[test]
    fn test_parse_empty_statement_and_rem_after_of() {
        let case_x = Statement::Case {
            value: Expression::Variable("X".to_string()),
        };
        assert_eq!(
            line_statements("20 CASE X OF :"),
            Ok(vec![case_x.clone(), Statement::Empty])
        );
        assert_eq!(
            line_statements("20 CASE X OF REM pick"),
            Ok(vec![case_x.clone()])
        );
        assert!(matches!(
            line_statements("20 CASE X OF : REM pick").as_deref(),
            Ok([Statement::Case { .. }, Statement::Rem { .. }])
        ));
    }

    #[test]
    fn test_parse_upper_case_words_before_equals_are_assignments() {
        for word in ["WHEN", "OTHERWISE", "ENDCASE", "OF"] {
            let source = format!("10 {}=5", word);
            assert_eq!(
                line_statements(&source),
                Ok(vec![Statement::Assignment {
                    target: word.to_string(),
                    expression: Expression::Integer(5),
                }]),
                "{}",
                source
            );
            let source = format!("10 {}(3)=5", word);
            assert!(
                matches!(
                    line_statements(&source).as_deref(),
                    Ok([Statement::ArrayAssignment { .. }])
                ),
                "{}",
                source
            );
        }
    }

    #[test]
    fn test_parse_case_value_can_be_a_variable_named_of() {
        assert_eq!(
            line_statements("20 CASE OF OF"),
            Ok(vec![Statement::Case {
                value: Expression::Variable("OF".to_string()),
            }])
        );
    }

    #[test]
    fn test_parse_when_line_with_values_and_statements() {
        assert_eq!(
            line_statements("30 WHEN 1"),
            Ok(vec![Statement::When {
                values: vec![Expression::Integer(1)],
            }])
        );
        assert_eq!(
            line_statements("30 WHEN 1, FNa(2, 3), \"x\" : PRINT 1 : PRINT 2"),
            Ok(vec![
                Statement::When {
                    values: vec![
                        Expression::Integer(1),
                        Expression::FunctionCall {
                            name: "FNa".to_string(),
                            args: vec![Expression::Integer(2), Expression::Integer(3)],
                        },
                        Expression::String("x".to_string()),
                    ],
                },
                print_integer(1),
                print_integer(2),
            ])
        );
    }

    #[test]
    fn test_parse_when_errors() {
        assert_eq!(
            syntax_message(line_statements("30 WHEN")),
            "Expected expression"
        );
        assert_eq!(
            syntax_message(line_statements("30 WHEN 1,")),
            "Expected expression"
        );
        assert_eq!(
            syntax_message(line_statements("30 WHEN 1 PRINT 1")),
            "Unexpected PRINT after expression"
        );
        assert_eq!(
            syntax_message(line_statements("30 PRINT 1 : WHEN 1")),
            "WHEN must be the first statement of a line"
        );
    }

    #[test]
    fn test_parse_otherwise_line_with_and_without_statements() {
        assert_eq!(
            line_statements("40 OTHERWISE"),
            Ok(vec![Statement::Otherwise])
        );
        assert_eq!(
            line_statements("40 OTHERWISE PRINT 1"),
            Ok(vec![Statement::Otherwise, print_integer(1)])
        );
        assert_eq!(
            line_statements("40 OTHERWISE : PRINT 1 : PRINT 2"),
            Ok(vec![
                Statement::Otherwise,
                print_integer(1),
                print_integer(2)
            ])
        );
        assert_eq!(
            syntax_message(line_statements("40 PRINT 1 : OTHERWISE")),
            "OTHERWISE must be the first statement of a line"
        );
    }

    #[test]
    fn test_parse_endcase_line_and_endcase_statement() {
        assert_eq!(line_statements("50 ENDCASE"), Ok(vec![Statement::EndCase]));
        assert_eq!(
            line_statements("50 ENDCASE : PRINT 1"),
            Ok(vec![Statement::EndCase, print_integer(1)])
        );
        assert_eq!(
            line_statements("50 PRINT 1 : ENDCASE"),
            Ok(vec![print_integer(1), Statement::EndCase])
        );
        assert_eq!(
            syntax_message(line_statements("50 ENDCASE 1")),
            "Unexpected 1 after ENDCASE"
        );
    }

    /// Parse one IF line and return its THEN part and its ELSE part.
    fn if_branches(source: &str) -> (Vec<Statement>, Option<Vec<Statement>>) {
        use crate::tokenizer::tokenize;
        match parse_statement(&tokenize(source).unwrap()).unwrap() {
            Statement::If {
                then_part,
                else_part,
                ..
            } => (then_part, else_part),
            other => panic!("Expected IF statement, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_if_then_line_number_is_goto() {
        let (then_part, else_part) = if_branches("IF X THEN 30");

        assert_eq!(then_part, vec![Statement::Goto { line_number: 30 }]);
        assert_eq!(else_part, None);
    }

    #[test]
    fn test_parse_if_then_line_number_else_line_number() {
        let (then_part, else_part) = if_branches("IF X THEN 30 ELSE 50");

        assert_eq!(then_part, vec![Statement::Goto { line_number: 30 }]);
        assert_eq!(else_part, Some(vec![Statement::Goto { line_number: 50 }]));
    }

    #[test]
    fn test_parse_if_then_print_else_line_number() {
        let (then_part, else_part) = if_branches(r#"IF X THEN PRINT "A" ELSE 50"#);

        assert_eq!(
            then_part,
            vec![Statement::Print {
                items: vec![PrintItem::Expression(Expression::String("A".to_string()))],
            }]
        );
        assert_eq!(else_part, Some(vec![Statement::Goto { line_number: 50 }]));
    }

    #[test]
    fn test_parse_if_then_line_number_else_print() {
        let (then_part, else_part) = if_branches(r#"IF X THEN 30 ELSE PRINT "B""#);

        assert_eq!(then_part, vec![Statement::Goto { line_number: 30 }]);
        assert_eq!(
            else_part,
            Some(vec![Statement::Print {
                items: vec![PrintItem::Expression(Expression::String("B".to_string()))],
            }])
        );
    }

    #[test]
    fn test_parse_if_then_line_number_then_more_statements_is_goto() {
        let (then_part, _) = if_branches(r#"IF X THEN 30 : PRINT "A""#);

        assert_eq!(then_part, vec![Statement::Goto { line_number: 30 }]);
    }

    /// Parse one numbered IF line and return its error.
    fn if_line_error(source: &str) -> BBCBasicError {
        use crate::tokenizer::tokenize;
        parse_line(&tokenize(source).unwrap()).expect_err("line gives an error")
    }

    fn line_number_error(line: u16) -> BBCBasicError {
        BBCBasicError::SyntaxError {
            message: "Expected end of statement after line number".to_string(),
            line: Some(line),
        }
    }

    #[test]
    fn test_parse_if_then_line_number_with_calculation_is_error() {
        assert_eq!(if_line_error("20 IF X THEN 30+1"), line_number_error(20));
    }

    #[test]
    fn test_parse_if_then_two_line_numbers_is_error() {
        assert_eq!(if_line_error("20 IF X THEN 30 40"), line_number_error(20));
    }

    #[test]
    fn test_parse_if_else_line_number_with_calculation_is_error() {
        assert_eq!(
            if_line_error("20 IF X THEN 30 ELSE 50+1"),
            line_number_error(20)
        );
    }

    #[test]
    fn test_parse_if_else_two_line_numbers_is_error() {
        assert_eq!(
            if_line_error("20 IF X THEN 30 ELSE 50 60"),
            line_number_error(20)
        );
    }

    #[test]
    fn test_parse_if_then_line_number_colon_print_1_is_goto() {
        let (then_part, _) = if_branches("IF X THEN 30 : PRINT 1");

        assert_eq!(then_part, vec![Statement::Goto { line_number: 30 }]);
    }

    #[test]
    fn test_parse_if_else_line_number_colon_print_1_is_goto() {
        let (_, else_part) = if_branches("IF X THEN 30 ELSE 50 : PRINT 1");

        assert_eq!(else_part, Some(vec![Statement::Goto { line_number: 50 }]));
    }

    #[test]
    fn test_parse_if_then_else() {
        // RED: Parse "IF X% > 10 THEN PRINT \"Big\" ELSE PRINT \"Small\""
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"IF X% > 10 THEN PRINT "Big" ELSE PRINT "Small""#).unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::If {
                condition: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Integer(10)),
                },
                then_part: vec![Statement::Print {
                    items: vec![PrintItem::Expression(Expression::String("Big".to_string()))],
                }],
                else_part: Some(vec![Statement::Print {
                    items: vec![PrintItem::Expression(Expression::String(
                        "Small".to_string()
                    ))],
                }]),
            }
        );
    }

    #[test]
    fn test_parse_if_else_in_rem_after_inner_then_is_part_of_the_rem() {
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"IF A% THEN IF B% THEN REM note ELSE PRINT "z""#).unwrap();
        let stmt = parse_statement(&line).unwrap();

        let Statement::If {
            then_part,
            else_part: None,
            ..
        } = stmt
        else {
            panic!("outer IF has an ELSE part: {:?}", stmt);
        };
        assert!(
            matches!(
                then_part.as_slice(),
                [Statement::If {
                    then_part: inner,
                    else_part: None,
                    ..
                }] if matches!(inner.as_slice(), [Statement::Rem { .. }])
            ),
            "inner IF is not THEN REM with no ELSE: {:?}",
            then_part
        );
    }

    #[test]
    fn test_parse_if_then_assignment() {
        // RED: Parse "IF X% < 5 THEN Y% = 0"
        use crate::tokenizer::tokenize;
        let line = tokenize("IF X% < 5 THEN Y% = 0").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::If {
                condition: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::LessThan,
                    right: Box::new(Expression::Integer(5)),
                },
                then_part: vec![Statement::Assignment {
                    target: "Y%".to_string(),
                    expression: Expression::Integer(0),
                }],
                else_part: None,
            }
        );
    }

    #[test]
    fn test_parse_print_file_statement() {
        // Test: PRINT# F%, "Hello"
        let line = TokenizedLine {
            line_number: Some(10),
            tokens: vec![
                Token::Keyword(0xF1),                // PRINT
                Token::Operator('#'),                // #
                Token::Identifier("F%".to_string()), // F%
                Token::Separator(','),               // ,
                Token::String("Hello".to_string()),  // "Hello"
            ],
        };

        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::PrintFile { handle, items } => {
                assert!(matches!(handle, Expression::Variable(_)));
                assert_eq!(items.len(), 1);
                assert!(matches!(
                    items[0],
                    PrintItem::Expression(Expression::String(_))
                ));
            }
            _ => panic!("Expected PrintFile statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_input_file_statement() {
        // Test: INPUT# F%, A%, B$
        let line = TokenizedLine {
            line_number: Some(20),
            tokens: vec![
                Token::Keyword(0xE8),                // INPUT
                Token::Operator('#'),                // #
                Token::Identifier("F%".to_string()), // F%
                Token::Separator(','),               // ,
                Token::Identifier("A%".to_string()), // A%
                Token::Separator(','),               // ,
                Token::Identifier("B$".to_string()), // B$
            ],
        };

        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::InputFile { handle, targets } => {
                assert!(matches!(handle, Expression::Variable(_)));
                assert_eq!(
                    targets,
                    vec![
                        InputItem::Variable("A%".to_string()),
                        InputItem::Variable("B$".to_string()),
                    ]
                );
            }
            _ => panic!("Expected InputFile statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_input_file_array_element_targets() {
        use crate::tokenizer::tokenize;
        let stmt = parse_statement(&tokenize("INPUT#H, V(1), M(I%, 2), X").unwrap()).unwrap();

        assert_eq!(
            stmt,
            Statement::InputFile {
                handle: Expression::Variable("H".to_string()),
                targets: vec![
                    InputItem::ArrayElement {
                        name: "V".to_string(),
                        indices: vec![Expression::Integer(1)],
                    },
                    InputItem::ArrayElement {
                        name: "M".to_string(),
                        indices: vec![
                            Expression::Variable("I%".to_string()),
                            Expression::Integer(2),
                        ],
                    },
                    InputItem::Variable("X".to_string()),
                ],
            }
        );
    }

    #[test]
    fn test_parse_input_file_rejects_tokens_that_are_not_a_target() {
        use crate::tokenizer::tokenize;
        for source in [
            "INPUT#H, V(1) X",
            "INPUT#H, A B",
            "INPUT#H, A; B",
            "INPUT#H, A,",
            "INPUT#H, \"A\"",
        ] {
            let result = parse_statement(&tokenize(source).unwrap());
            assert!(
                matches!(
                    &result,
                    Err(BBCBasicError::SyntaxError { message, .. })
                        if message == "Expected a variable or an array element in INPUT#"
                ),
                "{source}: {result:?}"
            );
        }
    }

    #[test]
    fn test_parse_close_file_statement() {
        // Test: CLOSE# F%
        let line = TokenizedLine {
            line_number: Some(30),
            tokens: vec![
                Token::Keyword(0xD9),                // CLOSE
                Token::Operator('#'),                // #
                Token::Identifier("F%".to_string()), // F%
            ],
        };

        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::CloseFile { handle } => {
                assert!(matches!(handle, Expression::Variable(_)));
            }
            _ => panic!("Expected CloseFile statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_bput_statement() {
        use crate::tokenizer::tokenize;
        let line = tokenize("20 BPUT#F%, 65").unwrap();

        assert_eq!(
            parse_statement(&line).unwrap(),
            Statement::Bput {
                handle: Expression::Variable("F%".to_string()),
                value: Expression::Integer(65),
            }
        );
    }

    #[test]
    fn test_parse_bput_without_comma_is_error() {
        use crate::tokenizer::tokenize;
        let line = tokenize("20 BPUT#F%").unwrap();

        assert!(parse_statement(&line).is_err());
    }

    #[test]
    fn test_parse_ptr_assignment() {
        use crate::tokenizer::tokenize;
        let line = tokenize("30 PTR#F% = 1").unwrap();

        assert_eq!(
            parse_statement(&line).unwrap(),
            Statement::SetPtr {
                handle: Expression::Variable("F%".to_string()),
                position: Expression::Integer(1),
            }
        );
    }

    #[test]
    fn test_parse_ptr_without_equals_is_error() {
        use crate::tokenizer::tokenize;
        let line = tokenize("30 PTR#F%").unwrap();

        assert!(parse_statement(&line).is_err());
    }

    /// Parse `X% = <expression>` and return the expression.
    fn parse_assigned_expression(source: &str) -> Expression {
        use crate::tokenizer::tokenize;
        let line = tokenize(source).unwrap();
        match parse_statement(&line).unwrap() {
            Statement::Assignment { expression, .. } => expression,
            stmt => panic!("Expected Assignment statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_bget_with_channel() {
        assert_eq!(
            parse_assigned_expression("X% = BGET#G%"),
            Expression::FunctionCall {
                name: "BGET".to_string(),
                args: vec![Expression::Variable("G%".to_string())],
            }
        );
    }

    #[test]
    fn test_parse_eof_with_channel() {
        assert_eq!(
            parse_assigned_expression("X% = EOF#G%"),
            Expression::FunctionCall {
                name: "EOF".to_string(),
                args: vec![Expression::Variable("G%".to_string())],
            }
        );
    }

    #[test]
    fn test_parse_eof_with_bracketed_channel() {
        assert_eq!(
            parse_assigned_expression("X% = EOF#(G% + 0)"),
            Expression::FunctionCall {
                name: "EOF".to_string(),
                args: vec![Expression::BinaryOp {
                    left: Box::new(Expression::Variable("G%".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(0)),
                }],
            }
        );
    }

    #[test]
    fn test_parse_ptr_and_ext_with_channel() {
        for name in ["PTR", "EXT"] {
            assert_eq!(
                parse_assigned_expression(&format!("X% = {}#G%", name)),
                Expression::FunctionCall {
                    name: name.to_string(),
                    args: vec![Expression::Variable("G%".to_string())],
                }
            );
        }
    }

    #[test]
    fn test_parse_inkey_and_inkey_string_with_time_limit() {
        for (source, name) in [("X% = INKEY(10)", "INKEY"), ("X$ = INKEY$(10)", "INKEY$")] {
            assert_eq!(
                parse_assigned_expression(source),
                Expression::FunctionCall {
                    name: name.to_string(),
                    args: vec![Expression::Integer(10)],
                },
                "{source}"
            );
        }
    }

    #[test]
    fn test_parse_inkey_with_negative_argument() {
        assert_eq!(
            parse_assigned_expression("X% = INKEY(-99)"),
            Expression::FunctionCall {
                name: "INKEY".to_string(),
                args: vec![parse_assigned_expression("X% = -99")],
            }
        );
    }

    #[test]
    fn test_parse_channel_function_binds_tighter_than_operator() {
        // BGET#G% + 1 is (BGET#G%) + 1, because the channel is a primary.
        assert_eq!(
            parse_assigned_expression("X% = BGET#G% + 1"),
            Expression::BinaryOp {
                left: Box::new(Expression::FunctionCall {
                    name: "BGET".to_string(),
                    args: vec![Expression::Variable("G%".to_string())],
                }),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Integer(1)),
            }
        );
    }

    #[test]
    fn test_parse_openin_function() {
        // Test: F% = OPENIN("test.txt")
        use crate::tokenizer::tokenize;
        let line = tokenize("F% = OPENIN(\"test.txt\")").unwrap();

        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::Assignment { target, expression } => {
                assert_eq!(target, "F%");
                // OPENIN(...) is parsed as FunctionCall
                match &expression {
                    Expression::FunctionCall { name, args } => {
                        assert_eq!(name, "OPENIN");
                        assert_eq!(args.len(), 1);
                    }
                    _ => panic!("Expected FunctionCall expression, got {:?}", expression),
                }
            }
            _ => panic!("Expected Assignment statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_openout_function() {
        // Test: F% = OPENOUT("output.txt")
        use crate::tokenizer::tokenize;
        let line = tokenize("F% = OPENOUT(\"output.txt\")").unwrap();

        let stmt = parse_statement(&line).unwrap();

        match stmt {
            Statement::Assignment { target, expression } => {
                assert_eq!(target, "F%");
                // OPENOUT(...) is parsed as FunctionCall
                match &expression {
                    Expression::FunctionCall { name, args } => {
                        assert_eq!(name, "OPENOUT");
                        assert_eq!(args.len(), 1);
                    }
                    _ => panic!("Expected FunctionCall expression, got {:?}", expression),
                }
            }
            _ => panic!("Expected Assignment statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_quit() {
        // Use the token that the tokenizer gives for QUIT
        let line = crate::tokenizer::tokenize("QUIT").unwrap();
        let stmt = parse_statement(&line).unwrap();
        assert_eq!(stmt, Statement::Quit);
    }

    fn parse_trace(text: &str) -> Result<Statement> {
        parse_statement(&crate::tokenizer::tokenize(text).unwrap())
    }

    #[test]
    fn test_parse_trace_on() {
        assert_eq!(
            parse_trace("TRACE ON").unwrap(),
            Statement::Trace {
                mode: TraceMode::On
            }
        );
    }

    #[test]
    fn test_parse_trace_off() {
        assert_eq!(
            parse_trace("TRACE OFF").unwrap(),
            Statement::Trace {
                mode: TraceMode::Off
            }
        );
    }

    #[test]
    fn test_parse_trace_line_limit() {
        assert_eq!(
            parse_trace("TRACE 100").unwrap(),
            Statement::Trace {
                mode: TraceMode::Below(Expression::Integer(100))
            }
        );
    }

    #[test]
    fn test_parse_trace_with_no_operand_gives_error() {
        assert!(parse_trace("TRACE").is_err());
    }

    #[test]
    fn test_parse_mode_number() {
        assert_eq!(
            parse_trace("MODE 1").unwrap(),
            Statement::Mode {
                mode: Expression::Integer(1)
            }
        );
    }

    #[test]
    fn test_parse_mode_expression() {
        assert_eq!(
            parse_trace("MODE X% + 1").unwrap(),
            Statement::Mode {
                mode: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(1)),
                }
            }
        );
    }

    #[test]
    fn test_parse_mode_with_no_operand_gives_error() {
        assert!(parse_trace("MODE").is_err());
    }

    #[test]
    fn test_parse_colour_number() {
        assert_eq!(
            parse_trace("COLOUR 1").unwrap(),
            Statement::Colour {
                colour: Expression::Integer(1)
            }
        );
    }

    #[test]
    fn test_parse_colour_expression() {
        assert_eq!(
            parse_trace("COLOUR X% + 128").unwrap(),
            Statement::Colour {
                colour: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(128)),
                }
            }
        );
    }

    #[test]
    fn test_parse_colour_with_no_operand_gives_error() {
        assert!(parse_trace("COLOUR").is_err());
    }

    #[test]
    fn test_parse_colour_with_two_operands_gives_error() {
        assert!(parse_trace("COLOUR 1, 2").is_err());
    }

    fn vdu_item(value: Expression, two_bytes: bool) -> VduItem {
        VduItem { value, two_bytes }
    }

    #[test]
    fn test_parse_vdu_comma_items_are_single_bytes() {
        assert_eq!(
            parse_trace("VDU 22, 4").unwrap(),
            Statement::Vdu {
                items: vec![
                    vdu_item(Expression::Integer(22), false),
                    vdu_item(Expression::Integer(4), false),
                ]
            }
        );
    }

    #[test]
    fn test_parse_vdu_semicolon_items_are_two_bytes() {
        assert_eq!(
            parse_trace("VDU 25, 4, 100; 200;").unwrap(),
            Statement::Vdu {
                items: vec![
                    vdu_item(Expression::Integer(25), false),
                    vdu_item(Expression::Integer(4), false),
                    vdu_item(Expression::Integer(100), true),
                    vdu_item(Expression::Integer(200), true),
                ]
            }
        );
    }

    #[test]
    fn test_parse_vdu_comma_in_brackets_does_not_split_the_item() {
        assert_eq!(
            parse_trace("VDU ASC(MID$(A$, 2, 1))").unwrap(),
            Statement::Vdu {
                items: vec![vdu_item(parse_text("ASC(MID$(A$, 2, 1))").unwrap(), false)]
            }
        );
    }

    #[test]
    fn test_parse_vdu_with_no_operand_gives_error() {
        assert!(parse_trace("VDU").is_err());
    }

    #[test]
    fn test_parse_vdu_with_empty_item_gives_error() {
        assert!(parse_trace("VDU 1,,2").is_err());
    }

    /// Tokenize the text and parse it as one expression.
    fn parse_text(text: &str) -> Result<Expression> {
        let line = crate::tokenizer::tokenize(text).unwrap();
        parse_expression(&line.tokens)
    }

    fn binary(left: Expression, op: BinaryOperator, right: Expression) -> Expression {
        Expression::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    #[test]
    fn test_parse_left_shift() {
        assert_eq!(
            parse_text("1 << 4").unwrap(),
            binary(
                Expression::Integer(1),
                BinaryOperator::LeftShift,
                Expression::Integer(4)
            )
        );
    }

    #[test]
    fn test_parse_right_shift() {
        assert_eq!(
            parse_text("256 >> 4").unwrap(),
            binary(
                Expression::Integer(256),
                BinaryOperator::RightShift,
                Expression::Integer(4)
            )
        );
    }

    #[test]
    fn test_parse_comparisons_keep_their_operators() {
        let a = || Expression::Variable("A%".to_string());
        let b = || Expression::Variable("B%".to_string());
        let cases = [
            ("A% < B%", BinaryOperator::LessThan),
            ("A% <= B%", BinaryOperator::LessThanOrEqual),
            ("A% <> B%", BinaryOperator::NotEqual),
            ("A% > B%", BinaryOperator::GreaterThan),
            ("A% >= B%", BinaryOperator::GreaterThanOrEqual),
        ];
        for (text, op) in cases {
            assert_eq!(parse_text(text).unwrap(), binary(a(), op, b()), "{text}");
        }
    }

    #[test]
    fn test_parse_unsigned_right_shift_gives_error() {
        assert!(parse_text("1 >>> 2").is_err());
    }

    fn indirect(width: Indirection, address: Expression) -> Expression {
        Expression::UnaryOp {
            op: UnaryOperator::Indirect(width),
            operand: Box::new(address),
        }
    }

    fn var(name: &str) -> Expression {
        Expression::Variable(name.to_string())
    }

    #[test]
    fn test_parse_unary_memory_reads() {
        assert_eq!(
            parse_text("?A").unwrap(),
            indirect(Indirection::Byte, var("A"))
        );
        assert_eq!(
            parse_text("!A").unwrap(),
            indirect(Indirection::Word, var("A"))
        );
    }

    #[test]
    fn test_parse_binary_memory_reads_add_the_offset() {
        assert_eq!(
            parse_text("A?1").unwrap(),
            indirect(
                Indirection::Byte,
                binary(var("A"), BinaryOperator::Add, Expression::Integer(1))
            )
        );
        assert_eq!(
            parse_text("A!I").unwrap(),
            indirect(
                Indirection::Word,
                binary(var("A"), BinaryOperator::Add, var("I"))
            )
        );
    }

    #[test]
    fn test_memory_operators_bind_more_than_arithmetic() {
        // A?1+2 is (A?1)+2
        assert_eq!(
            parse_text("A?1 + 2").unwrap(),
            binary(
                indirect(
                    Indirection::Byte,
                    binary(var("A"), BinaryOperator::Add, Expression::Integer(1))
                ),
                BinaryOperator::Add,
                Expression::Integer(2)
            )
        );
        // ?A-1 is (?A)-1
        assert_eq!(
            parse_text("?A - 1").unwrap(),
            binary(
                indirect(Indirection::Byte, var("A")),
                BinaryOperator::Subtract,
                Expression::Integer(1)
            )
        );
        // 2^A!0 is 2^(A!0)
        assert_eq!(
            parse_text("2 ^ A!0").unwrap(),
            binary(
                Expression::Integer(2),
                BinaryOperator::Power,
                indirect(
                    Indirection::Word,
                    binary(var("A"), BinaryOperator::Add, Expression::Integer(0))
                )
            )
        );
        // ?(A+1) reads the bracketed address
        assert_eq!(
            parse_text("?(A + 1)").unwrap(),
            indirect(
                Indirection::Byte,
                binary(var("A"), BinaryOperator::Add, Expression::Integer(1))
            )
        );
    }

    fn parse_one(source: &str) -> Statement {
        let line = crate::tokenizer::tokenize(source).unwrap();
        parse_statement(&line).unwrap()
    }

    #[test]
    fn test_parse_memory_writes() {
        let a_plus = |offset: Expression| binary(var("A"), BinaryOperator::Add, offset);
        let cases = [
            ("?A = 5", Indirection::Byte, var("A")),
            ("!A = 5", Indirection::Word, var("A")),
            ("A?1 = 5", Indirection::Byte, a_plus(Expression::Integer(1))),
            ("A!I = 5", Indirection::Word, a_plus(var("I"))),
            ("LET ?A = 5", Indirection::Byte, var("A")),
            (
                "?(A + 1) = 5",
                Indirection::Byte,
                a_plus(Expression::Integer(1)),
            ),
        ];
        for (source, width, address) in cases {
            assert_eq!(
                parse_one(source),
                Statement::MemoryAssignment {
                    width,
                    address,
                    expression: Expression::Integer(5),
                },
                "{source}"
            );
        }
    }

    #[test]
    fn test_memory_write_value_is_a_full_expression() {
        assert_eq!(
            parse_one("?A = B = 1"),
            Statement::MemoryAssignment {
                width: Indirection::Byte,
                address: var("A"),
                expression: binary(var("B"), BinaryOperator::Equal, Expression::Integer(1)),
            }
        );
    }

    #[test]
    fn test_memory_write_without_equals_gives_error() {
        for source in ["?A", "A?1 5", "!A + 1 = 5"] {
            let line = crate::tokenizer::tokenize(source).unwrap();
            assert!(parse_statement(&line).is_err(), "{source}");
        }
    }

    #[test]
    fn test_parse_expression_with_leftover_tokens_gives_error() {
        for text in ["1 2", "1 + 2 3", "(1) (2)", "A B"] {
            let result = parse_text(text);
            assert!(
                matches!(result, Err(BBCBasicError::SyntaxError { .. })),
                "{text}: {result:?}"
            );
        }
    }

    #[test]
    fn test_parse_expression_with_all_tokens_used_gives_no_error() {
        for text in ["1 + 2", "-(1)", "A(1) + B"] {
            assert!(parse_text(text).is_ok(), "{text}");
        }
    }

    fn data_items(source: &str) -> Vec<DataValue> {
        use crate::tokenizer::tokenize;
        match parse_line(&tokenize(source).unwrap()).unwrap().as_slice() {
            [Statement::Data { values }] => values.clone(),
            other => panic!("not one DATA statement: {:?}", other),
        }
    }

    fn texts(items: &[&str]) -> Vec<DataValue> {
        items
            .iter()
            .map(|s| DataValue::String(s.to_string()))
            .collect()
    }

    #[test]
    fn test_parse_data_splits_text_at_commas() {
        assert_eq!(
            data_items("10 DATA 1, -2.5, \"A,B\",  C D , ,\"x\"\"y\""),
            texts(&["1", "-2.5", "A,B", "C D ", "", "x\"y"])
        );
    }

    #[test]
    fn test_parse_data_colon_and_rem_are_part_of_one_item() {
        assert_eq!(data_items("10 DATA 1 : REM x"), texts(&["1 : REM x"]));
    }

    #[test]
    fn test_parse_data_with_no_text_gives_no_items() {
        assert_eq!(data_items("10 DATA"), texts(&[]));
    }

    #[test]
    fn test_parse_data_after_then_keeps_else_in_the_data() {
        use crate::tokenizer::tokenize;
        let line = tokenize("IF A% THEN PRINT \"y\" : DATA 1,2 ELSE PRINT \"z\"").unwrap();
        let Statement::If {
            then_part,
            else_part,
            ..
        } = parse_statement(&line).unwrap()
        else {
            panic!("not an IF statement");
        };
        assert_eq!(else_part, None);
        assert_eq!(
            then_part[1],
            Statement::Data {
                values: texts(&["1", "2 ELSE PRINT \"z\""])
            }
        );
    }

    #[test]
    fn test_parse_stereo_gives_channel_and_position() {
        use crate::tokenizer::tokenize;
        let stmt = parse_statement(&tokenize("STEREO 1, -50").unwrap()).unwrap();

        assert_eq!(
            stmt,
            Statement::Stereo {
                channel: Expression::Integer(1),
                position: Expression::Integer(-50),
            }
        );
    }

    #[test]
    fn test_parse_stereo_with_wrong_argument_count_gives_syntax_error() {
        use crate::tokenizer::tokenize;
        for source in ["STEREO", "STEREO 1", "STEREO 1, 2, 3"] {
            let result = parse_statement(&tokenize(source).unwrap());
            assert!(
                matches!(result, Err(BBCBasicError::SyntaxError { .. })),
                "{source} gave {result:?}"
            );
        }
    }

    #[test]
    fn test_parse_adval_gives_function_call_with_one_argument() {
        // ADVAL 0x96
        let tokens = [
            Token::Keyword(0x96),
            Token::Separator('('),
            Token::Integer(1),
            Token::Separator(')'),
        ];
        let expr = parse_expression(&tokens).unwrap();

        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "ADVAL".to_string(),
                args: vec![Expression::Integer(1)],
            }
        );
    }

    #[test]
    fn test_split_at_commas_skips_a_comma_in_brackets() {
        let tokens = crate::tokenizer::tokenize("P(1,1), FNa(2, (3, 4)), 5")
            .unwrap()
            .tokens;

        let parts = split_at_commas(&tokens);

        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], &tokens[..6]);
        assert_eq!(parts[2], [Token::Integer(5)]);
    }

    #[test]
    fn test_split_at_commas_skips_a_comma_in_a_string() {
        let tokens = crate::tokenizer::tokenize("\"a,b\", 1").unwrap().tokens;

        let parts = split_at_commas(&tokens);

        assert_eq!(
            parts,
            vec![
                &[Token::String("a,b".to_string())][..],
                &[Token::Integer(1)][..]
            ]
        );
    }

    #[test]
    fn test_split_at_commas_gives_one_part_with_no_comma() {
        let tokens = crate::tokenizer::tokenize("1 + 2").unwrap().tokens;

        assert_eq!(split_at_commas(&tokens), vec![&tokens[..]]);
    }

    #[test]
    fn test_comma_separated_expressions_take_an_array_element() {
        let tokens = crate::tokenizer::tokenize("P(1,1), 100").unwrap().tokens;

        let expressions = parse_comma_separated_expressions(&tokens, Some(20)).unwrap();

        assert_eq!(expressions.len(), 2);
        assert!(matches!(expressions[0], Expression::ArrayAccess { .. }));
        assert_eq!(expressions[1], Expression::Integer(100));
    }

    #[test]
    fn test_comma_separated_expressions_reject_an_empty_value() {
        for source in ["100,,200", "100,", ",1", "69,,"] {
            let tokens = crate::tokenizer::tokenize(source).unwrap().tokens;

            let result = parse_comma_separated_expressions(&tokens, Some(20));

            assert_eq!(
                result,
                Err(BBCBasicError::SyntaxError {
                    message: "Missing value at a comma".to_string(),
                    line: Some(20),
                }),
                "{}",
                source
            );
        }
    }

    #[test]
    fn test_comma_separated_expressions_keep_the_error_for_no_tokens() {
        let result = parse_comma_separated_expressions(&[], Some(20));

        assert_eq!(
            result,
            Err(BBCBasicError::SyntaxError {
                message: "Expected at least one expression".to_string(),
                line: Some(20),
            })
        );
    }

    #[test]
    fn test_parse_read_variables() {
        assert_eq!(
            parse_input_text("READ A, B$, C%"),
            Statement::Read {
                variables: vec![input_var("A"), input_var("B$"), input_var("C%")],
            }
        );
    }

    #[test]
    fn test_parse_read_array_elements() {
        assert_eq!(
            parse_input_text("READ Q(1,2), N$(I), X"),
            Statement::Read {
                variables: vec![
                    input_element("Q", vec![Expression::Integer(1), Expression::Integer(2)]),
                    input_element("N$", vec![Expression::Variable("I".to_string())]),
                    input_var("X"),
                ],
            }
        );
    }

    #[test]
    fn test_parse_read_target_that_is_not_a_variable_is_an_error() {
        use crate::tokenizer::tokenize;
        for source in ["READ 5", "READ A + 1", "READ A(1) B", "READ \"x\"", "READ"] {
            assert!(
                parse_statement(&tokenize(source).unwrap()).is_err(),
                "{}",
                source
            );
        }
    }

    #[test]
    fn test_parse_for_each_statement() {
        assert_eq!(
            line_statements("10 FOR EACH S$ IN L%"),
            Ok(vec![Statement::ForEach {
                variable: "S$".to_string(),
                collection: Expression::Variable("L%".to_string()),
            }])
        );
    }

    #[test]
    fn test_parse_for_each_named_variable_is_a_normal_for() {
        assert!(matches!(
            line_statements("10 FOR EACH = 1 TO 3").as_deref(),
            Ok([Statement::For { variable, .. }]) if variable == "EACH"
        ));
    }

    #[test]
    fn test_parse_next_each_is_a_next_with_the_name_each() {
        assert_eq!(
            line_statements("10 NEXT EACH"),
            Ok(vec![Statement::Next {
                variables: vec!["EACH".to_string()],
            }])
        );
    }

    #[test]
    fn test_parse_for_each_errors() {
        assert_eq!(
            syntax_message(line_statements("10 FOR EACH N L")),
            "Expected IN in FOR EACH statement"
        );
        assert_eq!(
            syntax_message(line_statements("10 FOR EACH N IN")),
            "Expected a collection after IN"
        );
        assert_eq!(
            syntax_message(line_statements("10 FOR EACH 5 IN L")),
            "Expected variable name after FOR EACH"
        );
    }
}
