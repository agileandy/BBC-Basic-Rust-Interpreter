//! The parse of structures (issues #191 and #192).
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
//! A member is a real, an integer, a string, an array or a sub-structure
//! (issue #192). The same manual section, "Declaring a structure", gives
//! `DIM mystruct{four(3), five%(1,2), seven{a,b%}}`, with the members
//! `mystruct.four()` and `mystruct.seven.a`. Its section "Structure members"
//! says that operations on entire arrays, such as SUM, cannot use structure
//! members, so `s.a()` is a syntax error.
//!
//! Issue #194 adds a whole structure as a parameter and as an argument of a
//! PROC or FN, `DEF PROCmove(p{}, dx)` and `PROCmove(point{}, 5)`, and the
//! LOCAL structure, `LOCAL s{}`. The manual section is "Passing structures
//! to procedures and functions" on the same page.
//!
//! The forms of the later child issues give a syntax error: an array of
//! structures, a structure from a prototype, and a reference to a whole
//! structure, such as `b{} = a{}`.

use super::{
    find_closing_bracket, missing_value_at_a_comma, parse_comma_separated_expressions,
    parse_expression, tokens_text, DimItem, Expression, InputItem, Statement,
};
use crate::error::{BBCBasicError, Result};
use crate::tokenizer::{is_short_form, Token};

/// A member path, such as `shape.pos.x` or `net.layer.weights(I%, J%)`
/// (issue #192). Only the last member can have indices: it is an array
/// member. A member before it is a sub-structure, and arrays of structures
/// are a later issue.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberPath {
    /// The name of the structure
    pub structure: String,
    /// The member names, one for each full stop
    pub members: Vec<String>,
    /// The indices of an array member. An empty list is a member that is
    /// not an array element.
    pub indices: Vec<Expression>,
}

impl MemberPath {
    /// The name of the last member. Its suffix gives the type.
    pub fn member(&self) -> &str {
        self.members.last().map_or("", String::as_str)
    }
}

/// A member in the DIM of a structure (issue #192)
#[derive(Debug, Clone, PartialEq)]
pub enum MemberDecl {
    /// A real, an integer or a string, such as `x`, `n%` or `s$`
    Scalar(String),
    /// An array, such as `weights(20, 20)`. The dimensions are the top
    /// indices, as in `DIM a(20, 20)`.
    Array {
        name: String,
        dimensions: Vec<Expression>,
    },
    /// A sub-structure, such as `pos{x, y}`
    Structure {
        name: String,
        members: Vec<MemberDecl>,
    },
}

impl MemberDecl {
    /// The name of the member
    pub fn name(&self) -> &str {
        match self {
            MemberDecl::Scalar(name)
            | MemberDecl::Array { name, .. }
            | MemberDecl::Structure { name, .. } => name,
        }
    }
}

/// A syntax error with this message
fn syntax_error(message: &str, line_number: Option<u16>) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: message.to_string(),
        line: line_number,
    }
}

/// A `{` in a statement that is not a DIM is a reference to a whole
/// structure. A statement with PROC, FN, DEF or LOCAL can have `name{}`
/// (issue #194): the parse of the statement checks where it is. Each other
/// reference is a form of a later issue: give its syntax error.
pub(super) fn check_no_whole_structure(tokens: &[Token], line_number: Option<u16>) -> Result<()> {
    let has_brace = tokens.contains(&Token::Separator('{'));
    let has_dim = tokens.contains(&Token::Keyword(0xDE));
    if has_brace && !has_dim && !(has_routine_keyword(tokens) && only_empty_braces(tokens)) {
        return Err(syntax_error(
            "A reference to a whole structure is not supported",
            line_number,
        ));
    }
    Ok(())
}

/// True if the tokens have PROC, FN, DEF or LOCAL
fn has_routine_keyword(tokens: &[Token]) -> bool {
    tokens
        .iter()
        .any(|token| matches!(token, Token::Keyword(0xF2 | 0xA4 | 0xDD | 0xEA)))
}

/// True if each `{` of the tokens is a `name{}`
fn only_empty_braces(tokens: &[Token]) -> bool {
    tokens.iter().enumerate().all(|(index, token)| {
        *token != Token::Separator('{')
            || (index > 0
                && matches!(tokens[index - 1], Token::Identifier(_))
                && tokens.get(index + 1) == Some(&Token::Separator('}')))
    })
}

/// Check that a name can be a structure name. A keyword short form cannot,
/// because `P.x` is `PRINT x`. A name with `%` or `$` cannot.
fn check_structure_name(name: &str, line_number: Option<u16>) -> Result<()> {
    if is_short_form(name) {
        let message =
            format!("{name}. is a keyword short form, so {name} cannot be a structure name");
        return Err(syntax_error(&message, line_number));
    }
    check_no_type_suffix(name, line_number)
}

/// A structure name or a sub-structure name cannot have the suffix of a
/// type. A sub-structure name can be a short form, because `s.P.x` is not
/// `PRINT` (issue #192).
fn check_no_type_suffix(name: &str, line_number: Option<u16>) -> Result<()> {
    if name.ends_with(['%', '$']) {
        return Err(syntax_error(
            "A structure name cannot end in % or $",
            line_number,
        ));
    }
    Ok(())
}

/// If the tokens after a name start with `{}`, give the structure name of a
/// parameter or a LOCAL, `name{}` (issue #194). The 2 brace tokens are the
/// caller's to skip.
pub(super) fn structure_name(
    name: &str,
    after: &[Token],
    line_number: Option<u16>,
) -> Result<Option<String>> {
    match after {
        [Token::Separator('{'), Token::Separator('}'), ..] => {
            check_structure_name(name, line_number)?;
            Ok(Some(format!("{name}{{}}")))
        }
        _ => Ok(None),
    }
}

/// If the tokens at `pos` are `{}` after a name in an expression, consume
/// them and give the whole structure, an argument of a PROC or FN (issue #194)
pub(super) fn whole_structure(name: &str, tokens: &[Token], pos: &mut usize) -> Option<Expression> {
    match tokens.get(*pos..*pos + 2) {
        Some([Token::Separator('{'), Token::Separator('}')]) => {
            *pos += 2;
            Some(Expression::Structure(name.to_string()))
        }
        _ => None,
    }
}

/// Parse a structure of a DIM. The tokens start at the `{` after the
/// name. Give the item and the number of tokens that it uses, to the `}`.
pub(super) fn parse_dim_structure(
    name: String,
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<(DimItem, usize)> {
    check_structure_name(&name, line_number)?;
    let (members, close) = parse_members(tokens, line_number)?;
    Ok((DimItem::Structure { name, members }, close + 1))
}

/// Parse the members of a structure or a sub-structure. The tokens start at
/// the `{`. Give the members and the position of the `}`.
fn parse_members(tokens: &[Token], line_number: Option<u16>) -> Result<(Vec<MemberDecl>, usize)> {
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

    let mut members: Vec<MemberDecl> = Vec::new();
    let mut pos = 1;
    loop {
        let (member, last) = parse_member_decl(tokens, pos, line_number)?;
        if members.iter().any(|other| other.name() == member.name()) {
            let message = format!("Duplicate member name: {}", member.name());
            return Err(syntax_error(&message, line_number));
        }
        members.push(member);
        pos = last + 1;
        match tokens.get(pos) {
            Some(Token::Separator(',')) => pos += 1,
            Some(Token::Separator('}')) => return Ok((members, pos)),
            _ => return Err(syntax_error("Expected '}' in DIM", line_number)),
        }
    }
}

/// Parse the member of a DIM at `pos`: a name, a name with dimensions in
/// brackets, or a name with members in braces. Give the member and the
/// position of its last token.
fn parse_member_decl(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
) -> Result<(MemberDecl, usize)> {
    let name = match tokens.get(pos) {
        Some(Token::Identifier(name)) => name.clone(),
        _ => return Err(syntax_error("Expected member name in DIM", line_number)),
    };
    match tokens.get(pos + 1) {
        Some(Token::Separator('(')) => {
            let close = find_closing_bracket(tokens, pos + 1)
                .ok_or_else(|| syntax_error("Expected ')' in DIM", line_number))?;
            let dimensions =
                parse_comma_separated_expressions(&tokens[pos + 2..close], line_number)?;
            Ok((MemberDecl::Array { name, dimensions }, close))
        }
        Some(Token::Separator('{')) => {
            check_no_type_suffix(&name, line_number)?;
            let (members, close) = parse_members(&tokens[pos + 1..], line_number)?;
            Ok((MemberDecl::Structure { name, members }, pos + 1 + close))
        }
        _ => Ok((MemberDecl::Scalar(name), pos)),
    }
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

/// Parse the member path at `pos`: the member token and the indices in
/// brackets after it, if any. Give the path and the position of its last
/// token. Empty brackets, such as `s.a()`, are a syntax error: the manual
/// does not permit a whole array member.
pub(super) fn parse_member_path(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
) -> Result<(MemberPath, usize)> {
    let Some(Token::Member(structure, members)) = tokens.get(pos) else {
        return Err(syntax_error("Expected variable name", line_number));
    };
    let mut path = MemberPath {
        structure: structure.clone(),
        members: members.clone(),
        indices: Vec::new(),
    };
    if !matches!(tokens.get(pos + 1), Some(Token::Separator('('))) {
        return Ok((path, pos));
    }
    let close = find_closing_bracket(tokens, pos + 1)
        .ok_or_else(|| syntax_error("Expected ')' after the member indices", line_number))?;
    if close == pos + 2 {
        return Err(syntax_error(
            "A whole array member is not supported: use an index",
            line_number,
        ));
    }
    path.indices = parse_comma_separated_expressions(&tokens[pos + 2..close], line_number)?;
    Ok((path, close))
}

/// Parse `point.x = expr` or `s.a(1) = expr` (issues #191 and #192). The
/// tokens start at the member.
pub(super) fn parse_member_assignment(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Statement> {
    let (target, last) = parse_member_path(tokens, 0, line_number)?;
    match tokens.get(last + 1) {
        Some(Token::Operator('=')) => Ok(Statement::MemberAssignment {
            target,
            expression: parse_expression(&tokens[last + 2..])?,
        }),
        _ => Err(syntax_error("Expected '='", line_number)),
    }
}

/// The expression of the member path at `pos`. Move `pos` after it.
pub(super) fn member_expression(tokens: &[Token], pos: &mut usize) -> Result<Expression> {
    let (path, last) = parse_member_path(tokens, *pos, None)?;
    *pos = last + 1;
    Ok(Expression::Member(Box::new(path)))
}

/// The target of READ, SWAP or INPUT# if the tokens are one member path.
/// `None` is tokens that are not a member path.
pub(super) fn member_target(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Option<InputItem>> {
    if !matches!(tokens.first(), Some(Token::Member(..))) {
        return Ok(None);
    }
    let (path, last) = parse_member_path(tokens, 0, line_number)?;
    Ok((last + 1 == tokens.len()).then_some(InputItem::Member(path)))
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

    fn path(structure: &str, members: &[&str], indices: Vec<Expression>) -> MemberPath {
        MemberPath {
            structure: structure.to_string(),
            members: members.iter().map(|member| member.to_string()).collect(),
            indices,
        }
    }

    fn member(structure: &str, members: &[&str], indices: Vec<Expression>) -> Expression {
        Expression::Member(Box::new(path(structure, members, indices)))
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
                members: ["x", "y%", "n$"]
                    .map(|name| MemberDecl::Scalar(name.to_string()))
                    .to_vec(),
            }
        );
        assert!(matches!(&items[1], DimItem::Array { name, .. } if name == "a"));
    }

    #[test]
    fn test_dim_of_array_members_and_a_sub_structure() {
        let statements = parse("DIM s{a(2, N%), p{x, t$(1)}}, b(1)").unwrap();
        let Statement::Dim { items } = &statements[0] else {
            panic!("{statements:?}");
        };
        assert_eq!(
            items[0],
            DimItem::Structure {
                name: "s".to_string(),
                members: vec![
                    MemberDecl::Array {
                        name: "a".to_string(),
                        dimensions: vec![
                            Expression::Integer(2),
                            Expression::Variable("N%".to_string()),
                        ],
                    },
                    MemberDecl::Structure {
                        name: "p".to_string(),
                        members: vec![
                            MemberDecl::Scalar("x".to_string()),
                            MemberDecl::Array {
                                name: "t$".to_string(),
                                dimensions: vec![Expression::Integer(1)],
                            },
                        ],
                    },
                ],
            }
        );
        assert!(matches!(&items[1], DimItem::Array { name, .. } if name == "b"));
    }

    #[test]
    fn test_tokens_after_a_structure_are_an_error() {
        for text in [
            "DIM p{x} 5",
            "DIM p{x},",
            "DIM p{x",
            "DIM p{x y}",
            "DIM p{q{x} y}",
            "DIM p{a(1) y}",
            "DIM p{q%{x}}",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn test_member_assignment_and_expression() {
        assert_eq!(
            parse("p.x = q.y% + 1").unwrap(),
            vec![Statement::MemberAssignment {
                target: path("p", &["x"], Vec::new()),
                expression: Expression::BinaryOp {
                    left: Box::new(member("q", &["y%"], Vec::new())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(1)),
                },
            }]
        );
    }

    #[test]
    fn test_member_paths_with_indices() {
        assert_eq!(
            parse("n.l.w(1, I%) = s.p.x * s.a(2)").unwrap(),
            vec![Statement::MemberAssignment {
                target: path(
                    "n",
                    &["l", "w"],
                    vec![
                        Expression::Integer(1),
                        Expression::Variable("I%".to_string())
                    ]
                ),
                expression: Expression::BinaryOp {
                    left: Box::new(member("s", &["p", "x"], Vec::new())),
                    op: BinaryOperator::Multiply,
                    right: Box::new(member("s", &["a"], vec![Expression::Integer(2)])),
                },
            }]
        );
    }

    #[test]
    fn test_a_whole_array_member_is_an_error() {
        for text in ["s.a() = 1", "PRINT s.a()", "READ s.a()", "INPUT s.a()"] {
            assert!(
                matches!(
                    parse(text),
                    Err(BBCBasicError::SyntaxError { ref message, .. })
                        if message == "A whole array member is not supported: use an index"
                ),
                "{text}"
            );
        }
    }

    #[test]
    fn test_member_targets_of_input_read_and_swap() {
        let target = InputItem::Member(path("p", &["s$"], Vec::new()));
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
    fn test_member_path_targets_with_indices() {
        let element = InputItem::Member(path("s", &["p", "a"], vec![Expression::Integer(1)]));
        let nested = InputItem::Member(path("s", &["p", "t$"], Vec::new()));
        assert_eq!(
            parse("INPUT s.p.a(1), s.p.t$").unwrap(),
            vec![Statement::Input {
                items: vec![element.clone(), nested.clone()]
            }]
        );
        assert_eq!(
            parse("READ s.p.a(1), s.p.t$").unwrap(),
            vec![Statement::Read {
                variables: vec![element.clone(), nested.clone()]
            }]
        );
        assert_eq!(
            parse("SWAP s.p.a(1), s.p.t$").unwrap(),
            vec![Statement::Swap {
                first: element,
                second: nested,
            }]
        );
    }

    #[test]
    fn test_whole_structure_outside_dim_is_an_error() {
        for text in [
            "b{} = a{}",
            "IF 1 THEN b{} = a{}",
            "PROCa(a{x})",
            "LOCAL a{x}",
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

    #[test]
    fn test_structure_parameters_arguments_and_local() {
        assert_eq!(
            parse("DEF PROCmove(p{}, dx)").unwrap(),
            vec![Statement::DefProc {
                name: "move".to_string(),
                params: vec!["p{}".to_string(), "dx".to_string()],
            }]
        );
        assert!(matches!(
            &parse("DEF FNlen(p{}) = p.x").unwrap()[0],
            Statement::DefFn { params, .. } if params == &["p{}".to_string()]
        ));
        assert_eq!(
            parse("PROCmove(point{}, 5)").unwrap(),
            vec![Statement::ProcCall {
                name: "move".to_string(),
                args: vec![
                    Expression::Structure("point".to_string()),
                    Expression::Integer(5)
                ],
            }]
        );
        assert_eq!(
            parse("LOCAL s{}, n").unwrap(),
            vec![Statement::Local {
                variables: vec!["s{}".to_string(), "n".to_string()],
            }]
        );
        assert!(matches!(
            &parse("x = FNlen(a{})").unwrap()[0],
            Statement::Assignment { expression: Expression::FunctionCall { args, .. }, .. }
                if args == &[Expression::Structure("a".to_string())]
        ));
    }

    #[test]
    fn test_a_structure_parameter_name_must_be_a_structure_name() {
        for text in ["DEF PROCa(n%{})", "DEF FNa(s${}) = 1", "LOCAL P{}"] {
            assert!(
                matches!(parse(text), Err(BBCBasicError::SyntaxError { .. })),
                "{text}"
            );
        }
    }
}
