//! JSON extension functions (issue #102). These are NOT BBC BASIC.
//!
//! `LOADJSON$(file$)` gives the content of a JSON file as compact text.
//! `SAVEJSON file$, json$` writes a JSON text with an indentation of 2 spaces.
//! `JSONVALID(json$)` tells if a text is correct JSON.
//!
//! The path functions (issue #103) read one value of a JSON text:
//! `JSONGET$(json$, path$)`, `JSONGET(json$, path$)`, `JSONTYPE$(json$, path$)`,
//! `JSONLEN(json$, path$)` and `JSONKEY$(json$, path$, n)`. A path has keys
//! with dots (`user.name`), array indices from 0 in square brackets
//! (`items[2]`), keys in JSON string form in square brackets (`["my key"]`),
//! or no text for the root.
//!
//! The parse step and the print steps are separate functions, so that other
//! JSON functions (a value at a path, a change to a text) can use them.
//!
//! The objects keep their keys in the order of the text, because `serde_json`
//! has the `preserve_order` feature in this build.

use crate::error::BBCBasicError;
use serde_json::Value;
use std::fs;

/// An error from a JSON function. The executor converts it to a BASIC error.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonError {
    /// The file cannot be read. The value is the file name.
    FileNotFound(String),
    /// The text is not JSON. The value is the detail from the parser.
    BadJson(String),
    /// The file cannot be written. The value is the detail.
    WriteFailed(String),
    /// The path has wrong syntax, or the value at the path does not exist.
    /// The value is the path text.
    BadPath(String),
    /// The value at the path has the wrong type for the function.
    TypeMismatch,
    /// An index is out of range.
    Subscript,
}

impl From<JsonError> for BBCBasicError {
    fn from(error: JsonError) -> Self {
        match error {
            JsonError::FileNotFound(name) => BBCBasicError::FileNotFound(name),
            JsonError::BadJson(detail) => BBCBasicError::BadJson(detail),
            JsonError::WriteFailed(detail) => BBCBasicError::DiskError(detail),
            JsonError::BadPath(path) => BBCBasicError::BadPath(path),
            JsonError::TypeMismatch => BBCBasicError::TypeMismatch,
            JsonError::Subscript => BBCBasicError::SubscriptOutOfRange,
        }
    }
}

/// One step of a path: a key of an object, or an index of an array.
#[derive(Debug, Clone, PartialEq)]
pub enum PathStep {
    Key(String),
    Index(usize),
}

/// A parsed path. `steps` gives the steps, so that a function that changes a
/// JSON text can go through the same path.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonPath {
    steps: Vec<PathStep>,
}

impl JsonPath {
    /// Parse a path text. Wrong syntax gives `BadPath` with the path text.
    pub fn parse(text: &str) -> Result<JsonPath, JsonError> {
        let bad = || JsonError::BadPath(text.to_string());
        let mut steps = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('[') {
                let (step, after) = parse_bracket(after).ok_or_else(bad)?;
                steps.push(step);
                rest = after;
            } else {
                // A key at the start has no dot. A key after a step has a dot.
                let key_text = if steps.is_empty() {
                    rest
                } else {
                    rest.strip_prefix('.').ok_or_else(bad)?
                };
                let end = key_text.find(['.', '[', ']']).unwrap_or(key_text.len());
                if end == 0 || key_text[end..].starts_with(']') {
                    return Err(bad());
                }
                steps.push(PathStep::Key(key_text[..end].to_string()));
                rest = &key_text[end..];
            }
        }
        Ok(JsonPath { steps })
    }

    /// The steps of the path, from the root.
    pub fn steps(&self) -> &[PathStep] {
        &self.steps
    }

    /// Give the value at the path, or `None` if it does not exist.
    pub fn find<'a>(&self, root: &'a Value) -> Option<&'a Value> {
        self.steps.iter().try_fold(root, |value, step| match step {
            PathStep::Key(key) => value.as_object()?.get(key),
            PathStep::Index(index) => value.as_array()?.get(*index),
        })
    }
}

/// Parse the text after `[` to the `]`: an index or a key in JSON string
/// form. Give the step and the text after the `]`.
fn parse_bracket(text: &str) -> Option<(PathStep, &str)> {
    if text.starts_with('"') {
        let mut escaped = false;
        let close = text.char_indices().skip(1).find_map(|(at, c)| {
            let is_close = c == '"' && !escaped;
            escaped = c == '\\' && !escaped;
            is_close.then_some(at)
        })?;
        let key: String = serde_json::from_str(&text[..=close]).ok()?;
        let rest = text[close + 1..].strip_prefix(']')?;
        Some((PathStep::Key(key), rest))
    } else {
        let close = text.find(']')?;
        let digits = &text[..close];
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some((PathStep::Index(digits.parse().ok()?), &text[close + 1..]))
    }
}

/// Give the value at a path of a JSON text, or `None` if it does not exist.
fn value_at(json: &str, path: &str) -> Result<Option<Value>, JsonError> {
    let root = parse(json)?;
    Ok(JsonPath::parse(path)?.find(&root).cloned())
}

/// Give the value at a path of a JSON text. A value that does not exist
/// gives `BadPath`.
fn existing_value_at(json: &str, path: &str) -> Result<Value, JsonError> {
    value_at(json, path)?.ok_or_else(|| JsonError::BadPath(path.to_string()))
}

/// `JSONGET$`: a string value as its text, and other values as compact JSON.
pub fn get_text(json: &str, path: &str) -> Result<String, JsonError> {
    Ok(match existing_value_at(json, path)? {
        Value::String(text) => text,
        value => to_compact(&value),
    })
}

/// `JSONGET`: a number. A string with a number in it gives the number.
/// `true` gives -1, `false` and `null` give 0. Other values give `TypeMismatch`.
pub fn get_number(json: &str, path: &str) -> Result<f64, JsonError> {
    match existing_value_at(json, path)? {
        Value::Number(number) => number.as_f64().ok_or(JsonError::TypeMismatch),
        Value::Bool(true) => Ok(-1.0),
        Value::Bool(false) | Value::Null => Ok(0.0),
        Value::String(text) => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .ok_or(JsonError::TypeMismatch),
        Value::Array(_) | Value::Object(_) => Err(JsonError::TypeMismatch),
    }
}

/// `JSONTYPE$`: the type name of the value at the path, or `missing`.
pub fn type_name(json: &str, path: &str) -> Result<&'static str, JsonError> {
    Ok(match value_at(json, path)? {
        None => "missing",
        Some(Value::Object(_)) => "object",
        Some(Value::Array(_)) => "array",
        Some(Value::String(_)) => "string",
        Some(Value::Number(_)) => "number",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Null) => "null",
    })
}

/// `JSONLEN`: the element count of an array, the key count of an object, or
/// the length of a string in bytes (as `LEN`). Other values give `TypeMismatch`.
pub fn length(json: &str, path: &str) -> Result<usize, JsonError> {
    match existing_value_at(json, path)? {
        Value::Array(elements) => Ok(elements.len()),
        Value::Object(members) => Ok(members.len()),
        Value::String(text) => Ok(text.len()),
        _ => Err(JsonError::TypeMismatch),
    }
}

/// `JSONKEY$`: the key at position `n` (from 0) of an object, in the order of
/// the text. A value that is not an object gives `TypeMismatch`.
pub fn key_at(json: &str, path: &str, n: i32) -> Result<String, JsonError> {
    match existing_value_at(json, path)? {
        Value::Object(members) => usize::try_from(n)
            .ok()
            .and_then(|index| members.keys().nth(index).cloned())
            .ok_or(JsonError::Subscript),
        _ => Err(JsonError::TypeMismatch),
    }
}

/// Parse a JSON text.
pub fn parse(text: &str) -> Result<Value, JsonError> {
    serde_json::from_str(text).map_err(|error| JsonError::BadJson(error.to_string()))
}

/// Print a value as compact JSON text, with no spaces or new lines outside strings.
pub fn to_compact(value: &Value) -> String {
    value.to_string()
}

/// Print a value as JSON text with an indentation of 2 spaces and a new line at the end.
pub fn to_indented(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

/// Tell if a text is correct JSON.
pub fn is_valid(text: &str) -> bool {
    parse(text).is_ok()
}

/// Read a JSON file and give its content as compact JSON text.
pub fn load_file(file_name: &str) -> Result<String, JsonError> {
    let text = fs::read_to_string(file_name)
        .map_err(|_| JsonError::FileNotFound(file_name.to_string()))?;
    Ok(to_compact(&parse(&text)?))
}

/// Write a JSON text to a file with an indentation of 2 spaces. A text that
/// is not JSON gives an error, and the file does not change.
pub fn save_file(file_name: &str, text: &str) -> Result<(), JsonError> {
    let value = parse(text)?;
    fs::write(file_name, to_indented(&value))
        .map_err(|error| JsonError::WriteFailed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gives_the_value_of_nested_json() {
        let value = parse(r#"{"a": [1, {"b": null}], "c": true}"#).unwrap();
        assert_eq!(value["a"][1]["b"], Value::Null);
        assert_eq!(value["c"], Value::Bool(true));
    }

    #[test]
    fn test_parse_of_bad_text_gives_bad_json_with_detail() {
        assert_eq!(
            parse("{bad"),
            Err(JsonError::BadJson(
                "key must be a string at line 1 column 2".to_string()
            ))
        );
    }

    #[test]
    fn test_compact_print_has_no_spaces_outside_strings() {
        let value = parse("{ \"a\" : [ 1 , 2 ] ,\n \"b\" : \"x y\" }").unwrap();
        assert_eq!(to_compact(&value), r#"{"a":[1,2],"b":"x y"}"#);
    }

    #[test]
    fn test_compact_print_keeps_quote_and_non_ascii_character() {
        let value = parse(r#"{"s":"Café \"Q\""}"#).unwrap();
        assert_eq!(to_compact(&value), "{\"s\":\"Café \\\"Q\\\"\"}");
    }

    #[test]
    fn test_indented_print_uses_2_spaces_and_ends_with_new_line() {
        let value = parse(r#"{"a":[1,2]}"#).unwrap();
        assert_eq!(
            to_indented(&value),
            "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n"
        );
    }

    #[test]
    fn test_is_valid_tells_correct_and_bad_text() {
        assert!(is_valid("[1, 2]"));
        assert!(!is_valid(""));
        assert!(!is_valid("[1,]"));
    }

    const DOC: &str = r#"{"user":{"name":"Ann","age":42,"admin":true,"tags":["a","b"]},
        "items":[{"price":1.5},{"price":2},{"price":"9.25"}],"my key":"spaced","none":null,
        "word":"abc","empty":{}}"#;

    fn key(text: &str) -> PathStep {
        PathStep::Key(text.to_string())
    }

    #[test]
    fn test_empty_path_is_the_root() {
        assert_eq!(JsonPath::parse("").unwrap().steps(), &[]);
    }

    #[test]
    fn test_path_with_keys_and_indices() {
        let path = JsonPath::parse("items[2].price").unwrap();
        assert_eq!(
            path.steps(),
            &[key("items"), PathStep::Index(2), key("price")]
        );
        let path = JsonPath::parse("a[0][1].b.c").unwrap();
        assert_eq!(
            path.steps(),
            &[
                key("a"),
                PathStep::Index(0),
                PathStep::Index(1),
                key("b"),
                key("c")
            ]
        );
    }

    #[test]
    fn test_path_with_bracket_key() {
        let path = JsonPath::parse(r#"["my key"].x["a.b"][3]"#).unwrap();
        assert_eq!(
            path.steps(),
            &[key("my key"), key("x"), key("a.b"), PathStep::Index(3)]
        );
        let path = JsonPath::parse(r#"["q\"t"]"#).unwrap();
        assert_eq!(path.steps(), &[key("q\"t")]);
    }

    #[test]
    fn test_path_with_wrong_syntax_gives_bad_path() {
        for text in [
            ".a", "a.", "a..b", "a[", "a[]", "a[x]", "a[-1]", "a[1", "a]", "[\"a\"", "[\"a]",
            "a[0]b", "[\"a\"]b",
        ] {
            assert_eq!(
                JsonPath::parse(text),
                Err(JsonError::BadPath(text.to_string())),
                "path {text}"
            );
        }
    }

    #[test]
    fn test_find_gives_the_value_or_none() {
        let value = parse(DOC).unwrap();
        let path = JsonPath::parse("user.tags[1]").unwrap();
        assert_eq!(path.find(&value), Some(&Value::String("b".to_string())));
        assert_eq!(JsonPath::parse("user.tags[2]").unwrap().find(&value), None);
        assert_eq!(JsonPath::parse("items.price").unwrap().find(&value), None);
        assert_eq!(JsonPath::parse("user[0]").unwrap().find(&value), None);
    }

    #[test]
    fn test_get_text_gives_a_string_raw_and_other_values_as_json() {
        assert_eq!(get_text(DOC, "user.name"), Ok("Ann".to_string()));
        assert_eq!(get_text(DOC, "user.age"), Ok("42".to_string()));
        assert_eq!(get_text(DOC, "user.tags"), Ok(r#"["a","b"]"#.to_string()));
        assert_eq!(get_text(DOC, "none"), Ok("null".to_string()));
        assert_eq!(get_text(DOC, r#"["my key"]"#), Ok("spaced".to_string()));
        assert_eq!(
            get_text(DOC, "user.nick"),
            Err(JsonError::BadPath("user.nick".to_string()))
        );
        assert!(matches!(get_text("{bad", ""), Err(JsonError::BadJson(_))));
    }

    #[test]
    fn test_get_number_gives_numbers_booleans_and_null() {
        assert_eq!(get_number(DOC, "user.age"), Ok(42.0));
        assert_eq!(get_number(DOC, "items[0].price"), Ok(1.5));
        assert_eq!(get_number(DOC, "items[2].price"), Ok(9.25));
        assert_eq!(get_number(DOC, "user.admin"), Ok(-1.0));
        assert_eq!(get_number("false", ""), Ok(0.0));
        assert_eq!(get_number(DOC, "none"), Ok(0.0));
    }

    #[test]
    fn test_get_number_of_other_values_gives_type_mismatch() {
        assert_eq!(get_number(DOC, "word"), Err(JsonError::TypeMismatch));
        assert_eq!(get_number(DOC, "user"), Err(JsonError::TypeMismatch));
        assert_eq!(get_number(r#""inf""#, ""), Err(JsonError::TypeMismatch));
        assert_eq!(
            get_number(DOC, "items[3]"),
            Err(JsonError::BadPath("items[3]".to_string()))
        );
    }

    #[test]
    fn test_type_name_gives_each_type_and_missing() {
        assert_eq!(type_name(DOC, ""), Ok("object"));
        assert_eq!(type_name(DOC, "items"), Ok("array"));
        assert_eq!(type_name(DOC, "word"), Ok("string"));
        assert_eq!(type_name(DOC, "user.age"), Ok("number"));
        assert_eq!(type_name(DOC, "user.admin"), Ok("boolean"));
        assert_eq!(type_name(DOC, "none"), Ok("null"));
        assert_eq!(type_name(DOC, "user.x.y"), Ok("missing"));
        assert_eq!(
            type_name(DOC, "a.."),
            Err(JsonError::BadPath("a..".to_string()))
        );
    }

    #[test]
    fn test_length_of_array_object_and_string() {
        assert_eq!(length(DOC, "items"), Ok(3));
        assert_eq!(length(DOC, "user"), Ok(4));
        assert_eq!(length(DOC, "empty"), Ok(0));
        assert_eq!(length(DOC, "word"), Ok(3));
        assert_eq!(length(DOC, "user.age"), Err(JsonError::TypeMismatch));
        assert_eq!(
            length(DOC, "nope"),
            Err(JsonError::BadPath("nope".to_string()))
        );
    }

    #[test]
    fn test_key_gives_the_nth_key_from_0() {
        assert_eq!(key_at(DOC, "user", 0), Ok("name".to_string()));
        assert_eq!(key_at(DOC, "user", 3), Ok("tags".to_string()));
        assert_eq!(key_at(DOC, "", 2), Ok("my key".to_string()));
        assert_eq!(key_at(DOC, "user", 4), Err(JsonError::Subscript));
        assert_eq!(key_at(DOC, "user", -1), Err(JsonError::Subscript));
        assert_eq!(key_at(DOC, "items", 0), Err(JsonError::TypeMismatch));
        assert_eq!(
            key_at(DOC, "nope", 0),
            Err(JsonError::BadPath("nope".to_string()))
        );
    }
}
