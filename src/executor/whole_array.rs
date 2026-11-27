//! Execute a whole-array statement (issue #187). The element loops are in
//! `crate::arrays`. This file gets the arrays, evaluates each scalar operand
//! one time, and stores the result.

use super::Executor;
use crate::arrays::{self, ArrayData, Operand, Value};
use crate::error::Result;
use crate::parser::{ArrayExpression, ArrayOperand, BinaryOperator, Expression};

impl Executor {
    /// Execute `target() [op]= value`. The full result is calculated before
    /// the target changes, so an error leaves the target as it was.
    pub(super) fn execute_array_assign(
        &mut self,
        target: &str,
        operator: Option<&BinaryOperator>,
        value: &ArrayExpression,
    ) -> Result<()> {
        let target_array = self.variables.get_array(target)?;
        let dimensions = target_array.dimensions().unwrap_or_default().to_vec();
        let data = ArrayData::from_variable(target_array)?;
        let text = data.is_string();

        let first = self.array_operand(&value.first, text)?;
        let mut rest = Vec::with_capacity(value.rest.len());
        for (op, operand) in &value.rest {
            rest.push((op.clone(), self.array_operand(operand, text)?));
        }
        let mut list = Vec::with_capacity(value.list.len());
        for expression in &value.list {
            list.push(self.array_value(expression, text)?);
        }

        let result = arrays::assign(&data, operator, &first, &rest, &list)?;
        self.variables
            .replace_array(target, result.into_variable(dimensions))
    }

    /// The value of an operand: a copy of an array, or a scalar of the type
    /// class of the target
    fn array_operand(&mut self, operand: &ArrayOperand, text: bool) -> Result<Operand> {
        match operand {
            ArrayOperand::Array(name) => Ok(Operand::Array(ArrayData::from_variable(
                self.variables.get_array(name)?,
            )?)),
            ArrayOperand::Scalar(expression) => {
                Ok(Operand::Scalar(self.array_value(expression, text)?))
            }
        }
    }

    /// Evaluate a scalar as a string for a string array, or as a number
    fn array_value(&mut self, expression: &Expression, text: bool) -> Result<Value> {
        if text {
            Ok(Value::Text(self.eval_string(expression)?))
        } else {
            Ok(Value::Number(self.eval_real(expression)?))
        }
    }
}
