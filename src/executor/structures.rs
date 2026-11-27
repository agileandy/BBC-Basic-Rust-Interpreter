//! The run of structures (issue #191). The form is the form of BBC BASIC for
//! Windows: see `src/parser/structures.rs` for the manual page.
//!
//! The suffix of a member name gives its type, as for a variable: `%` is an
//! integer, `$` is a string, and no suffix is a real.

use super::{format_number, format_real, real_to_integer, Executor};
use crate::error::{BBCBasicError, Result};
use crate::parser::Expression;
use crate::variables::Variable;

impl Executor {
    /// Execute `DIM name{members}`
    pub(super) fn execute_dim_structure(&mut self, name: &str, members: &[String]) -> Result<()> {
        self.variables.dim_structure(name, members)
    }

    /// Execute `structure.member = expression`. The value gets the type of
    /// the member before the store.
    pub(super) fn execute_member_assignment(
        &mut self,
        structure: &str,
        member: &str,
        expression: &Expression,
    ) -> Result<()> {
        // Find the member first, so that an error in it comes before an
        // error in the expression
        self.variables.get_member(structure, member)?;
        let value = if member.ends_with('%') {
            Variable::Integer(self.eval_integer(expression)?)
        } else if member.ends_with('$') {
            Variable::String(self.eval_string(expression)?)
        } else {
            Variable::Real(self.eval_real(expression)?)
        };
        self.variables.set_member(structure, member, value)
    }

    /// The value of a member
    pub(super) fn member_value(&self, structure: &str, member: &str) -> Result<Variable> {
        self.variables.get_member(structure, member).cloned()
    }

    /// The value of a member as an integer. A string gives Type mismatch.
    pub(super) fn member_integer(&self, structure: &str, member: &str) -> Result<i32> {
        match self.member_value(structure, member)? {
            Variable::Integer(value) => Ok(value),
            Variable::Real(value) => real_to_integer(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The value of a member as a real. A string gives Type mismatch.
    pub(super) fn member_real(&self, structure: &str, member: &str) -> Result<f64> {
        match self.member_value(structure, member)? {
            Variable::Integer(value) => Ok(f64::from(value)),
            Variable::Real(value) => Ok(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The value of a member as a string. A number gives Type mismatch, as
    /// a numeric variable does in a string context (issue 146).
    pub(super) fn member_string(&self, structure: &str, member: &str) -> Result<String> {
        match self.member_value(structure, member)? {
            Variable::String(value) => Ok(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The PRINT text of a member
    pub(super) fn format_member(&self, structure: &str, member: &str) -> Result<String> {
        Ok(match self.member_value(structure, member)? {
            Variable::Integer(value) => value.to_string(),
            Variable::Real(value) => format_real(value, self.exact_reals),
            Variable::String(value) => value,
            _ => return Err(BBCBasicError::TypeMismatch),
        })
    }

    /// The LVAR lines of the structures: one line for each member, such as
    /// `point.x = 1.5`, in name order and in member order
    pub(super) fn structure_listing(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, members) in self.variables.structures() {
            for (member, value) in members {
                let value = match value {
                    Variable::Integer(value) => value.to_string(),
                    Variable::Real(value) => format_number(*value),
                    Variable::String(value) => format!("\"{value}\""),
                    _ => continue,
                };
                lines.push(format!("{name}.{member} = {value}"));
            }
        }
        lines
    }
}
