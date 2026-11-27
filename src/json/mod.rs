//! JSON extension functions (issue #102). These are NOT BBC BASIC.
//!
//! `LOADJSON$(file$)` gives the content of a JSON file as compact text.
//! `SAVEJSON file$, json$` writes a JSON text with an indentation of 2 spaces.
//! `JSONVALID(json$)` tells if a text is correct JSON.
//!
//! The parse step and the print steps are separate functions, so that other
//! JSON functions (a value at a path, a change to a text) can use them.
//!
//! The objects keep their keys in alphabetical order, because `serde_json`
//! has no `preserve_order` feature in this build.

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
}

impl From<JsonError> for BBCBasicError {
    fn from(error: JsonError) -> Self {
        match error {
            JsonError::FileNotFound(name) => BBCBasicError::FileNotFound(name),
            JsonError::BadJson(detail) => BBCBasicError::BadJson(detail),
            JsonError::WriteFailed(detail) => BBCBasicError::DiskError(detail),
        }
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
}
