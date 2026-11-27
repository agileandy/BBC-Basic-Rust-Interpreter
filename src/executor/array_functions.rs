//! Evaluate the array functions SUM, SUMLEN, MOD and DIM (issue #188). The
//! parse is `parse_array_function` in `crate::parser`, and the element loops
//! are in `crate::arrays`. The reference is the manual of BBC BASIC for
//! Windows, page bbcwin2.html, "Array functions".

use super::{real_to_integer, Executor};
use crate::arrays::{self, Value};
use crate::error::{BBCBasicError, Result};
use crate::parser::Expression;

/// True for the name of an array function
pub(super) fn is_array_function(name: &str) -> bool {
    matches!(name, "SUM" | "SUMLEN" | "MOD" | "DIM")
}

/// The name of the first argument, if it is a whole array `a()`
fn array_argument(args: &[Expression]) -> Option<&str> {
    match args.first() {
        Some(Expression::ArrayAccess { name, indices }) if indices.is_empty() => Some(name),
        _ => None,
    }
}

/// True if the call gives a string: SUM of a string array
pub(super) fn gives_string(name: &str, args: &[Expression]) -> bool {
    name == "SUM" && array_argument(args).is_some_and(|array| array.ends_with('$'))
}

/// True if the call can give a real value: MOD, and SUM of a real array
pub(super) fn gives_real(name: &str, args: &[Expression]) -> bool {
    let real_array = |array: &str| !array.ends_with('%') && !array.ends_with('$');
    name == "MOD" || (name == "SUM" && array_argument(args).is_some_and(real_array))
}

impl Executor {
    /// Evaluate an array function. A first argument that is not a whole
    /// array gives Type mismatch, and an array with no DIM gives Array.
    fn eval_array_function(&mut self, name: &str, args: &[Expression]) -> Result<Value> {
        let array_name = array_argument(args).ok_or(BBCBasicError::TypeMismatch)?;
        let n = match args.get(1) {
            Some(expression) => Some(real_to_integer(self.eval_number(expression)?)?),
            None => None,
        };
        let array = self.variables.get_array(array_name)?;
        let count = |value: i32| Value::Number(f64::from(value));
        match name {
            "SUM" => arrays::sum(array),
            "SUMLEN" => arrays::sum_len(array).map(count),
            "MOD" => arrays::modulus(array).map(Value::Number),
            _ => arrays::dimension(array, n).map(count),
        }
    }

    /// The numeric value of an array function. A string gives Type mismatch.
    pub(super) fn array_function_number(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        match self.eval_array_function(name, args)? {
            Value::Number(value) => Ok(value),
            Value::Text(_) => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The string value of an array function. A number gives Type mismatch.
    pub(super) fn array_function_text(
        &mut self,
        name: &str,
        args: &[Expression],
    ) -> Result<String> {
        match self.eval_array_function(name, args)? {
            Value::Text(text) => Ok(text),
            Value::Number(_) => Err(BBCBasicError::TypeMismatch),
        }
    }
}
