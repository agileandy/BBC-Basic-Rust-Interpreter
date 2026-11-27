//! The run of structures (issue #191). The form is the form of BBC BASIC for
//! Windows: see `src/parser/structures.rs` for the manual page.
//!
//! The suffix of a member name gives its type, as for a variable: `%` is an
//! integer, `$` is a string, and no suffix is a real.

use super::{format_number, format_real, real_to_integer, Executor};
use crate::error::{BBCBasicError, Result};
use crate::parser::{Expression, InputItem};
use crate::variables::{StructureId, Variable};
use std::collections::HashMap;

/// The value of one argument of a PROC or FN call
#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    /// A number or a string, for a parameter by value
    Value(Variable),
    /// A whole structure such as `point{}`, for a structure parameter
    /// (issue #194). The routine uses the structure of the caller.
    Structure(StructureId),
}

impl Executor {
    /// Execute `DIM name{members}`. A DIM of a LOCAL structure makes a
    /// structure that the routine of the LOCAL removes at its end.
    pub(super) fn execute_dim_structure(&mut self, name: &str, members: &[String]) -> Result<()> {
        let local = self.variables.is_local_structure_without_dim(name);
        let id = self.variables.dim_structure(name, members)?;
        if local {
            // The innermost frame that saved the name has the LOCAL
            if let Some(frame) = self
                .local_stack
                .iter_mut()
                .rev()
                .find(|frame| frame.saved_structures.contains_key(name))
            {
                frame.local_structures.push(id);
            }
        }
        Ok(())
    }

    /// The structure of a whole structure argument, such as `point{}` in
    /// `PROCmove(point{}, 5)` (issue #194). The manual of BBC BASIC for
    /// Windows, "General information", section "Passing structures to
    /// procedures and functions"
    /// (https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin2.html#structures),
    /// says that the structure is passed by reference. The manual names no
    /// error for an argument that is not a structure: it gives Bad use of
    /// structure.
    pub(super) fn structure_argument(&self, argument: &Expression) -> Result<StructureId> {
        match argument {
            Expression::Structure(name) => self.variables.structure_id(name),
            _ => Err(BBCBasicError::BadStructure),
        }
    }

    /// Make the structure parameter `name{}` a second name for the structure
    /// of the caller. `declare_local` saved the old entry of the name first.
    pub(super) fn bind_structure_parameter(&mut self, param: &str, id: StructureId) {
        let name = param.strip_suffix("{}").unwrap_or(param);
        self.variables.bind_structure(name, id);
    }

    /// Execute `LOCAL name{}`: save the structure entry of the name in the
    /// current frame, and make the name a structure with no DIM. The manual
    /// page is the page of `structure_argument`, section "LOCAL and PRIVATE
    /// structures". A second LOCAL for the name in the frame keeps the first
    /// saved entry. The caller checks that a frame is open.
    pub(super) fn declare_local_structure(&mut self, name: &str) {
        let variables = &self.variables;
        if let Some(frame) = self.local_stack.last_mut() {
            frame
                .saved_structures
                .entry(name.to_string())
                .or_insert_with(|| variables.save_structure(name));
        }
        self.variables.declare_local_structure(name);
    }

    /// At the exit of a frame, put back the saved structure names and
    /// remove the LOCAL structures of the frame
    pub(super) fn restore_structures(
        &mut self,
        saved: HashMap<String, Option<Option<StructureId>>>,
        local: Vec<StructureId>,
    ) {
        for (name, entry) in saved {
            self.variables.restore_structure(name, entry);
        }
        for id in local {
            self.variables.remove_structure(id);
        }
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
    /// and a flag that is true for a number, as `format_expression` gives
    pub(super) fn format_member(&self, structure: &str, member: &str) -> Result<(String, bool)> {
        Ok(match self.member_value(structure, member)? {
            Variable::Integer(value) => (value.to_string(), true),
            Variable::Real(value) => (format_real(value, self.exact_reals), true),
            Variable::String(value) => (value, false),
            _ => return Err(BBCBasicError::TypeMismatch),
        })
    }

    /// Check that a member target of INPUT, READ or INPUT# exists, before
    /// the statement takes a reply, a DATA value or a record. Another target
    /// needs no check here.
    pub(super) fn check_member_target(&self, target: &InputItem) -> Result<()> {
        if let InputItem::Member { structure, member } = target {
            self.variables.get_member(structure, member)?;
        }
        Ok(())
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
