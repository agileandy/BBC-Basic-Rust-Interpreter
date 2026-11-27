//! Parse a matrix statement (extension, issue #190):
//! `MATTRANSPOSE dest(), src()`, `MATINVERSE dest(), src()`,
//! `MATRELU dest(), src()`, `MATSIGMOID dest(), src()`,
//! `MATTANH dest(), src()` and `MATSOFTMAX dest(), src()`.
//!
//! The caller tries an assignment first, so that a variable or an array with
//! one of these names keeps its old meaning.

use super::{parse_array_reference, Statement};
use crate::error::{BBCBasicError, Result};
use crate::matrix::MatrixStatement;
use crate::tokenizer::Token;

/// Tell if a name is a matrix statement
pub(super) fn is_matrix_statement(name: &str) -> bool {
    MatrixStatement::from_name(name).is_some()
}

/// Parse the statement. The first token is the name of a matrix statement.
pub(super) fn parse_matrix_statement(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Statement> {
    let Some(Token::Identifier(name)) = tokens.first() else {
        unreachable!("the caller checks the statement name");
    };
    let statement = MatrixStatement::from_name(name).expect("the caller checks the name");
    let target = parse_array_reference(tokens, 1);
    let source = parse_array_reference(tokens, 5);
    match (target, tokens.get(4), source, tokens.len()) {
        (Some(target), Some(Token::Separator(',')), Some(source), 8) => Ok(Statement::Matrix {
            statement,
            target: target.to_string(),
            source: source.to_string(),
        }),
        _ => Err(BBCBasicError::SyntaxError {
            message: format!("{name} requires a target array and a source array"),
            line: line_number,
        }),
    }
}
