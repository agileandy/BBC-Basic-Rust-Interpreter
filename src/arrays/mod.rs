//! Whole-array assignment and arithmetic (issue #187).
//!
//! The reference is the manual of BBC BASIC for Windows, page bbcwin2.html,
//! sections "Array arithmetic" and "Initialising arrays", and the function
//! `expra` of BBC BASIC for SDL (src/bbeval.c), which has the same rules:
//!
//! - An array expression is evaluated strictly from left to right.
//! - Each step stores its result in a buffer of the type of the target
//!   array. So `A%() *= 2.5` multiplies by 2, as the manual tells.
//! - All arrays in a statement must have the same number of elements, or
//!   the result is "Type mismatch". The dimensions can be different.
//! - A list of values sets the first elements in storage order (row by
//!   row), and the other elements do not change.
//!
//! This module has no access to the executor. The executor evaluates each
//! scalar operand one time and gives the values here.

use crate::error::{BBCBasicError, Result};
use crate::parser::BinaryOperator;
use crate::variables::{Variable, MAX_STRING_LENGTH};

/// The elements of an array, in storage order
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayData {
    Integer(Vec<i32>),
    Real(Vec<f64>),
    String(Vec<String>),
}

/// A scalar operand, evaluated one time
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
}

/// An operand of an array expression
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Array(ArrayData),
    Scalar(Value),
}

impl ArrayData {
    /// The elements of an array variable. A scalar gives Type mismatch.
    pub fn from_variable(variable: &Variable) -> Result<Self> {
        match variable {
            Variable::IntegerArray { values, .. } => Ok(Self::Integer(values.clone())),
            Variable::RealArray { values, .. } => Ok(Self::Real(values.clone())),
            Variable::StringArray { values, .. } => Ok(Self::String(values.clone())),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// An array variable with these elements and the dimensions
    pub fn into_variable(self, dimensions: Vec<usize>) -> Variable {
        match self {
            Self::Integer(values) => Variable::IntegerArray { values, dimensions },
            Self::Real(values) => Variable::RealArray { values, dimensions },
            Self::String(values) => Variable::StringArray { values, dimensions },
        }
    }

    /// The number of elements
    pub fn len(&self) -> usize {
        match self {
            Self::Integer(values) => values.len(),
            Self::Real(values) => values.len(),
            Self::String(values) => values.len(),
        }
    }

    /// True if the array has no elements
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True for a string array
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// An array of the same type and size, with each element 0 or ""
    fn zeroed(&self) -> Self {
        match self {
            Self::Integer(values) => Self::Integer(vec![0; values.len()]),
            Self::Real(values) => Self::Real(vec![0.0; values.len()]),
            Self::String(values) => Self::String(vec![String::new(); values.len()]),
        }
    }

    /// Element `index` as a scalar value
    fn value(&self, index: usize) -> Value {
        match self {
            Self::Integer(values) => Value::Number(f64::from(values[index])),
            Self::Real(values) => Value::Number(values[index]),
            Self::String(values) => Value::Text(values[index].clone()),
        }
    }

    /// Apply `op` to element `index` and `value`, and store the result in the
    /// element. `None` is a plain assignment.
    fn modify(&mut self, index: usize, op: Option<&BinaryOperator>, value: &Value) -> Result<()> {
        match (self, value) {
            (Self::Integer(values), Value::Number(number)) => {
                let result = match op {
                    None => *number,
                    Some(op) => calculate(f64::from(values[index]), op, *number)?,
                };
                values[index] = to_integer(result)?;
            }
            (Self::Real(values), Value::Number(number)) => {
                values[index] = match op {
                    None => *number,
                    Some(op) => calculate(values[index], op, *number)?,
                };
            }
            (Self::String(values), Value::Text(text)) => match op {
                None => values[index] = text.clone(),
                Some(BinaryOperator::Add) => {
                    if values[index].len() + text.len() > MAX_STRING_LENGTH {
                        return Err(BBCBasicError::StringTooLong);
                    }
                    values[index].push_str(text);
                }
                Some(_) => return Err(BBCBasicError::TypeMismatch),
            },
            _ => return Err(BBCBasicError::TypeMismatch),
        }
        Ok(())
    }

    /// Apply `op` to each element and the operand. An array operand must
    /// have the same number of elements and the same type class.
    fn apply(&mut self, op: Option<&BinaryOperator>, operand: &Operand) -> Result<()> {
        match operand {
            Operand::Array(array) => {
                if array.len() != self.len() || array.is_string() != self.is_string() {
                    return Err(BBCBasicError::TypeMismatch);
                }
                for index in 0..self.len() {
                    self.modify(index, op, &array.value(index))?;
                }
            }
            Operand::Scalar(value) => {
                for index in 0..self.len() {
                    self.modify(index, op, value)?;
                }
            }
        }
        Ok(())
    }
}

/// Convert a real to an integer. The fraction is truncated toward zero. A
/// value out of the integer range gives Too big, as an assignment does.
fn to_integer(value: f64) -> Result<i32> {
    let truncated = value.trunc();
    if truncated.is_finite() && truncated >= i32::MIN as f64 && truncated <= i32::MAX as f64 {
        Ok(truncated as i32)
    } else {
        Err(BBCBasicError::TooBig)
    }
}

/// Calculate `left op right` with the errors of the same scalar operation
fn calculate(left: f64, op: &BinaryOperator, right: f64) -> Result<f64> {
    let integers = || Ok::<_, BBCBasicError>((to_integer(left)?, to_integer(right)?));
    let value = match op {
        BinaryOperator::Add => left + right,
        BinaryOperator::Subtract => left - right,
        BinaryOperator::Multiply => left * right,
        BinaryOperator::Divide => {
            if right == 0.0 {
                return Err(BBCBasicError::DivisionByZero);
            }
            left / right
        }
        BinaryOperator::Power => {
            if left < 0.0 && right.fract() != 0.0 {
                return Err(BBCBasicError::LogRange);
            }
            if left == 0.0 && right < 0.0 {
                return Err(BBCBasicError::DivisionByZero);
            }
            left.powf(right)
        }
        BinaryOperator::IntegerDivide => {
            let (left, right) = integers()?;
            if right == 0 {
                return Err(BBCBasicError::DivisionByZero);
            }
            f64::from(left.checked_div(right).ok_or(BBCBasicError::TooBig)?)
        }
        BinaryOperator::Modulo => {
            let (left, right) = integers()?;
            if right == 0 {
                return Err(BBCBasicError::DivisionByZero);
            }
            f64::from(left.wrapping_rem(right))
        }
        BinaryOperator::And => {
            let (left, right) = integers()?;
            f64::from(left & right)
        }
        BinaryOperator::Or => {
            let (left, right) = integers()?;
            f64::from(left | right)
        }
        BinaryOperator::Eor => {
            let (left, right) = integers()?;
            f64::from(left ^ right)
        }
        _ => return Err(BBCBasicError::TypeMismatch),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BBCBasicError::TooBig)
    }
}

/// Calculate the new elements of `target` for the statement
/// `target() [op]= first rest..., list...`, and give them. The target does
/// not change, so that an error leaves it as it was.
///
/// `first` and `rest` are the array expression, from left to right. `list`
/// is the values after the first comma, for the elements 1, 2 and more. With
/// a list, only the first `list.len() + 1` elements change.
pub fn assign(
    target: &ArrayData,
    op: Option<&BinaryOperator>,
    first: &Operand,
    rest: &[(BinaryOperator, Operand)],
    list: &[Value],
) -> Result<ArrayData> {
    let mut buffer = target.zeroed();
    buffer.apply(None, first)?;
    for (step, operand) in rest {
        buffer.apply(Some(step), operand)?;
    }

    let count = if list.is_empty() {
        target.len()
    } else {
        if list.len() >= target.len() {
            return Err(BBCBasicError::SyntaxError {
                message: "Too many values for the array".to_string(),
                line: None,
            });
        }
        for (index, value) in list.iter().enumerate() {
            buffer.modify(index + 1, None, value)?;
        }
        list.len() + 1
    };

    let mut result = target.clone();
    for index in 0..count {
        result.modify(index, op, &buffer.value(index))?;
    }
    Ok(result)
}

// The array functions (issue #188). The reference is the manual of BBC BASIC
// for Windows: bbcwin7.html (SUM and SUMLEN), bbcwin6.html (MOD) and
// bbcwin4.html ("DIM as a function"). The errors are from the functions
// `item` and `dimfunc` of BBC BASIC for SDL (src/bbeval.c).

/// `SUM(array())`: the sum of the elements of a numeric array, or the join
/// of the elements of a string array. An integer sum out of the integer
/// range gives Too big, as an integer addition does in this interpreter.
pub fn sum(array: &Variable) -> Result<Value> {
    match array {
        Variable::IntegerArray { values, .. } => {
            let total: i64 = values.iter().map(|&value| i64::from(value)).sum();
            let total = i32::try_from(total).map_err(|_| BBCBasicError::TooBig)?;
            Ok(Value::Number(f64::from(total)))
        }
        Variable::RealArray { values, .. } => finite(values.iter().sum()).map(Value::Number),
        Variable::StringArray { values, .. } => {
            if values.iter().map(String::len).sum::<usize>() > MAX_STRING_LENGTH {
                return Err(BBCBasicError::StringTooLong);
            }
            Ok(Value::Text(values.concat()))
        }
        _ => Err(BBCBasicError::TypeMismatch),
    }
}

/// `SUMLEN(array$())`: the total length of the elements of a string array.
/// The total can be more than the string limit. A numeric array gives Type
/// mismatch.
pub fn sum_len(array: &Variable) -> Result<i32> {
    match array {
        Variable::StringArray { values, .. } => {
            let total: usize = values.iter().map(String::len).sum();
            i32::try_from(total).map_err(|_| BBCBasicError::TooBig)
        }
        _ => Err(BBCBasicError::TypeMismatch),
    }
}

/// `MOD(array())`: the square root of the sum of the squares of the
/// elements of a numeric array. A string array gives Type mismatch.
pub fn modulus(array: &Variable) -> Result<f64> {
    let squares: f64 = match array {
        Variable::IntegerArray { values, .. } => {
            values.iter().map(|&value| f64::from(value).powi(2)).sum()
        }
        Variable::RealArray { values, .. } => values.iter().map(|value| value.powi(2)).sum(),
        _ => return Err(BBCBasicError::TypeMismatch),
    };
    Ok(finite(squares)?.sqrt())
}

/// `DIM(array())` with no `n`: the number of dimensions. `DIM(array(), n)`:
/// the highest index of dimension `n`. An `n` out of the range 1 to the
/// number of dimensions gives Subscript ("Bad subscript" in BBC BASIC).
pub fn dimension(array: &Variable, n: Option<i32>) -> Result<i32> {
    let dimensions = array.dimensions().ok_or(BBCBasicError::TypeMismatch)?;
    let count = |value: usize| i32::try_from(value).map_err(|_| BBCBasicError::TooBig);
    let Some(n) = n else {
        return count(dimensions.len());
    };
    let size = n
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| dimensions.get(index))
        .ok_or(BBCBasicError::SubscriptOutOfRange)?;
    count(size - 1)
}

/// The value, or Too big for an infinite value
fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BBCBasicError::TooBig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(value: f64) -> Operand {
        Operand::Scalar(Value::Number(value))
    }

    #[test]
    fn test_copy_an_array_converts_to_the_target_type() {
        let target = ArrayData::Integer(vec![0, 0]);
        let source = Operand::Array(ArrayData::Real(vec![2.7, -2.7]));
        let result = assign(&target, None, &source, &[], &[]).unwrap();
        assert_eq!(result, ArrayData::Integer(vec![2, -2]));
    }

    #[test]
    fn test_fill_with_one_value() {
        let target = ArrayData::Real(vec![1.0, 2.0, 3.0]);
        let result = assign(&target, None, &number(0.5), &[], &[]).unwrap();
        assert_eq!(result, ArrayData::Real(vec![0.5, 0.5, 0.5]));
    }

    #[test]
    fn test_a_list_changes_only_the_first_elements() {
        let target = ArrayData::Integer(vec![9, 9, 9, 9]);
        let list = [Value::Number(2.0), Value::Number(3.0)];
        let result = assign(&target, None, &number(1.0), &[], &list).unwrap();
        assert_eq!(result, ArrayData::Integer(vec![1, 2, 3, 9]));
    }

    #[test]
    fn test_a_list_with_a_value_for_each_element_is_correct() {
        let target = ArrayData::Integer(vec![0, 0]);
        let list = [Value::Number(2.0)];
        let result = assign(&target, None, &number(1.0), &[], &list).unwrap();
        assert_eq!(result, ArrayData::Integer(vec![1, 2]));
    }

    #[test]
    fn test_a_list_with_too_many_values_is_a_syntax_error() {
        let target = ArrayData::Integer(vec![0, 0]);
        let list = [Value::Number(2.0), Value::Number(3.0)];
        let result = assign(&target, None, &number(1.0), &[], &list);
        assert!(matches!(result, Err(BBCBasicError::SyntaxError { .. })));
    }

    #[test]
    fn test_steps_are_from_left_to_right() {
        // 1 + 10 * 2 is 22 from left to right
        let target = ArrayData::Real(vec![0.0]);
        let rest = [
            (
                BinaryOperator::Add,
                Operand::Array(ArrayData::Real(vec![10.0])),
            ),
            (BinaryOperator::Multiply, number(2.0)),
        ];
        let result = assign(&target, None, &number(1.0), &rest, &[]).unwrap();
        assert_eq!(result, ArrayData::Real(vec![22.0]));
    }

    #[test]
    fn test_each_step_stores_in_the_target_type() {
        // 7 / 2 is 3 in an integer buffer, then 3 * 2 is 6
        let target = ArrayData::Integer(vec![0]);
        let rest = [
            (BinaryOperator::Divide, number(2.0)),
            (BinaryOperator::Multiply, number(2.0)),
        ];
        let result = assign(&target, None, &number(7.0), &rest, &[]).unwrap();
        assert_eq!(result, ArrayData::Integer(vec![6]));
    }

    #[test]
    fn test_compound_assignment_uses_the_old_elements() {
        let target = ArrayData::Integer(vec![3, 5]);
        let result = assign(
            &target,
            Some(&BinaryOperator::Multiply),
            &number(2.5),
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(result, ArrayData::Integer(vec![6, 10]));
    }

    #[test]
    fn test_join_strings() {
        let target = ArrayData::String(vec![String::new(); 2]);
        let first = Operand::Array(ArrayData::String(vec!["a".into(), "b".into()]));
        let rest = [(
            BinaryOperator::Add,
            Operand::Scalar(Value::Text("!".into())),
        )];
        let result = assign(&target, None, &first, &rest, &[]).unwrap();
        assert_eq!(result, ArrayData::String(vec!["a!".into(), "b!".into()]));
    }

    #[test]
    fn test_a_string_with_an_other_operator_is_a_type_mismatch() {
        let target = ArrayData::String(vec![String::new()]);
        let rest = [(
            BinaryOperator::Subtract,
            Operand::Scalar(Value::Text("x".into())),
        )];
        let first = Operand::Scalar(Value::Text("y".into()));
        let result = assign(&target, None, &first, &rest, &[]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_a_number_into_a_string_array_is_a_type_mismatch() {
        let target = ArrayData::String(vec![String::new()]);
        let result = assign(&target, None, &number(1.0), &[], &[]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_a_different_element_count_is_a_type_mismatch() {
        let target = ArrayData::Real(vec![0.0; 3]);
        let source = Operand::Array(ArrayData::Real(vec![0.0; 4]));
        let result = assign(&target, None, &source, &[], &[]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_a_string_array_into_a_numeric_array_is_a_type_mismatch() {
        let target = ArrayData::Real(vec![0.0]);
        let source = Operand::Array(ArrayData::String(vec![String::new()]));
        let result = assign(&target, None, &source, &[], &[]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_errors_of_the_scalar_operations() {
        for (op, left, right, error) in [
            (
                BinaryOperator::Divide,
                1.0,
                0.0,
                BBCBasicError::DivisionByZero,
            ),
            (
                BinaryOperator::IntegerDivide,
                1.0,
                0.0,
                BBCBasicError::DivisionByZero,
            ),
            (
                BinaryOperator::Modulo,
                1.0,
                0.0,
                BBCBasicError::DivisionByZero,
            ),
            (
                BinaryOperator::Multiply,
                1e300,
                1e300,
                BBCBasicError::TooBig,
            ),
            (BinaryOperator::Power, -8.0, 0.5, BBCBasicError::LogRange),
            (BinaryOperator::And, 3e9, 1.0, BBCBasicError::TooBig),
        ] {
            assert_eq!(calculate(left, &op, right), Err(error), "{op:?}");
        }
    }

    #[test]
    fn test_integer_operators() {
        assert_eq!(
            calculate(-17.0, &BinaryOperator::IntegerDivide, 5.0),
            Ok(-3.0)
        );
        assert_eq!(calculate(-17.0, &BinaryOperator::Modulo, 5.0), Ok(-2.0));
        assert_eq!(calculate(12.0, &BinaryOperator::And, 10.0), Ok(8.0));
        assert_eq!(calculate(12.0, &BinaryOperator::Or, 3.0), Ok(15.0));
        assert_eq!(calculate(12.0, &BinaryOperator::Eor, 5.0), Ok(9.0));
    }

    #[test]
    fn test_an_error_in_the_middle_leaves_the_target_unchanged() {
        let target = ArrayData::Real(vec![7.0, 7.0, 7.0]);
        let divisor = Operand::Array(ArrayData::Real(vec![1.0, 0.0, 1.0]));
        let result = assign(&target, Some(&BinaryOperator::Divide), &divisor, &[], &[]);
        assert_eq!(result, Err(BBCBasicError::DivisionByZero));
        assert_eq!(target, ArrayData::Real(vec![7.0, 7.0, 7.0]));
    }

    #[test]
    fn test_a_string_too_long() {
        let long = "x".repeat(40000);
        let target = ArrayData::String(vec![long.clone()]);
        let first = Operand::Scalar(Value::Text(long));
        let result = assign(&target, Some(&BinaryOperator::Add), &first, &[], &[]);
        assert_eq!(result, Err(BBCBasicError::StringTooLong));
    }
}
