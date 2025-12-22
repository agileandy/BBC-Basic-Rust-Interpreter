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
    /// Empty statement
    Empty,
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
pub fn parse_statement(tokens: &TokenizedLine) -> Result<Statement> {
    // For now, return a basic implementation
    // Full implementation will be done in task 8
    Ok(Statement::Empty)
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
}