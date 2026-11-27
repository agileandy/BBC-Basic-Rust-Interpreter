//! The parse of structures (issue #191).
//!
//! The form is the form of BBC BASIC for Windows, manual page "General
//! information", section "Structures"
//! (https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin2.html#structures):
//! `DIM point{x, y%, name$}` declares a structure, and `point.x` is a member.
//!
//! A member access is `name.member` where the text before the full stop is
//! not a keyword short form (issue #167). So `P.x` is `PRINT x`, and a DIM
//! of a structure with a short form name, such as `P`, gives a syntax error.
//!
//! In this issue a member is a real, an integer or a string. The forms of the
//! later child issues give a syntax error: an array member, a nested
//! structure, an array of structures, a structure from a prototype, and a
//! reference to a whole structure, such as `b{} = a{}` or `PROCa(s{})`.

use super::{
    missing_value_at_a_comma, parse_expression, tokens_text, DimItem, Expression, InputItem,
    Statement,
};
use crate::error::{BBCBasicError, Result};
use crate::tokenizer::{is_short_form, Token};

/// A syntax error with this message
fn syntax_error(message: &str, line_number: Option<u16>) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: message.to_string(),
        line: line_number,
    }
}

/// The error of an array member, such as `p{a(3)}` or `p.a(1)`
pub(super) fn array_member_error(line_number: Option<u16>) -> BBCBasicError {
    syntax_error("Array members are not supported", line_number)
}

/// A `{` in a statement that is not a DIM is a reference to a whole
/// structure, a form of a later issue. Give its syntax error.
pub(super) fn check_no_whole_structure(tokens: &[Token], line_number: Option<u16>) -> Result<()> {
    let has_brace = tokens.contains(&Token::Separator('{'));
    let has_dim = tokens.contains(&Token::Keyword(0xDE));
    if has_brace && !has_dim {
        return Err(syntax_error(
            "A reference to a whole structure is not supported",
            line_number,
        ));
    }
    Ok(())
}

/// Parse a structure of a DIM. The tokens start at the `{` after the
/// name. Give the item and the number of tokens that it uses, to the `}`.
pub(super) fn parse_dim_structure(
    name: String,
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<(DimItem, usize)> {
    if is_short_form(&name) {
        let message =
            format!("{name}. is a keyword short form, so {name} cannot be a structure name");
        return Err(syntax_error(&message, line_number));
    }
    if name.ends_with(['%', '$']) {
        return Err(syntax_error(
            "A structure name cannot end in % or $",
            line_number,
        ));
    }
    match tokens.get(1..3) {
        Some([Token::Separator('}'), Token::Operator('=')]) => {
            return Err(syntax_error(
                "A structure from a prototype is not supported",
                line_number,
            ))
        }
        Some([Token::Separator('('), _]) => {
            return Err(syntax_error(
                "Arrays of structures are not supported",
                line_number,
            ))
        }
        _ => {}
    }

    let mut members: Vec<String> = Vec::new();
    let mut pos = 1;
    loop {
        let member = match tokens.get(pos) {
            Some(Token::Identifier(member)) => member,
            _ => return Err(syntax_error("Expected member name in DIM", line_number)),
        };
        match tokens.get(pos + 1) {
            Some(Token::Separator('(')) => return Err(array_member_error(line_number)),
            Some(Token::Separator('{')) => {
                return Err(syntax_error(
                    "Nested structures are not supported",
                    line_number,
                ))
            }
            _ => {}
        }
        if members.contains(member) {
            let message = format!("Duplicate member name: {member}");
            return Err(syntax_error(&message, line_number));
        }
        members.push(member.clone());
        pos += 1;
        match tokens.get(pos) {
            Some(Token::Separator(',')) => pos += 1,
            Some(Token::Separator('}')) => break,
            _ => return Err(syntax_error("Expected '}' in DIM", line_number)),
        }
    }
    Ok((DimItem::Structure { name, members }, pos + 1))
}

/// After a structure of a DIM at `pos`, give the position of the next DIM
/// item, or `None` at the end of the statement. Only a comma or the end can
/// follow the structure, as after an array (issue #177).
pub(super) fn next_dim_item(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
) -> Result<Option<usize>> {
    match tokens.get(pos) {
        None => Ok(None),
        Some(Token::Separator(',')) => match tokens.get(pos + 1) {
            None | Some(Token::Separator(',')) => Err(missing_value_at_a_comma(line_number)),
            Some(_) => Ok(Some(pos + 1)),
        },
        Some(token) => {
            let message = format!(
                "Unexpected {} after DIM item",
                tokens_text(std::slice::from_ref(token))
            );
            Err(syntax_error(&message, line_number))
        }
    }
}

/// Parse `point.x = expr` (issue #191). The tokens start at the member.
pub(super) fn parse_member_assignment(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Statement> {
    let [Token::Member(structure, member), rest @ ..] = tokens else {
        return Err(syntax_error("Expected variable name", line_number));
    };
    match rest.first() {
        Some(Token::Separator('(')) => Err(array_member_error(line_number)),
        Some(Token::Operator('=')) => Ok(Statement::MemberAssignment {
            structure: structure.clone(),
            member: member.clone(),
            expression: parse_expression(&rest[1..])?,
        }),
        _ => Err(syntax_error("Expected '='", line_number)),
    }
}

/// The expression of a member token. A bracket after it is an array member.
pub(super) fn member_expression(
    structure: &str,
    member: &str,
    next: Option<&Token>,
) -> Result<Expression> {
    if matches!(next, Some(Token::Separator('('))) {
        return Err(array_member_error(None));
    }
    Ok(Expression::Member {
        structure: structure.to_string(),
        member: member.to_string(),
    })
}

/// The target of INPUT, READ or SWAP for a member token, if the tokens are
/// one member token
pub(super) fn member_target(tokens: &[Token]) -> Option<InputItem> {
    match tokens {
        [Token::Member(structure, member)] => Some(InputItem::Member {
            structure: structure.clone(),
            member: member.clone(),
        }),
        _ => None,
    }
}

/// The INPUT item of the member token at the start of the tokens
pub(super) fn input_member(tokens: &[Token], line_number: Option<u16>) -> Result<InputItem> {
    if matches!(tokens.get(1), Some(Token::Separator('('))) {
        return Err(array_member_error(line_number));
    }
    member_target(&tokens[..1]).ok_or_else(|| syntax_error("Expected variable name", line_number))
}

/// The FOR loop variable cannot be a member in this issue. The manual
/// permits it ("Any numeric assignable item"): a later issue can add it.
pub(super) fn check_for_variable(tokens: &[Token], line_number: Option<u16>) -> Result<()> {
    if matches!(tokens.first(), Some(Token::Member(..))) {
        return Err(syntax_error(
            "A FOR variable that is a structure member is not supported",
            line_number,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{parse_line, BinaryOperator};
    use super::*;
    use crate::tokenizer::tokenize;

    fn parse(text: &str) -> Result<Vec<Statement>> {
        parse_line(&tokenize(text).unwrap())
    }

    fn member(structure: &str, member: &str) -> Expression {
        Expression::Member {
            structure: structure.to_string(),
            member: member.to_string(),
        }
    }

    #[test]
    fn test_dim_of_a_structure_and_an_array() {
        let statements = parse("DIM p{x, y%, n$}, a(2)").unwrap();
        let Statement::Dim { items } = &statements[0] else {
            panic!("{statements:?}");
        };
        assert_eq!(
            items[0],
            DimItem::Structure {
                name: "p".to_string(),
                members: vec!["x".to_string(), "y%".to_string(), "n$".to_string()],
            }
        );
        assert!(matches!(&items[1], DimItem::Array { name, .. } if name == "a"));
    }

    #[test]
    fn test_tokens_after_a_structure_are_an_error() {
        for text in ["DIM p{x} 5", "DIM p{x},", "DIM p{x", "DIM p{x y}"] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn test_member_assignment_and_expression() {
        assert_eq!(
            parse("p.x = q.y% + 1").unwrap(),
            vec![Statement::MemberAssignment {
                structure: "p".to_string(),
                member: "x".to_string(),
                expression: Expression::BinaryOp {
                    left: Box::new(member("q", "y%")),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(1)),
                },
            }]
        );
    }

    #[test]
    fn test_member_targets_of_input_read_and_swap() {
        let target = InputItem::Member {
            structure: "p".to_string(),
            member: "s$".to_string(),
        };
        assert_eq!(
            parse("INPUT p.s$").unwrap(),
            vec![Statement::Input {
                items: vec![target.clone()]
            }]
        );
        assert_eq!(
            parse("READ p.s$").unwrap(),
            vec![Statement::Read {
                variables: vec![target.clone()]
            }]
        );
        assert_eq!(
            parse("SWAP p.s$, p.s$").unwrap(),
            vec![Statement::Swap {
                first: target.clone(),
                second: target,
            }]
        );
    }

    #[test]
    fn test_whole_structure_outside_dim_is_an_error() {
        for text in [
            "b{} = a{}",
            "PROCa(a{})",
            "LOCAL a{}",
            "IF 1 THEN b{} = a{}",
        ] {
            assert!(
                matches!(
                    parse(text),
                    Err(BBCBasicError::SyntaxError { ref message, .. })
                        if message == "A reference to a whole structure is not supported"
                ),
                "{text}"
            );
        }
        assert!(parse("IF 1 THEN DIM p{x}").is_ok());
    }
}
