//! Parse a whole-array statement (issue #187): `A() = B()`, `A() = 5`,
//! `A() = 1, 2, 3`, `A() = B() * 2 + C()` and `A() += 1`.
//!
//! The reference is the manual of BBC BASIC for Windows, page bbcwin2.html,
//! "Array arithmetic" and "Initialising arrays". An array expression is
//! strictly from left to right. A scalar operand binds only the operators
//! with a higher priority than the operator before it, as the function
//! `expra` of BBC BASIC for SDL (src/bbeval.c) does. So `B() + 2 * 3` adds 6.

use super::{parse_expr_precedence, tokens_text, BinaryOperator, Expression, Statement};
use crate::error::{BBCBasicError, Result};
use crate::tokenizer::Token;

/// An operand of an array expression
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayOperand {
    /// A whole array, such as `B()`. The name has no bracket.
    Array(String),
    /// A scalar expression, evaluated one time
    Scalar(Expression),
    /// The dot product `B() . C()` of 2 arrays (issue #189). It is the only
    /// operand of the statement.
    Product(String, String),
}

/// The right side of a whole-array statement
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayExpression {
    /// The first operand
    pub first: ArrayOperand,
    /// The next operators and operands, from left to right
    pub rest: Vec<(BinaryOperator, ArrayOperand)>,
    /// The values after the first comma, for the elements 1, 2 and more
    pub list: Vec<Expression>,
}

/// If the tokens at `pos` are a whole-array reference `name()`, give the
/// name. Issues #188, #189 and #190 use this for the array functions.
pub fn parse_array_reference(tokens: &[Token], pos: usize) -> Option<&str> {
    match tokens.get(pos..pos + 3) {
        Some([Token::Identifier(name), Token::Separator('('), Token::Separator(')')]) => Some(name),
        _ => None,
    }
}

/// The keyword codes of the operators AND, DIV, EOR, MOD and OR
const AND: u8 = 0x80;
const DIV: u8 = 0x81;
const EOR: u8 = 0x82;
const MOD: u8 = 0x83;
const OR: u8 = 0x84;

/// The array operator of a token. The manual gives + - * / ^ AND OR EOR DIV
/// and MOD. The dot product is in `parse_product`.
fn array_operator(token: &Token) -> Option<BinaryOperator> {
    match token {
        Token::Operator('+') => Some(BinaryOperator::Add),
        Token::Operator('-') => Some(BinaryOperator::Subtract),
        Token::Operator('*') => Some(BinaryOperator::Multiply),
        Token::Operator('/') => Some(BinaryOperator::Divide),
        Token::Operator('^') => Some(BinaryOperator::Power),
        Token::Keyword(AND) => Some(BinaryOperator::And),
        Token::Keyword(DIV) => Some(BinaryOperator::IntegerDivide),
        Token::Keyword(EOR) => Some(BinaryOperator::Eor),
        Token::Keyword(MOD) => Some(BinaryOperator::Modulo),
        Token::Keyword(OR) => Some(BinaryOperator::Or),
        _ => None,
    }
}

/// The lowest precedence that a scalar operand after `op` can contain. `None`
/// is the first operand. These are the levels of `expra` in BBC BASIC for SDL.
fn scalar_precedence(op: Option<&BinaryOperator>) -> u8 {
    match op {
        Some(BinaryOperator::Or | BinaryOperator::Eor) => 20,
        Some(BinaryOperator::And) => 40,
        Some(BinaryOperator::Add | BinaryOperator::Subtract) => 50,
        _ => 60,
    }
}

fn syntax_error(message: String, line_number: Option<u16>) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message,
        line: line_number,
    }
}

/// Parse one operand at `pos`
fn parse_operand(
    tokens: &[Token],
    pos: &mut usize,
    op: Option<&BinaryOperator>,
) -> Result<ArrayOperand> {
    if let Some(name) = parse_array_reference(tokens, *pos) {
        *pos += 3;
        return Ok(ArrayOperand::Array(name.to_string()));
    }
    let scalar = parse_expr_precedence(tokens, pos, scalar_precedence(op))?;
    Ok(ArrayOperand::Scalar(scalar))
}

/// Parse a statement that starts with `name()`. The caller checks the first
/// 3 tokens with `parse_array_reference`.
pub(super) fn parse_whole_array_statement(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Statement> {
    let target = parse_array_reference(tokens, 0)
        .expect("the caller checks the array reference")
        .to_string();

    // `=`, or a compound assignment such as `+=`
    let (operator, mut pos) = match (tokens.get(3), tokens.get(4)) {
        (Some(Token::Operator('=')), _) => (None, 4),
        (Some(token), Some(Token::Operator('='))) if array_operator(token).is_some() => {
            (array_operator(token), 5)
        }
        _ => {
            return Err(syntax_error(
                "Expected '=' after array".to_string(),
                line_number,
            ))
        }
    };
    if pos >= tokens.len() {
        return Err(syntax_error("Expected expression".to_string(), line_number));
    }

    // A unary minus before the first array is 0 minus the array, as in BBC
    // BASIC for SDL. A minus before a number is part of the number.
    if let Some(product) = parse_product(tokens, &mut pos, line_number)? {
        return finish(tokens, pos, line_number, target, operator, product);
    }
    let mut rest = Vec::new();
    let first = match (tokens.get(pos), parse_array_reference(tokens, pos + 1)) {
        (Some(Token::Operator('-')), Some(name)) => {
            rest.push((
                BinaryOperator::Subtract,
                ArrayOperand::Array(name.to_string()),
            ));
            pos += 4;
            ArrayOperand::Scalar(Expression::Integer(0))
        }
        _ => parse_operand(tokens, &mut pos, None)?,
    };

    while let Some(op) = tokens.get(pos).and_then(array_operator) {
        pos += 1;
        let operand = parse_operand(tokens, &mut pos, Some(&op))?;
        rest.push((op, operand));
    }

    let mut list = Vec::new();
    while let Some(Token::Separator(',')) = tokens.get(pos) {
        pos += 1;
        list.push(parse_expr_precedence(tokens, &mut pos, 0)?);
    }

    finish(
        tokens,
        pos,
        line_number,
        target,
        operator,
        ArrayExpression { first, rest, list },
    )
}

/// The dot operator of a matrix product (issue #189)
const DOT: Token = Token::Operator('.');

/// If the tokens at `pos` are `B() . C()`, give the product as the only
/// operand, and move `pos` after it. The reference is the manual of BBC
/// BASIC for Windows, page bbcwin2.html, "Dot product". In `expra` of BBC
/// BASIC for SDL, the dot ends the array expression, so an operator after
/// the product is an error. This interpreter also gives an error for an
/// operator or a minus before the product, which `expra` ignores.
fn parse_product(
    tokens: &[Token],
    pos: &mut usize,
    line_number: Option<u16>,
) -> Result<Option<ArrayExpression>> {
    let Some(left) = parse_array_reference(tokens, *pos) else {
        return Ok(None);
    };
    if tokens.get(*pos + 3) != Some(&DOT) {
        return Ok(None);
    }
    let Some(right) = parse_array_reference(tokens, *pos + 4) else {
        return Err(syntax_error(
            "Expected array after '.'".to_string(),
            line_number,
        ));
    };
    *pos += 7;
    Ok(Some(ArrayExpression {
        first: ArrayOperand::Product(left.to_string(), right.to_string()),
        rest: Vec::new(),
        list: Vec::new(),
    }))
}

/// Give the statement, or a syntax error for a token after the expression
fn finish(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
    target: String,
    operator: Option<BinaryOperator>,
    value: ArrayExpression,
) -> Result<Statement> {
    match tokens.get(pos) {
        Some(&DOT) => Err(syntax_error(
            "The dot operator must be the only operator".to_string(),
            line_number,
        )),
        Some(token) => Err(syntax_error(
            format!(
                "Unexpected {} after array expression",
                tokens_text(std::slice::from_ref(token))
            ),
            line_number,
        )),
        None => Ok(Statement::ArrayAssign {
            target,
            operator,
            value,
        }),
    }
}

/// The keyword code of DIM, and the extended keyword of SUM
const DIM: u8 = 0xDE;
const SUM: Token = Token::ExtendedKeyword(0xC6, 0x8E);

/// Parse an array function at `pos` (issue #188): `SUM(a())`, `SUMLEN(a$())`,
/// `MOD(a())`, `DIM(a())` or `DIM(a(), n)`. The reference is the manual of
/// BBC BASIC for Windows, page bbcwin2.html, "Array functions".
///
/// The new form applies only with `(` and an array reference after the
/// keyword. For other tokens, give `None` and do not move `pos`: `MOD` and
/// `DIM` keep their old meaning, and `SUMLEN` is a variable or an array.
/// `SUM` with another argument is a call, which gives Type mismatch when it
/// runs, as in BBC BASIC.
///
/// The first argument of the call is the array as `ArrayAccess` with no
/// indices, the same form as the parse of `a()` in other places.
pub(super) fn parse_array_function(
    tokens: &[Token],
    pos: &mut usize,
) -> Result<Option<Expression>> {
    let name = match tokens.get(*pos) {
        Some(token) if *token == SUM => "SUM",
        Some(Token::Identifier(word)) if word == "SUMLEN" => "SUMLEN",
        Some(Token::Keyword(MOD)) => "MOD",
        Some(Token::Keyword(DIM)) => "DIM",
        _ => return Ok(None),
    };
    if tokens.get(*pos + 1) != Some(&Token::Separator('(')) {
        return Ok(None);
    }
    let mut next = *pos + 2;
    let first = match parse_array_reference(tokens, next) {
        Some(array) => {
            next += 3;
            Expression::ArrayAccess {
                name: array.to_string(),
                indices: Vec::new(),
            }
        }
        None if name == "SUM" => parse_expr_precedence(tokens, &mut next, 0)?,
        None => return Ok(None),
    };
    let mut args = vec![first];
    if name == "DIM" && tokens.get(next) == Some(&Token::Separator(',')) {
        next += 1;
        args.push(parse_expr_precedence(tokens, &mut next, 0)?);
    }
    if tokens.get(next) != Some(&Token::Separator(')')) {
        return Err(syntax_error(format!("Expected ')' after {name}"), None));
    }
    *pos = next + 1;
    Ok(Some(Expression::FunctionCall {
        name: name.to_string(),
        args,
    }))
}
