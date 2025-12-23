//! Parser for BBC BASIC statements and expressions
//!
//! Analyzes tokenized BBC BASIC statements and creates abstract syntax trees
//! for execution.

use crate::error::BBCBasicError;
use crate::error::Result;
use crate::tokenizer::{create_reverse_keyword_maps, Token, TokenizedLine};

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
    Tab(Expression), // TAB(n)
    Spc(Expression), // SPC(n)
    Semicolon,       // ;
    Comma,           // ,
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
    /// PRINT statement
    Print { items: Vec<PrintItem> },
    /// INPUT statement
    Input { variables: Vec<String> },
    /// FOR loop
    For {
        variable: String,
        start: Expression,
        end: Expression,
        step: Option<Expression>,
    },
    /// NEXT statement
    Next { variables: Vec<String> },
    /// IF statement
    If {
        condition: Expression,
        then_part: Vec<Statement>,
        else_part: Option<Vec<Statement>>,
    },
    /// GOTO statement
    Goto { line_number: u16 },
    /// GOSUB statement
    Gosub { line_number: u16 },
    /// RETURN statement (optionally with value for functions)
    Return { value: Option<Expression> },
    /// DIM statement for array dimensioning
    Dim {
        arrays: Vec<(String, Vec<Expression>)>,
    },
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
    /// DEF FN - define a function (single-line with return expression)
    DefFn {
        name: String,
        params: Vec<String>,
        expression: Expression,
    },
    /// ENDPROC - end procedure definition
    EndProc,
    /// LOCAL statement - declares local variables in a procedure
    Local { variables: Vec<String> },
    /// DATA statement - stores data values
    Data { values: Vec<DataValue> },
    /// READ statement - reads data into variables
    Read { variables: Vec<String> },
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
    /// ON GOTO statement - computed GOTO based on expression value
    OnGoto {
        expression: Expression,
        targets: Vec<u16>,
    },
    /// ON GOSUB statement - computed GOSUB based on expression value
    OnGosub {
        expression: Expression,
        targets: Vec<u16>,
    },
    /// ON ERROR GOTO statement - set error handler
    OnError { line_number: u16 },
    /// ON ERROR OFF statement - clear error handler
    OnErrorOff,
    /// PRINT# statement - write to file
    PrintFile {
        handle: Expression,
        items: Vec<PrintItem>,
    },
    /// INPUT# statement - read from file
    InputFile {
        handle: Expression,
        variables: Vec<String>,
    },
    /// CLOSE# statement - close file
    CloseFile { handle: Expression },
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
    /// CIRCLE statement - draw a circle
    Circle {
        x: Expression,
        y: Expression,
        radius: Expression,
    },
    /// GCOL statement - set graphics color
    Gcol { mode: Expression, color: Expression },
    /// CLG statement - clear graphics screen
    Clg,
    /// ELLIPSE statement - draw an ellipse
    Ellipse {
        x: Expression,
        y: Expression,
        major: Expression,
        minor: Expression,
    },
    /// RECTANGLE statement - draw a rectangle
    Rectangle {
        x1: Expression,
        y1: Expression,
        width: Expression,
        height: Expression,
        filled: bool,
    },
    /// FILL statement - flood fill from coordinates
    Fill { x: Expression, y: Expression },
    /// Empty statement
    Empty,
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
                UnaryOperator::Not => ExpressionType::Integer,
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

/// Parse a tokenized line into a statement
pub fn parse_statement(line: &TokenizedLine) -> Result<Statement> {
    let tokens = &line.tokens;

    if tokens.is_empty() {
        return Ok(Statement::Empty);
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

        // Variable assignment (without LET keyword)
        Token::Identifier(_) => parse_assignment(tokens, line.line_number),

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
            } else {
                parse_input_statement(&tokens[1..])
            }
        }

        // DIM statement
        Token::Keyword(0xDE) => parse_dim_statement(&tokens[1..], line.line_number),

        // IF statement
        Token::Keyword(0xE7) => parse_if_statement(&tokens[1..], line.line_number),

        // END statement
        Token::Keyword(0xE0) => Ok(Statement::End),

        // STOP statement
        Token::Keyword(0xFA) => Ok(Statement::Stop),

        // QUIT statement
        Token::Keyword(0x98) => Ok(Statement::Quit),

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

        // DEF statement (DEF PROC or DEF FN)
        Token::Keyword(0xDD) => parse_def_statement(&tokens[1..], line.line_number),

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

        // Extended statements (0xC8 prefix)
        Token::ExtendedKeyword(0xC8, extended_token) => match extended_token {
            // WHILE statement
            0x95 => parse_while_statement(&tokens[1..], line.line_number),
            // ENDWHILE statement
            0xA4 => Ok(Statement::EndWhile),
            // CIRCLE statement
            0x8F => parse_circle_statement(&tokens[1..], line.line_number),
            // FILL statement
            0x90 => parse_fill_statement(&tokens[1..], line.line_number),
            // RECTANGLE statement
            0x93 => parse_rectangle_statement(&tokens[1..], line.line_number),
            // ELLIPSE statement
            0x9D => parse_ellipse_statement(&tokens[1..], line.line_number),
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown extended statement: {:?}", tokens[0]),
                line: line.line_number,
            }),
        },

        _ => Err(BBCBasicError::SyntaxError {
            message: format!("Unknown statement: {:?}", tokens[0]),
            line: line.line_number,
        }),
    }
}

/// Parse PRINT statement
fn parse_print_statement(tokens: &[Token]) -> Result<Statement> {
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

                let expr = parse_expression(&tokens[start_pos..pos - 1])?;
                items.push(PrintItem::Tab(expr));
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
                        Token::Separator(';') | Token::Separator(',') if paren_depth == 0 => {
                            break;
                        }
                        _ => {
                            end_pos += 1;
                        }
                    }
                }

                if end_pos > start_pos {
                    let expr = parse_expression(&tokens[start_pos..end_pos])?;
                    items.push(PrintItem::Expression(expr));
                    pos = end_pos;
                } else {
                    break;
                }
            }
        }
    }

    Ok(Statement::Print { items })
}

/// Parse assignment statement (A% = 42 or LET A% = 42)
fn parse_assignment(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
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

    if !matches!(tokens[1], Token::Operator('=')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected '='".to_string(),
            line: line_number,
        });
    }

    let expression = parse_expression(&tokens[2..])?;

    Ok(Statement::Assignment { target, expression })
}

/// Parse FOR statement
fn parse_for_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
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

/// Parse GOTO statement
fn parse_goto_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected line number after GOTO".to_string(),
            line: line_number,
        });
    }

    let line_num = match &tokens[0] {
        Token::Integer(n) => *n as u16,
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected line number after GOTO".to_string(),
                line: line_number,
            })
        }
    };

    Ok(Statement::Goto {
        line_number: line_num,
    })
}

/// Parse GOSUB statement
fn parse_gosub_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected line number after GOSUB".to_string(),
            line: line_number,
        });
    }

    let line_num = match &tokens[0] {
        Token::Integer(n) => *n as u16,
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected line number after GOSUB".to_string(),
                line: line_number,
            })
        }
    };

    Ok(Statement::Gosub {
        line_number: line_num,
    })
}

/// Parse ON statement (ON GOTO or ON GOSUB)
fn parse_on_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Syntax: ON <expression> GOTO|GOSUB <line1>, <line2>, ...
    // or: ON ERROR GOTO <line>
    // or: ON ERROR OFF

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
            return Err(BBCBasicError::SyntaxError {
                message: "Expected GOTO or OFF after ON ERROR".to_string(),
                line: line_number,
            });
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
                    Token::Integer(n) => {
                        return Ok(Statement::OnError {
                            line_number: n as u16,
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
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected GOTO or OFF after ON ERROR".to_string(),
                    line: line_number,
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

    // Parse line numbers after GOTO/GOSUB
    let line_tokens = &tokens[keyword_pos + 1..];
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
            Token::Integer(n) => {
                targets.push(*n as u16);
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
        })
    } else {
        Ok(Statement::OnGosub {
            expression,
            targets,
        })
    }
}

/// Parse INPUT statement
fn parse_input_statement(tokens: &[Token]) -> Result<Statement> {
    let mut variables = Vec::new();
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

    Ok(Statement::Input { variables })
}

/// Parse PRINT# statement (file I/O)
fn parse_print_file_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // First token should be the file handle expression
    // Format: PRINT# handle, items...
    
    // Find the comma that separates handle from print items
    let comma_pos = tokens.iter().position(|t| matches!(t, Token::Separator(',')));
    
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
    
    Ok(Statement::PrintFile { handle, items })
}

/// Parse INPUT# statement (file I/O)
fn parse_input_file_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Format: INPUT# handle, var1, var2, ...
    
    // Find the comma that separates handle from variables
    let comma_pos = tokens.iter().position(|t| matches!(t, Token::Separator(',')));
    
    if comma_pos.is_none() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected comma after file handle in INPUT#".to_string(),
            line: line_number,
        });
    }
    
    let comma_pos = comma_pos.unwrap();
    
    // Parse handle expression
    let handle = parse_expression(&tokens[..comma_pos])?;
    
    // Parse variable list after the comma
    let mut variables = Vec::new();
    let mut pos = comma_pos + 1;
    
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
    
    if variables.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected at least one variable in INPUT#".to_string(),
            line: line_number,
        });
    }
    
    Ok(Statement::InputFile { handle, variables })
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
            message: format!("PLOT requires 3 parameters (mode, x, y), got {}", args.len()),
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

/// Parse CIRCLE statement: CIRCLE x, y, radius
fn parse_circle_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "CIRCLE requires x, y, radius parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 3 {
        return Err(BBCBasicError::SyntaxError {
            message: format!("CIRCLE requires 3 parameters (x, y, radius), got {}", args.len()),
            line: line_number,
        });
    }

    Ok(Statement::Circle {
        x: args[0].clone(),
        y: args[1].clone(),
        radius: args[2].clone(),
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
            message: format!("GCOL requires 2 parameters (mode, color), got {}", args.len()),
            line: line_number,
        });
    }

    Ok(Statement::Gcol {
        mode: args[0].clone(),
        color: args[1].clone(),
    })
}

/// Parse ELLIPSE statement: ELLIPSE x, y, major, minor
fn parse_ellipse_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
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
    })
}

/// Parse RECTANGLE statement: RECTANGLE x1, y1, width, height
fn parse_rectangle_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "RECTANGLE requires x1, y1, width, height parameters".to_string(),
            line: line_number,
        });
    }

    let args = parse_comma_separated_expressions(tokens, line_number)?;

    if args.len() != 4 {
        return Err(BBCBasicError::SyntaxError {
            message: format!(
                "RECTANGLE requires 4 parameters (x1, y1, width, height), got {}",
                args.len()
            ),
            line: line_number,
        });
    }

    Ok(Statement::Rectangle {
        x1: args[0].clone(),
        y1: args[1].clone(),
        width: args[2].clone(),
        height: args[3].clone(),
        filled: true, // BBC BASIC RECTANGLE draws filled rectangles
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

/// Helper function to parse comma-separated expressions
fn parse_comma_separated_expressions(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Vec<Expression>> {
    let mut expressions = Vec::new();
    let mut start = 0;
    let mut pos = 0;

    while pos <= tokens.len() {
        // Check if we hit a comma or end of tokens
        if pos == tokens.len() || matches!(tokens[pos], Token::Separator(',')) {
            if start < pos {
                // Parse the expression between commas
                let expr = parse_expression(&tokens[start..pos])?;
                expressions.push(expr);
            }

            if pos < tokens.len() {
                // Skip the comma
                pos += 1;
                start = pos;
            } else {
                break;
            }
        } else {
            pos += 1;
        }
    }

    if expressions.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected at least one expression".to_string(),
            line: line_number,
        });
    }

    Ok(expressions)
}

/// Helper function to parse print items (extracted for reuse)
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
                
                // Find matching closing paren
                let mut paren_depth = 1;
                let start = pos;
                while pos < tokens.len() && paren_depth > 0 {
                    match tokens[pos] {
                        Token::Separator('(') => paren_depth += 1,
                        Token::Separator(')') => paren_depth -= 1,
                        _ => {}
                    }
                    if paren_depth > 0 {
                        pos += 1;
                    }
                }
                
                if paren_depth != 0 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Unmatched parenthesis in TAB".to_string(),
                        line: None,
                    });
                }
                
                let expr = parse_expression(&tokens[start..pos])?;
                items.push(PrintItem::Tab(expr));
                pos += 1; // skip ')'
            }
            // Handle SPC(expr)
            Token::Keyword(0xB7) => {
                pos += 1; // skip SPC keyword
                if pos >= tokens.len() || !matches!(tokens[pos], Token::Separator('(')) {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected '(' after SPC".to_string(),
                        line: None,
                    });
                }
                pos += 1; // skip '('
                
                // Find matching closing paren
                let mut paren_depth = 1;
                let start = pos;
                while pos < tokens.len() && paren_depth > 0 {
                    match tokens[pos] {
                        Token::Separator('(') => paren_depth += 1,
                        Token::Separator(')') => paren_depth -= 1,
                        _ => {}
                    }
                    if paren_depth > 0 {
                        pos += 1;
                    }
                }
                
                if paren_depth != 0 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Unmatched parenthesis in SPC".to_string(),
                        line: None,
                    });
                }
                
                let expr = parse_expression(&tokens[start..pos])?;
                items.push(PrintItem::Spc(expr));
                pos += 1; // skip ')'
            }
            _ => {
                // Find the next separator (, or ;) or end of tokens
                let next_sep = tokens[pos..]
                    .iter()
                    .position(|t| matches!(t, Token::Separator(',') | Token::Separator(';')))
                    .map(|p| p + pos)
                    .unwrap_or(tokens.len());
                
                let expr = parse_expression(&tokens[pos..next_sep])?;
                items.push(PrintItem::Expression(expr));
                pos = next_sep;
            }
        }
    }

    Ok(items)
}

/// Parse DIM statement
fn parse_dim_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let mut arrays = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        // Get array name
        let name = match &tokens[pos] {
            Token::Identifier(n) => {
                // Array names have opening paren
                format!("{}(", n.trim_end_matches('('))
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected array name in DIM".to_string(),
                    line: line_number,
                })
            }
        };
        pos += 1;

        // Expect opening paren
        if pos >= tokens.len() || !matches!(tokens[pos], Token::Separator('(')) {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected '(' after array name".to_string(),
                line: line_number,
            });
        }
        pos += 1;

        // Parse dimension expressions
        let mut dimensions = Vec::new();
        loop {
            // Find the extent of this dimension (until comma or closing paren)
            let start = pos;
            let mut depth = 0;
            while pos < tokens.len() {
                match &tokens[pos] {
                    Token::Separator('(') => depth += 1,
                    Token::Separator(')') if depth == 0 => break,
                    Token::Separator(')') => depth -= 1,
                    Token::Separator(',') if depth == 0 => break,
                    _ => {}
                }
                pos += 1;
            }

            if pos > start {
                let dim_expr = parse_expression(&tokens[start..pos])?;
                dimensions.push(dim_expr);
            }

            if pos >= tokens.len() {
                break;
            }

            match &tokens[pos] {
                Token::Separator(',') => pos += 1, // next dimension
                Token::Separator(')') => {
                    pos += 1; // end of this array
                    break;
                }
                _ => break,
            }
        }

        arrays.push((name, dimensions));

        // Check for comma (multiple arrays in one DIM)
        if pos < tokens.len() && matches!(tokens[pos], Token::Separator(',')) {
            pos += 1;
        } else {
            break;
        }
    }

    Ok(Statement::Dim { arrays })
}

/// Parse DATA statement
/// Supports: DATA value1, value2, value3, ...
fn parse_data_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    let mut values = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        // Skip commas
        if matches!(tokens[pos], Token::Separator(',')) {
            pos += 1;
            continue;
        }

        // Parse value
        match &tokens[pos] {
            Token::Integer(val) => {
                values.push(DataValue::Integer(*val));
                pos += 1;
            }
            Token::Real(val) => {
                values.push(DataValue::Real(*val));
                pos += 1;
            }
            Token::String(val) => {
                values.push(DataValue::String(val.clone()));
                pos += 1;
            }
            Token::Operator('-') if pos + 1 < tokens.len() => {
                // Handle negative numbers
                pos += 1;
                match &tokens[pos] {
                    Token::Integer(val) => {
                        values.push(DataValue::Integer(-val));
                        pos += 1;
                    }
                    Token::Real(val) => {
                        values.push(DataValue::Real(-val));
                        pos += 1;
                    }
                    _ => {
                        return Err(BBCBasicError::SyntaxError {
                            message: "Expected number after minus in DATA".to_string(),
                            line: line_number,
                        });
                    }
                }
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: format!("Invalid DATA value: {:?}", tokens[pos]),
                    line: line_number,
                });
            }
        }
    }

    Ok(Statement::Data { values })
}

/// Parse READ statement
/// Supports: READ var1, var2, var3, ...
fn parse_read_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
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
                variables.push(name.clone());
                pos += 1;
            }
            _ => {
                return Err(BBCBasicError::SyntaxError {
                    message: "Expected variable name in READ".to_string(),
                    line: line_number,
                });
            }
        }
    }

    if variables.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "READ requires at least one variable".to_string(),
            line: line_number,
        });
    }

    Ok(Statement::Read { variables })
}

/// Parse RESTORE statement
/// Supports: RESTORE [line_number]
fn parse_restore_statement(tokens: &[Token], _line_number: Option<u16>) -> Result<Statement> {
    if tokens.is_empty() {
        // RESTORE with no line number - reset to beginning
        Ok(Statement::Restore { line_number: None })
    } else if tokens.len() == 1 {
        // RESTORE with line number
        match &tokens[0] {
            Token::Integer(num) => Ok(Statement::Restore {
                line_number: Some(*num as u16),
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

    // Extract function name (identifier)
    let name = match &tokens[0] {
        Token::Identifier(n) => n.clone(),
        _ => {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected identifier for function name".to_string(),
                line: line_number,
            })
        }
    };

    // Parse parameters if present
    let (params, rest_start) = if tokens.len() > 1 && matches!(tokens[1], Token::Operator('(')) {
        // Find closing parenthesis
        let close_pos = tokens
            .iter()
            .skip(1)
            .position(|t| matches!(t, Token::Operator(')')))
            .ok_or(BBCBasicError::SyntaxError {
                message: "Expected ) after parameter list".to_string(),
                line: line_number,
            })?
            + 1;

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

    // Expect = after parameters
    if rest_start >= tokens.len() || !matches!(tokens[rest_start], Token::Operator('=')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected = after function parameters".to_string(),
            line: line_number,
        });
    }

    // Parse the expression after =
    let expression = parse_expression(&tokens[rest_start + 1..])?;

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
                variables.push(name.clone());
                pos += 1;
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

/// Parse argument list: (expr1, expr2, ...)
fn parse_argument_list(tokens: &[Token], line_number: Option<u16>) -> Result<Vec<Expression>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Expect opening parenthesis
    if !matches!(tokens[0], Token::Operator('(')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected ( after procedure name".to_string(),
            line: line_number,
        });
    }

    // Find closing parenthesis
    let close_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Operator(')')))
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
            Token::Operator('(') => depth += 1,
            Token::Operator(')') => depth -= 1,
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
    if !matches!(tokens[0], Token::Operator('(')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected ( after procedure name".to_string(),
            line: line_number,
        });
    }

    // Find closing parenthesis
    let close_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Operator(')')))
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
/// Supports: IF condition THEN statement [ELSE statement]
fn parse_if_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Find THEN keyword to split condition from then-part
    let then_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Keyword(0x8C)))
        .ok_or(BBCBasicError::SyntaxError {
            message: "Expected THEN after IF condition".to_string(),
            line: line_number,
        })?;

    // Parse condition (everything before THEN)
    let condition_tokens = &tokens[..then_pos];
    let condition = parse_expression(condition_tokens)?;

    // Find ELSE keyword (if present)
    let else_pos = tokens[then_pos + 1..]
        .iter()
        .position(|t| matches!(t, Token::Keyword(0x8B)));

    let (then_tokens, else_tokens) = if let Some(else_idx) = else_pos {
        // ELSE found: split then_part and else_part
        let absolute_else_pos = then_pos + 1 + else_idx;
        (
            &tokens[then_pos + 1..absolute_else_pos],
            Some(&tokens[absolute_else_pos + 1..]),
        )
    } else {
        // No ELSE: only then_part
        (&tokens[then_pos + 1..], None)
    };

    // Parse THEN part (single statement for now)
    let then_part = if then_tokens.is_empty() {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected statement after THEN".to_string(),
            line: line_number,
        });
    } else {
        // Create a temporary TokenizedLine for parsing
        let then_line = TokenizedLine::new(line_number, then_tokens.to_vec());
        vec![parse_statement(&then_line)?]
    };

    // Parse ELSE part if present
    let else_part = if let Some(else_toks) = else_tokens {
        if else_toks.is_empty() {
            return Err(BBCBasicError::SyntaxError {
                message: "Expected statement after ELSE".to_string(),
                line: line_number,
            });
        }
        let else_line = TokenizedLine::new(line_number, else_toks.to_vec());
        Some(vec![parse_statement(&else_line)?])
    } else {
        None
    };

    Ok(Statement::If {
        condition,
        then_part,
        else_part,
    })
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
    parse_expr_precedence(tokens, &mut pos, 0)
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
        let (prec, op) = match &tokens[*pos] {
            Token::Operator(ch) => {
                if let Some(p) = get_precedence(*ch) {
                    if let Some(binary_op) = char_to_binary_op(*ch) {
                        (p, binary_op)
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
                        (p, binary_op)
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

        *pos += 1; // consume operator

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

/// Parse a primary expression (literal, variable, function call, or parenthesized expression)
fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<Expression> {
    if *pos >= tokens.len() {
        return Err(BBCBasicError::SyntaxError {
            message: "Unexpected end of expression".to_string(),
            line: None,
        });
    }

    let token = &tokens[*pos];

    match token {
        // Literals
        Token::Integer(val) => {
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

        // Variables
        Token::Identifier(name) => {
            *pos += 1;
            Ok(Expression::Variable(name.clone()))
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
            } else {
                // It's a constant or keyword used as value
                Ok(Expression::Variable(keyword))
            }
        }

        _ => Err(BBCBasicError::SyntaxError {
            message: format!("Unexpected token in expression: {:?}", token),
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
            } => {
                assert_eq!(expression, Expression::Variable("X".to_string()));
                assert_eq!(targets, vec![100, 200, 300]);
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
            } => {
                assert_eq!(expression, Expression::Variable("Y%".to_string()));
                assert_eq!(targets, vec![1000, 2000]);
            }
            _ => panic!("Expected OnGosub statement"),
        }
    }

    #[test]
    fn test_parse_input() {
        // RED: Parse "INPUT A%, B$"
        use crate::tokenizer::tokenize;
        let line = tokenize("INPUT A%, B$").unwrap();
        let stmt = parse_statement(&line).unwrap();

        assert_eq!(
            stmt,
            Statement::Input {
                variables: vec!["A%".to_string(), "B$".to_string()],
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
                arrays: vec![
                    ("A%(".to_string(), vec![Expression::Integer(10)]),
                    (
                        "B(".to_string(),
                        vec![Expression::Integer(5), Expression::Integer(5)]
                    ),
                ],
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
                Token::Keyword(0xF1),              // PRINT
                Token::Operator('#'),              // #
                Token::Identifier("F%".to_string()),  // F%
                Token::Separator(','),             // ,
                Token::String("Hello".to_string()), // "Hello"
            ],
        };
        
        let stmt = parse_statement(&line).unwrap();
        
        match stmt {
            Statement::PrintFile { handle, items } => {
                assert!(matches!(handle, Expression::Variable(_)));
                assert_eq!(items.len(), 1);
                assert!(matches!(items[0], PrintItem::Expression(Expression::String(_))));
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
                Token::Keyword(0xE8),              // INPUT
                Token::Operator('#'),              // #
                Token::Identifier("F%".to_string()),  // F%
                Token::Separator(','),             // ,
                Token::Identifier("A%".to_string()),  // A%
                Token::Separator(','),             // ,
                Token::Identifier("B$".to_string()),  // B$
            ],
        };
        
        let stmt = parse_statement(&line).unwrap();
        
        match stmt {
            Statement::InputFile { handle, variables } => {
                assert!(matches!(handle, Expression::Variable(_)));
                assert_eq!(variables.len(), 2);
                assert_eq!(variables[0], "A%");
                assert_eq!(variables[1], "B$");
            }
            _ => panic!("Expected InputFile statement, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_close_file_statement() {
        // Test: CLOSE# F%
        let line = TokenizedLine {
            line_number: Some(30),
            tokens: vec![
                Token::Keyword(0xD9),              // CLOSE
                Token::Operator('#'),              // #
                Token::Identifier("F%".to_string()),  // F%
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
        // RED: Test that QUIT is parsed correctly
        let line = TokenizedLine::new(None, vec![Token::Keyword(0x98)]); // QUIT token
        let stmt = parse_statement(&line).unwrap();
        assert_eq!(stmt, Statement::Quit);
    }
}
