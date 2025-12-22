//! Parser for BBC BASIC statements and expressions
//! 
//! Analyzes tokenized BBC BASIC statements and creates abstract syntax trees
//! for execution.

use crate::error::Result;
use crate::tokenizer::{Token, TokenizedLine, create_reverse_keyword_maps};
use crate::error::BBCBasicError;

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
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
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
    Tab(Expression),      // TAB(n)
    Spc(Expression),      // SPC(n)
    Semicolon,           // ;
    Comma,               // ,
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
    Print {
        items: Vec<PrintItem>,
    },
    /// INPUT statement
    Input {
        variables: Vec<String>,
    },
    /// FOR loop
    For {
        variable: String,
        start: Expression,
        end: Expression,
        step: Option<Expression>,
    },
    /// NEXT statement
    Next {
        variables: Vec<String>,
    },
    /// IF statement
    If {
        condition: Expression,
        then_part: Vec<Statement>,
        else_part: Option<Vec<Statement>>,
    },
    /// GOTO statement
    Goto {
        line_number: u16,
    },
    /// GOSUB statement
    Gosub {
        line_number: u16,
    },
    /// RETURN statement
    Return,
    /// DIM statement for array dimensioning
    Dim {
        arrays: Vec<(String, Vec<Expression>)>,
    },
    /// REM statement (comment)
    Rem {
        comment: String,
    },
    /// END statement
    End,
    /// STOP statement
    Stop,
    /// Procedure call
    ProcCall {
        name: String,
        args: Vec<Expression>,
    },
    /// DATA statement - stores data values
    Data {
        values: Vec<DataValue>,
    },
    /// READ statement - reads data into variables
    Read {
        variables: Vec<String>,
    },
    /// RESTORE statement - resets data pointer (optionally to specific line)
    Restore {
        line_number: Option<u16>,
    },
    /// REPEAT statement - starts a REPEAT...UNTIL loop
    Repeat,
    /// UNTIL statement - ends a REPEAT...UNTIL loop
    Until {
        condition: Expression,
    },
    /// CLS statement - clear screen
    Cls,
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
                | Statement::Return
        )
    }

    /// Check if this statement ends program execution
    pub fn is_terminating(&self) -> bool {
        matches!(self, Statement::End | Statement::Stop)
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
                | BinaryOperator::Eor => ExpressionType::Integer,
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
        Token::Keyword(0xF1) => parse_print_statement(&tokens[1..]),
        
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
        
        // RETURN statement
        Token::Keyword(0xF8) => Ok(Statement::Return),
        
        // INPUT statement
        Token::Keyword(0xE8) => parse_input_statement(&tokens[1..]),
        
        // DIM statement
        Token::Keyword(0xDE) => parse_dim_statement(&tokens[1..], line.line_number),
        
        // IF statement
        Token::Keyword(0xE7) => parse_if_statement(&tokens[1..], line.line_number),
        
        // END statement
        Token::Keyword(0xE0) => Ok(Statement::End),
        
        // STOP statement
        Token::Keyword(0xFA) => Ok(Statement::Stop),
        
        // REM statement (comment)
        Token::Keyword(0xF4) => {
            // Everything after REM is a comment
            let comment = tokens[1..].iter()
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
                
                let expr = parse_expression(&tokens[start_pos..pos-1])?;
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
                
                let expr = parse_expression(&tokens[start_pos..pos-1])?;
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
        _ => return Err(BBCBasicError::SyntaxError {
            message: "Expected variable name".to_string(),
            line: line_number,
        }),
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
        _ => return Err(BBCBasicError::SyntaxError {
            message: "Expected variable name after FOR".to_string(),
            line: line_number,
        }),
    };
    
    if !matches!(tokens[1], Token::Operator('=')) {
        return Err(BBCBasicError::SyntaxError {
            message: "Expected '=' in FOR statement".to_string(),
            line: line_number,
        });
    }
    
    // Find TO keyword
    let to_pos = tokens.iter().position(|t| matches!(t, Token::Keyword(0xB8)))
        .ok_or(BBCBasicError::SyntaxError {
            message: "Expected TO in FOR statement".to_string(),
            line: line_number,
        })?;
    
    let start = parse_expression(&tokens[2..to_pos])?;
    
    // Check for STEP keyword
    let step_pos = tokens.iter().position(|t| matches!(t, Token::Keyword(0x88)));
    
    let (end, step) = if let Some(step_pos) = step_pos {
        let end = parse_expression(&tokens[to_pos + 1..step_pos])?;
        let step = parse_expression(&tokens[step_pos + 1..])?;
        (end, Some(step))
    } else {
        let end = parse_expression(&tokens[to_pos + 1..])?;
        (end, None)
    };
    
    Ok(Statement::For { variable, start, end, step })
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
        _ => return Err(BBCBasicError::SyntaxError {
            message: "Expected line number after GOTO".to_string(),
            line: line_number,
        }),
    };
    
    Ok(Statement::Goto { line_number: line_num })
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
        _ => return Err(BBCBasicError::SyntaxError {
            message: "Expected line number after GOSUB".to_string(),
            line: line_number,
        }),
    };
    
    Ok(Statement::Gosub { line_number: line_num })
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
            _ => return Err(BBCBasicError::SyntaxError {
                message: "Expected array name in DIM".to_string(),
                line: line_number,
            }),
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
                line_number: Some(*num as u16) 
            }),
            Token::LineNumber(num) => Ok(Statement::Restore { 
                line_number: Some(*num) 
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

/// Parse IF statement
/// Supports: IF condition THEN statement [ELSE statement]
fn parse_if_statement(tokens: &[Token], line_number: Option<u16>) -> Result<Statement> {
    // Find THEN keyword to split condition from then-part
    let then_pos = tokens.iter().position(|t| matches!(t, Token::Keyword(0x8C)))
        .ok_or(BBCBasicError::SyntaxError {
            message: "Expected THEN after IF condition".to_string(),
            line: line_number,
        })?;
    
    // Parse condition (everything before THEN)
    let condition_tokens = &tokens[..then_pos];
    let condition = parse_expression(condition_tokens)?;
    
    // Find ELSE keyword (if present)
    let else_pos = tokens[then_pos + 1..].iter().position(|t| matches!(t, Token::Keyword(0x8B)));
    
    let (then_tokens, else_tokens) = if let Some(else_idx) = else_pos {
        // ELSE found: split then_part and else_part
        let absolute_else_pos = then_pos + 1 + else_idx;
        (&tokens[then_pos + 1..absolute_else_pos], Some(&tokens[absolute_else_pos + 1..]))
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
        '^' => Some(60),  // Power (highest)
        '*' | '/' => Some(50),  // Multiplication, Division
        '+' | '-' => Some(40),  // Addition, Subtraction
        '=' | '<' | '>' => Some(30),  // Comparison
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

/// Parse expression with precedence climbing algorithm
fn parse_expr_precedence(tokens: &[Token], pos: &mut usize, min_prec: u8) -> Result<Expression> {
    // Parse the left-hand side (primary expression)
    let mut left = parse_primary(tokens, pos)?;

    // Parse binary operators with precedence
    while *pos < tokens.len() {
        // Check if current token is a binary operator
        let op_char = match &tokens[*pos] {
            Token::Operator(ch) => *ch,
            _ => break,
        };

        let prec = match get_precedence(op_char) {
            Some(p) if p >= min_prec => p,
            _ => break,
        };

        *pos += 1; // consume operator

        // Parse right-hand side with higher precedence
        let right = parse_expr_precedence(tokens, pos, prec + 1)?;

        // Create binary operation
        let op = char_to_binary_op(op_char).ok_or(BBCBasicError::SyntaxError {
            message: format!("Invalid operator: {}", op_char),
            line: None,
        })?;

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
            let keyword = main_reverse.get(byte).cloned().unwrap_or_else(|| "UNKNOWN".to_string());
            
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
        let tokens = vec![
            Token::Integer(2),
            Token::Operator('+'),
            Token::Integer(3),
        ];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(expr, Expression::BinaryOp {
            left: Box::new(Expression::Integer(2)),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Integer(3)),
        });
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
        assert_eq!(expr, Expression::BinaryOp {
            left: Box::new(Expression::Integer(2)),
            op: BinaryOperator::Add,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Integer(3)),
                op: BinaryOperator::Multiply,
                right: Box::new(Expression::Integer(4)),
            }),
        });
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
        assert_eq!(expr, Expression::BinaryOp {
            left: Box::new(Expression::Integer(2)),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Integer(3)),
        });
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
        assert_eq!(expr, Expression::FunctionCall {
            name: "SIN".to_string(),
            args: vec![Expression::Integer(45)],
        });
    }

    #[test]
    fn test_parse_unary_minus() {
        // RED: Parse "-5"
        let tokens = vec![
            Token::Operator('-'),
            Token::Integer(5),
        ];
        let expr = parse_expression(&tokens).unwrap();
        assert_eq!(expr, Expression::UnaryOp {
            op: UnaryOperator::Minus,
            operand: Box::new(Expression::Integer(5)),
        });
    }

    // TDD Tests for statement parsing

    #[test]
    fn test_parse_print_simple() {
        // RED: Parse "PRINT 42"
        use crate::tokenizer::tokenize;
        let line = tokenize("PRINT 42").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Print {
            items: vec![PrintItem::Expression(Expression::Integer(42))],
        });
    }

    #[test]
    fn test_parse_print_string() {
        // RED: Parse "PRINT \"Hello\""
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"PRINT "Hello""#).unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Print {
            items: vec![PrintItem::Expression(Expression::String("Hello".to_string()))],
        });
    }

    #[test]
    fn test_parse_assignment() {
        // RED: Parse "A% = 42"
        use crate::tokenizer::tokenize;
        let line = tokenize("A% = 42").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Assignment {
            target: "A%".to_string(),
            expression: Expression::Integer(42),
        });
    }

    #[test]
    fn test_parse_let_assignment() {
        // RED: Parse "LET B = 3.14"
        use crate::tokenizer::tokenize;
        let line = tokenize("LET B = 3.14").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Assignment {
            target: "B".to_string(),
            expression: Expression::Real(3.14),
        });
    }

    #[test]
    fn test_parse_for_loop() {
        // RED: Parse "FOR I% = 1 TO 10"
        use crate::tokenizer::tokenize;
        let line = tokenize("FOR I% = 1 TO 10").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::For {
            variable: "I%".to_string(),
            start: Expression::Integer(1),
            end: Expression::Integer(10),
            step: None,
        });
    }

    #[test]
    fn test_parse_for_loop_with_step() {
        // RED: Parse "FOR I% = 10 TO 1 STEP -1"
        use crate::tokenizer::tokenize;
        let line = tokenize("FOR I% = 10 TO 1 STEP -1").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::For {
            variable: "I%".to_string(),
            start: Expression::Integer(10),
            end: Expression::Integer(1),
            step: Some(Expression::Integer(-1)),
        });
    }

    #[test]
    fn test_parse_next() {
        // RED: Parse "NEXT I%"
        use crate::tokenizer::tokenize;
        let line = tokenize("NEXT I%").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Next {
            variables: vec!["I%".to_string()],
        });
    }

    #[test]
    fn test_parse_goto() {
        // RED: Parse "GOTO 100"
        use crate::tokenizer::tokenize;
        let line = tokenize("GOTO 100").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Goto {
            line_number: 100,
        });
    }

    #[test]
    fn test_parse_gosub() {
        // RED: Parse "GOSUB 1000"
        use crate::tokenizer::tokenize;
        let line = tokenize("GOSUB 1000").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Gosub {
            line_number: 1000,
        });
    }

    #[test]
    fn test_parse_return() {
        // RED: Parse "RETURN"
        use crate::tokenizer::tokenize;
        let line = tokenize("RETURN").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Return);
    }

    #[test]
    fn test_parse_input() {
        // RED: Parse "INPUT A%, B$"
        use crate::tokenizer::tokenize;
        let line = tokenize("INPUT A%, B$").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Input {
            variables: vec!["A%".to_string(), "B$".to_string()],
        });
    }

    #[test]
    fn test_parse_dim() {
        // RED: Parse "DIM A%(10), B(5, 5)"
        use crate::tokenizer::tokenize;
        let line = tokenize("DIM A%(10), B(5, 5)").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::Dim {
            arrays: vec![
                ("A%(".to_string(), vec![Expression::Integer(10)]),
                ("B(".to_string(), vec![Expression::Integer(5), Expression::Integer(5)]),
            ],
        });
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
        
        assert_eq!(stmt, Statement::Assignment {
            target: "X%".to_string(),
            expression: Expression::FunctionCall {
                name: "ABS".to_string(),
                args: vec![Expression::Integer(-5)],
            },
        });
    }

    #[test]
    fn test_parse_if_then_simple() {
        // RED: Parse "IF X% > 10 THEN PRINT \"Big\""
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"IF X% > 10 THEN PRINT "Big""#).unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Integer(10)),
            },
            then_part: vec![Statement::Print {
                items: vec![PrintItem::Expression(Expression::String("Big".to_string()))],
            }],
            else_part: None,
        });
    }

    #[test]
    fn test_parse_if_then_goto() {
        // RED: Parse "IF A% = 0 THEN GOTO 100"
        use crate::tokenizer::tokenize;
        let line = tokenize("IF A% = 0 THEN GOTO 100").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("A%".to_string())),
                op: BinaryOperator::Equal,
                right: Box::new(Expression::Integer(0)),
            },
            then_part: vec![Statement::Goto { line_number: 100 }],
            else_part: None,
        });
    }

    #[test]
    fn test_parse_if_then_else() {
        // RED: Parse "IF X% > 10 THEN PRINT \"Big\" ELSE PRINT \"Small\""
        use crate::tokenizer::tokenize;
        let line = tokenize(r#"IF X% > 10 THEN PRINT "Big" ELSE PRINT "Small""#).unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Integer(10)),
            },
            then_part: vec![Statement::Print {
                items: vec![PrintItem::Expression(Expression::String("Big".to_string()))],
            }],
            else_part: Some(vec![Statement::Print {
                items: vec![PrintItem::Expression(Expression::String("Small".to_string()))],
            }]),
        });
    }

    #[test]
    fn test_parse_if_then_assignment() {
        // RED: Parse "IF X% < 5 THEN Y% = 0"
        use crate::tokenizer::tokenize;
        let line = tokenize("IF X% < 5 THEN Y% = 0").unwrap();
        let stmt = parse_statement(&line).unwrap();
        
        assert_eq!(stmt, Statement::If {
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
        });
    }
}