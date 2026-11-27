//! The run of structures (issues #191 and #192). The form is the form of BBC
//! BASIC for Windows: see `src/parser/structures.rs` for the manual page.
//!
//! The suffix of a member name gives its type, as for a variable: `%` is an
//! integer, `$` is a string, and no suffix is a real.

use super::{format_number, format_real, real_to_integer, Executor};
use crate::error::{BBCBasicError, Result};
use crate::parser::{Expression, InputItem, MemberDecl, MemberPath};
use crate::variables::{MemberValue, Members, StructureId, Variable};
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
    /// structure that the routine of the LOCAL removes at its end. All the
    /// array sizes are evaluated before the structure exists, so an error
    /// declares no structure (issue #192).
    pub(super) fn execute_dim_structure(
        &mut self,
        name: &str,
        members: &[MemberDecl],
    ) -> Result<()> {
        let members = self.make_members(members)?;
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

    /// The members of a DIM, with each value at 0 or an empty string
    fn make_members(&mut self, members: &[MemberDecl]) -> Result<Members> {
        let mut values = Members::with_capacity(members.len());
        for member in members {
            let value = match member {
                MemberDecl::Scalar(name) => MemberValue::scalar(name),
                MemberDecl::Array { name, dimensions } => {
                    let sizes = self.eval_dimensions(dimensions)?;
                    MemberValue::array(name, sizes)
                }
                MemberDecl::Structure { members, .. } => {
                    MemberValue::Structure(self.make_members(members)?)
                }
            };
            values.push((member.name().to_string(), value));
        }
        Ok(values)
    }

    /// The index values of a member path. A negative index gives Subscript
    /// out of range, as for an array.
    pub(super) fn member_indices(&mut self, path: &MemberPath) -> Result<Vec<usize>> {
        let mut values = Vec::with_capacity(path.indices.len());
        for index in &path.indices {
            let value = self.eval_integer(index)?;
            if value < 0 {
                return Err(BBCBasicError::SubscriptOutOfRange);
            }
            values.push(value as usize);
        }
        Ok(values)
    }

    /// Store a value in a member, or in an element of an array member
    pub(super) fn store_member(&mut self, path: &MemberPath, value: Variable) -> Result<()> {
        let indices = self.member_indices(path)?;
        self.variables
            .set_member(&path.structure, &path.members, &indices, value)
    }

    /// Execute `structure.member = expression`. The value gets the type of
    /// the member before the store.
    pub(super) fn execute_member_assignment(
        &mut self,
        target: &MemberPath,
        expression: &Expression,
    ) -> Result<()> {
        // Find the member first, so that an error in it comes before an
        // error in the expression. The indices are evaluated one time.
        let indices = self.member_indices(target)?;
        let (structure, path) = (&target.structure, &target.members);
        self.variables.get_member(structure, path, &indices)?;
        let member = target.member();
        let value = if member.ends_with('%') {
            Variable::Integer(self.eval_integer(expression)?)
        } else if member.ends_with('$') {
            Variable::String(self.eval_string(expression)?)
        } else {
            Variable::Real(self.eval_real(expression)?)
        };
        self.variables.set_member(structure, path, &indices, value)
    }

    /// The value of a member, or of an element of an array member
    pub(super) fn member_value(&mut self, path: &MemberPath) -> Result<Variable> {
        let indices = self.member_indices(path)?;
        self.variables
            .get_member(&path.structure, &path.members, &indices)
    }

    /// The value of a member as an integer. A string gives Type mismatch.
    pub(super) fn member_integer(&mut self, path: &MemberPath) -> Result<i32> {
        match self.member_value(path)? {
            Variable::Integer(value) => Ok(value),
            Variable::Real(value) => real_to_integer(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The value of a member as a real. A string gives Type mismatch.
    pub(super) fn member_real(&mut self, path: &MemberPath) -> Result<f64> {
        match self.member_value(path)? {
            Variable::Integer(value) => Ok(f64::from(value)),
            Variable::Real(value) => Ok(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The value of a member as a string. A number gives Type mismatch, as
    /// a numeric variable does in a string context (issue 146).
    pub(super) fn member_string(&mut self, path: &MemberPath) -> Result<String> {
        match self.member_value(path)? {
            Variable::String(value) => Ok(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The PRINT text of a member
    /// and a flag that is true for a number, as `format_expression` gives
    pub(super) fn format_member(&mut self, path: &MemberPath) -> Result<(String, bool)> {
        Ok(match self.member_value(path)? {
            Variable::Integer(value) => (value.to_string(), true),
            Variable::Real(value) => (format_real(value, self.exact_reals), true),
            Variable::String(value) => (value, false),
            _ => return Err(BBCBasicError::TypeMismatch),
        })
    }

    /// Check that a member target of INPUT, READ or INPUT# exists, before
    /// the statement takes a reply, a DATA value or a record. Another target
    /// needs no check here.
    pub(super) fn check_member_target(&mut self, target: &InputItem) -> Result<()> {
        if let InputItem::Member(path) = target {
            self.member_value(path)?;
        }
        Ok(())
    }

    /// The LVAR lines of the structures, in name order and in member order.
    /// A scalar member is one line, such as `point.x = 1.5`. An array member
    /// shows its top indices, such as `s.a(3)`. A sub-structure is a line
    /// such as `s.p{}`, and its members follow it with 2 more spaces of
    /// indent (issue #192).
    pub(super) fn structure_listing(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, members) in self.variables.structures() {
            list_members(name, members, "", &mut lines);
        }
        lines
    }
}

/// Add the LVAR lines of these members to `lines`. `prefix` is the path of
/// the structure, such as `s.p`, and `indent` is the space at the left.
fn list_members(prefix: &str, members: &Members, indent: &str, lines: &mut Vec<String>) {
    for (member, value) in members {
        let path = format!("{prefix}.{member}");
        let line = match value {
            MemberValue::Structure(inner) => {
                lines.push(format!("{indent}{path}{{}}"));
                list_members(&path, inner, &format!("{indent}  "), lines);
                continue;
            }
            MemberValue::Variable(Variable::Integer(value)) => format!("{path} = {value}"),
            MemberValue::Variable(Variable::Real(value)) => {
                format!("{path} = {}", format_number(*value))
            }
            MemberValue::Variable(Variable::String(value)) => format!("{path} = \"{value}\""),
            MemberValue::Variable(array) => {
                let top_indices: Vec<String> = array
                    .dimensions()
                    .unwrap_or_default()
                    .iter()
                    .map(|size| (size - 1).to_string())
                    .collect();
                format!("{path}({})", top_indices.join(","))
            }
        };
        lines.push(format!("{indent}{line}"));
    }
}
