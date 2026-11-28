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
//! Issue #193 adds the forms of the sections "Arrays of structures",
//! "Structure prototypes" and "Copying a structure" on the same page:
//! `DIM t{(10) one, two%}` with the member `t{(5)}.two%`, `DIM b{} = a{}`,
//! `DIM c{(1, 2)} = a{}`, `DIM n{name$, pos{} = a{}}`, and the copy
//! `b{} = a{}`. An element `t{(5)}` is also an argument for a structure
//! parameter. A whole array of structures, `t{()}`, and an array of
//! structures as a member give a syntax error.

use super::whole_array::compound_operator;
use super::{
    find_closing_bracket, missing_value_at_a_comma, parse_comma_separated_expressions,
    parse_expression, tokens_text, DimItem, Expression, InputItem, Statement,
};
use crate::error::{BBCBasicError, Result};
use crate::tokenizer::{is_short_form, Token};

/// A member path, such as `shape.pos.x` or `net.layer.weights(I%, J%)`
/// (issue #192). Only the last member can have indices: it is an array
/// member. A member before it is a sub-structure. The structure can be an
/// element of an array of structures, such as `t{(I%)}.x` (issue #193).
/// A path with no members is a whole structure or element, such as `a{}`
/// or `t{(1)}`.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberPath {
    /// The name of the structure
    pub structure: String,
    /// The indices of an element of an array of structures. An empty list
    /// is a structure that is not an array.
    pub element: Vec<Expression>,
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
    /// A sub-structure from a prototype, such as `pos{} = a{}` (issue #193)
    Prototype { name: String, prototype: MemberPath },
}

impl MemberDecl {
    /// The name of the member
    pub fn name(&self) -> &str {
        match self {
            MemberDecl::Scalar(name)
            | MemberDecl::Array { name, .. }
            | MemberDecl::Structure { name, .. }
            | MemberDecl::Prototype { name, .. } => name,
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
/// structure or to an element. These references are valid, and each other
/// reference gives a syntax error:
/// - a member of an element, such as `t{(1)}.x` (issue #193)
/// - the 2 sides of a copy `b{} = a{}` that starts a statement (issue #193)
/// - `name{}` in a statement with PROC, FN, DEF or LOCAL (issue #194)
/// - an element with no member, `t{(1)}`, as a whole argument of a PROC or
///   FN (issue #193)
pub(super) fn check_no_whole_structure(tokens: &[Token], line_number: Option<u16>) -> Result<()> {
    if tokens.contains(&Token::Keyword(0xDE)) {
        return Ok(());
    }
    let routine = has_routine_keyword(tokens);
    let copies = copy_braces(tokens);
    let valid = tokens.iter().enumerate().all(|(index, token)| {
        *token != Token::Separator('{')
            || copies.contains(&index)
            || is_valid_reference(tokens, index, routine)
    });
    if !valid {
        return Err(syntax_error(
            "A reference to a whole structure is not supported",
            line_number,
        ));
    }
    Ok(())
}

/// The position of the `}` of the reference whose `{` is at `index`:
/// `name{}` or `name{(indices)}`. `None` is a reference that is not
/// complete: the parse gives its error.
fn reference_end(tokens: &[Token], index: usize) -> Option<usize> {
    match tokens.get(index + 1)? {
        Token::Separator('}') => Some(index + 1),
        Token::Separator('(') => {
            let close = find_closing_bracket(tokens, index + 1)?;
            (tokens.get(close + 1) == Some(&Token::Separator('}'))).then_some(close + 1)
        }
        _ => None,
    }
}

/// The positions of the `{` of each copy `b{} = a{}` (issue #193): a
/// reference that starts a statement and has `=` after it, and the
/// reference after the `=`. A statement starts at the first token, after
/// `:`, and after THEN or ELSE.
fn copy_braces(tokens: &[Token]) -> Vec<usize> {
    let mut braces = Vec::new();
    for start in 0..tokens.len() {
        let starts_statement = start == 0
            || matches!(
                tokens[start - 1],
                Token::Separator(':') | Token::Keyword(0x8B | 0x8C)
            );
        if !starts_statement
            || !matches!(tokens[start], Token::Identifier(_))
            || tokens.get(start + 1) != Some(&Token::Separator('{'))
        {
            continue;
        }
        let Some(end) = reference_end(tokens, start + 1) else {
            continue;
        };
        if tokens.get(end + 1) != Some(&Token::Operator('=')) {
            continue;
        }
        braces.push(start + 1);
        if matches!(tokens.get(end + 2), Some(Token::Identifier(_)))
            && tokens.get(end + 3) == Some(&Token::Separator('{'))
        {
            braces.push(end + 3);
        }
    }
    braces
}

/// True if the `{` at `index` is after a name and is a member of an
/// element, a `name{}` in a statement with a PROC, FN, DEF or LOCAL
/// (`routine`), or an element that is a whole argument in such a statement
fn is_valid_reference(tokens: &[Token], index: usize, routine: bool) -> bool {
    if index == 0 || !matches!(tokens[index - 1], Token::Identifier(_)) {
        return false;
    }
    let Some(end) = reference_end(tokens, index) else {
        // An element that is not complete: the parse gives the error
        return tokens.get(index + 1) == Some(&Token::Separator('('));
    };
    if end == index + 1 {
        return routine;
    }
    if matches!(tokens.get(end + 1), Some(Token::Member(structure, _)) if structure.is_empty()) {
        return true;
    }
    routine
        && index >= 2
        && matches!(tokens[index - 2], Token::Separator('(' | ','))
        && matches!(tokens.get(end + 1), Some(Token::Separator(')' | ',')))
}

/// True if the tokens have PROC, FN, DEF or LOCAL
fn has_routine_keyword(tokens: &[Token]) -> bool {
    tokens
        .iter()
        .any(|token| matches!(token, Token::Keyword(0xF2 | 0xA4 | 0xDD | 0xEA)))
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
    // An array of structures has its top indices in brackets after the `{`
    // (issue #193)
    let (dimensions, start) = match tokens.get(1) {
        Some(Token::Separator('(')) => {
            let close = find_closing_bracket(tokens, 1)
                .ok_or_else(|| syntax_error("Expected ')' in DIM", line_number))?;
            let dimensions = parse_comma_separated_expressions(&tokens[2..close], line_number)?;
            (dimensions, close + 1)
        }
        _ => (Vec::new(), 1),
    };
    if let Some((prototype, last)) = parse_prototype(tokens, start, line_number)? {
        let item = DimItem::StructurePrototype {
            name,
            dimensions,
            prototype,
        };
        return Ok((item, last + 1));
    }
    let (members, close) = parse_member_list(tokens, start, line_number)?;
    let item = if dimensions.is_empty() {
        DimItem::Structure { name, members }
    } else {
        DimItem::StructureArray {
            name,
            dimensions,
            members,
        }
    };
    Ok((item, close + 1))
}

/// If the tokens at `start` are `} = a{}`, give the prototype path and the
/// position of its last token (issue #193). The prototype is a structure or
/// an element of an array of structures.
fn parse_prototype(
    tokens: &[Token],
    start: usize,
    line_number: Option<u16>,
) -> Result<Option<(MemberPath, usize)>> {
    match tokens.get(start..start + 2) {
        Some([Token::Separator('}'), Token::Operator('=')]) => {
            parse_structure(tokens, start + 2, line_number).map(Some)
        }
        _ => Ok(None),
    }
}

/// Parse a whole structure or element at `pos`, such as `a{}` or `t{(1)}`.
/// Give the path, which has no members, and the position of its last token.
/// Other tokens give a syntax error.
fn parse_structure(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
) -> Result<(MemberPath, usize)> {
    let expected = || syntax_error("Expected a structure after '='", line_number);
    if !starts_member_path(tokens, pos) || matches!(tokens[pos], Token::Member(..)) {
        return Err(expected());
    }
    let (path, last) = parse_member_path(tokens, pos, line_number)?;
    if !path.members.is_empty() {
        return Err(expected());
    }
    Ok((path, last))
}

/// Parse the members of a sub-structure. The tokens start at the `{`. Give
/// the members and the position of the `}`.
fn parse_members(tokens: &[Token], line_number: Option<u16>) -> Result<(Vec<MemberDecl>, usize)> {
    if let Some(Token::Separator('(')) = tokens.get(1) {
        return Err(syntax_error(
            "Arrays of structures are not supported",
            line_number,
        ));
    }
    parse_member_list(tokens, 1, line_number)
}

/// Parse the member list that starts at `start`, to the `}`. Give the
/// members and the position of the `}`.
fn parse_member_list(
    tokens: &[Token],
    start: usize,
    line_number: Option<u16>,
) -> Result<(Vec<MemberDecl>, usize)> {
    let mut members: Vec<MemberDecl> = Vec::new();
    let mut pos = start;
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
            if let Some((prototype, last)) = parse_prototype(&tokens[pos + 1..], 1, line_number)? {
                return Ok((MemberDecl::Prototype { name, prototype }, pos + 1 + last));
            }
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
///
/// The path can also start at a name with `{}` or with an element index,
/// such as `a{}`, `t{(1)}` or `t{(1)}.x` (issue #193). Use
/// `starts_member_path` first.
pub(super) fn parse_member_path(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
) -> Result<(MemberPath, usize)> {
    let (mut path, pos) = match tokens.get(pos) {
        Some(Token::Member(structure, members)) if !structure.is_empty() => {
            let path = MemberPath {
                structure: structure.clone(),
                element: Vec::new(),
                members: members.clone(),
                indices: Vec::new(),
            };
            (path, pos)
        }
        Some(Token::Identifier(name)) if starts_member_path(tokens, pos) => {
            let (element, close) = parse_element(tokens, pos, line_number)?;
            let mut path = MemberPath {
                structure: name.clone(),
                element,
                members: Vec::new(),
                indices: Vec::new(),
            };
            match tokens.get(close + 1) {
                Some(Token::Member(structure, members)) if structure.is_empty() => {
                    path.members = members.clone();
                    (path, close + 1)
                }
                _ => return Ok((path, close)),
            }
        }
        _ => return Err(syntax_error("Expected variable name", line_number)),
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

/// True if a member path starts at `pos`: a member token with a structure
/// name, or a name with `{}` or `{(` (issue #193)
pub(super) fn starts_member_path(tokens: &[Token], pos: usize) -> bool {
    match tokens.get(pos) {
        Some(Token::Member(structure, _)) => !structure.is_empty(),
        Some(Token::Identifier(_)) => matches!(
            tokens.get(pos + 1..pos + 3),
            Some([Token::Separator('{'), Token::Separator('(' | '}')])
        ),
        _ => false,
    }
}

/// Parse the part `name{}` or `name{(indices)}` at `pos`. Give the element
/// indices, which are empty for `name{}`, and the position of the `}`.
fn parse_element(
    tokens: &[Token],
    pos: usize,
    line_number: Option<u16>,
) -> Result<(Vec<Expression>, usize)> {
    if tokens.get(pos + 2) == Some(&Token::Separator('}')) {
        return Ok((Vec::new(), pos + 2));
    }
    let close = find_closing_bracket(tokens, pos + 2)
        .ok_or_else(|| syntax_error("Expected ')' after the structure index", line_number))?;
    if close == pos + 3 {
        return Err(syntax_error(
            "A whole array of structures is not supported",
            line_number,
        ));
    }
    if tokens.get(close + 1) != Some(&Token::Separator('}')) {
        return Err(syntax_error(
            "Expected '}' after the structure index",
            line_number,
        ));
    }
    let element = parse_comma_separated_expressions(&tokens[pos + 3..close], line_number)?;
    Ok((element, close + 1))
}

/// Parse `point.x = expr` or `s.a(1) = expr` (issues #191 and #192), or
/// the copy `b{} = a{}` (issue #193). The tokens start at the member.
pub(super) fn parse_member_assignment(
    tokens: &[Token],
    line_number: Option<u16>,
) -> Result<Statement> {
    let (target, last) = parse_member_path(tokens, 0, line_number)?;
    // A compound assignment to a member: p.x *= 2 (issue #204)
    if let Some(operator) = compound_operator(tokens, last + 1) {
        if !target.members.is_empty() {
            return Ok(Statement::CompoundAssignment {
                target: InputItem::Member(target),
                operator,
                expression: parse_expression(&tokens[last + 3..])?,
            });
        }
    }
    if tokens.get(last + 1) != Some(&Token::Operator('=')) {
        return Err(syntax_error("Expected '='", line_number));
    }
    if !target.members.is_empty() {
        return Ok(Statement::MemberAssignment {
            target,
            expression: parse_expression(&tokens[last + 2..])?,
        });
    }
    let (source, end) = parse_structure(tokens, last + 2, line_number)?;
    if end + 1 != tokens.len() {
        return Err(syntax_error("Expected a structure after '='", line_number));
    }
    Ok(Statement::StructureCopy { target, source })
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
    if !starts_member_path(tokens, 0) {
        return Ok(None);
    }
    let (path, last) = parse_member_path(tokens, 0, line_number)?;
    Ok((last + 1 == tokens.len()).then_some(InputItem::Member(path)))
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
            element: Vec::new(),
            members: members.iter().map(|member| member.to_string()).collect(),
            indices,
        }
    }

    /// The path of an element of an array of structures (issue #193)
    fn element_path(structure: &str, element: Vec<Expression>, members: &[&str]) -> MemberPath {
        MemberPath {
            element,
            ..path(structure, members, Vec::new())
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
        // A copy, b{} = a{}, is valid since issue #193
        for text in ["PRINT a{}", "x = a{}", "PROCa(a{x})", "LOCAL a{x}"] {
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
    fn test_dim_of_an_array_of_structures() {
        let statements = parse("DIM t{(10, N%) a(2), p{x}}").unwrap();
        assert_eq!(
            statements,
            vec![Statement::Dim {
                items: vec![DimItem::StructureArray {
                    name: "t".to_string(),
                    dimensions: vec![
                        Expression::Integer(10),
                        Expression::Variable("N%".to_string())
                    ],
                    members: vec![
                        MemberDecl::Array {
                            name: "a".to_string(),
                            dimensions: vec![Expression::Integer(2)],
                        },
                        MemberDecl::Structure {
                            name: "p".to_string(),
                            members: vec![MemberDecl::Scalar("x".to_string())],
                        },
                    ],
                }]
            }]
        );
    }

    #[test]
    fn test_dim_of_structures_from_a_prototype() {
        let whole = path("a", &[], Vec::new());
        let statements = parse("DIM b{} = a{}, c{(3)} = t{(1)}, n{s$, p{} = a{}}").unwrap();
        assert_eq!(
            statements,
            vec![Statement::Dim {
                items: vec![
                    DimItem::StructurePrototype {
                        name: "b".to_string(),
                        dimensions: Vec::new(),
                        prototype: whole.clone(),
                    },
                    DimItem::StructurePrototype {
                        name: "c".to_string(),
                        dimensions: vec![Expression::Integer(3)],
                        prototype: element_path("t", vec![Expression::Integer(1)], &[]),
                    },
                    DimItem::Structure {
                        name: "n".to_string(),
                        members: vec![
                            MemberDecl::Scalar("s$".to_string()),
                            MemberDecl::Prototype {
                                name: "p".to_string(),
                                prototype: whole,
                            },
                        ],
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_members_of_elements_and_the_copy() {
        let element = element_path(
            "t",
            vec![Expression::Variable("I%".to_string())],
            &["p", "x"],
        );
        assert_eq!(
            parse("t{(I%)}.p.x = t{(1, 2)}.a(3)").unwrap(),
            vec![Statement::MemberAssignment {
                target: element.clone(),
                expression: Expression::Member(Box::new(MemberPath {
                    indices: vec![Expression::Integer(3)],
                    ..element_path(
                        "t",
                        vec![Expression::Integer(1), Expression::Integer(2)],
                        &["a"]
                    )
                })),
            }]
        );
        assert_eq!(
            parse("READ t{(I%)}.p.x").unwrap(),
            vec![Statement::Read {
                variables: vec![InputItem::Member(element)]
            }]
        );
        assert_eq!(
            parse("b{} = t{(1)}").unwrap(),
            vec![Statement::StructureCopy {
                target: path("b", &[], Vec::new()),
                source: element_path("t", vec![Expression::Integer(1)], &[]),
            }]
        );
        assert_eq!(
            parse("PROCa(t{(1)})").unwrap(),
            vec![Statement::ProcCall {
                name: "a".to_string(),
                args: vec![Expression::Member(Box::new(element_path(
                    "t",
                    vec![Expression::Integer(1)],
                    &[]
                )))],
            }]
        );
    }

    #[test]
    fn test_bad_forms_of_an_element_and_a_copy() {
        for (text, message) in [
            ("PRINT t{(1).x", "Expected '}' after the structure index"),
            ("PRINT t{(1}.x", "Expected ')' after the structure index"),
            (
                "t{()}.x = 1",
                "A whole array of structures is not supported",
            ),
            ("b{} = 1", "Expected a structure after '='"),
            ("b{} = a{} + 1", "Expected a structure after '='"),
            ("b{} = a.p", "Expected a structure after '='"),
            ("DIM b{} = a.p", "Expected a structure after '='"),
        ] {
            assert!(
                matches!(
                    parse(text),
                    Err(BBCBasicError::SyntaxError { message: ref found, .. }) if found == message
                ),
                "{text}: {:?}",
                parse(text)
            );
        }
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
