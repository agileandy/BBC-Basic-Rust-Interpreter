//! Execute the matrix statements and evaluate the matrix functions
//! (extension, issue #190). The algorithms are in `crate::matrix`. This
//! file gets the arrays and stores the result.

use super::{real_to_integer, Executor};
use crate::arrays::ArrayData;
use crate::error::{BBCBasicError, Result};
use crate::matrix::{MatrixFunction, MatrixStatement, Numbers};
use crate::parser::Expression;

impl Executor {
    /// Execute `statement target(), source()`. The full result is calculated
    /// before the target changes, so an error leaves the target as it was.
    pub(super) fn execute_matrix_statement(
        &mut self,
        statement: MatrixStatement,
        target: &str,
        source: &str,
    ) -> Result<()> {
        let result = statement.apply(&self.array_numbers(source)?)?;

        let target_array = self.variables.get_array(target)?;
        let dimensions = target_array.dimensions().unwrap_or_default().to_vec();
        let data = match ArrayData::from_variable(target_array)? {
            ArrayData::Integer(values) if values.len() == result.len() => ArrayData::Integer(
                result
                    .into_iter()
                    .map(real_to_integer)
                    .collect::<Result<_>>()?,
            ),
            ArrayData::Real(values) if values.len() == result.len() => ArrayData::Real(result),
            _ => return Err(BBCBasicError::TypeMismatch),
        };
        self.variables
            .replace_array(target, data.into_variable(dimensions))
    }

    /// Tell if `name(...)` is a call of a matrix function. An array with
    /// that name has priority, so that old programs do not change.
    pub(super) fn is_matrix_call(&self, name: &str) -> bool {
        MatrixFunction::from_name(name).is_some() && !self.variables.has_array(name)
    }

    /// Evaluate a matrix function. Each argument must be an array reference
    /// such as `A()`.
    pub(super) fn eval_matrix_function(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        let function = MatrixFunction::from_name(name).expect("the caller checks the name");
        let count = function.argument_count();
        if args.len() != count {
            let plural = if count == 1 { "" } else { "s" };
            return Err(BBCBasicError::SyntaxError {
                message: format!("{name} requires {count} argument{plural}"),
                line: None,
            });
        }
        let mut arrays = Vec::with_capacity(count);
        for arg in args {
            match arg {
                Expression::ArrayAccess { name, indices } if indices.is_empty() => {
                    arrays.push(self.array_numbers(name)?);
                }
                _ => return Err(BBCBasicError::TypeMismatch),
            }
        }
        function.evaluate(&arrays)
    }

    /// The values and the dimensions of a numeric array. A string array
    /// gives Type mismatch.
    fn array_numbers(&self, name: &str) -> Result<Numbers> {
        let array = self.variables.get_array(name)?;
        let dimensions = array.dimensions().unwrap_or_default().to_vec();
        let values = match ArrayData::from_variable(array)? {
            ArrayData::Integer(values) => values.into_iter().map(f64::from).collect(),
            ArrayData::Real(values) => values,
            ArrayData::String(_) => return Err(BBCBasicError::TypeMismatch),
        };
        Ok(Numbers { values, dimensions })
    }
}
