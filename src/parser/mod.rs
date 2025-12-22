//! Parser for BBC BASIC statements and expressions
//! 
//! Analyzes tokenized BBC BASIC statements and creates abstract syntax trees
//! for execution.

use crate::error::Result;
use crate::tokenizer::{Token, TokenizedLine};

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
    // For now, return a basic implementation
    // Full implementation will be done in task 6
    Ok(Expression::Integer(0))
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
}