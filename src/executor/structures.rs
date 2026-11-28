//! The run of structures (issues #191, #192 and #193). The form is the form
//! of BBC BASIC for Windows: see `src/parser/structures.rs` for the manual
//! page.
//!
//! The suffix of a member name gives its type, as for a variable: `%` is an
//! integer, `$` is a string, and no suffix is a real.

use super::{format_number, format_real, real_to_integer, Executor};
use crate::error::{BBCBasicError, Result};
use crate::parser::{Expression, InputItem, MemberDecl, MemberPath};
use crate::variables::{
    check_dim_space, element_bytes, zero_members, MemberValue, Members, StructureId, Variable,
    VariableStore,
};
use std::collections::HashMap;

/// The index values of a member path: the element of an array of
/// structures (issue #193) and the indices of an array member. Each list is
/// empty when the path has no indices there.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct MemberIndices {
    pub(super) element: Vec<usize>,
    pub(super) member: Vec<usize>,
}

impl MemberIndices {
    /// The value of the member of `path` at these indices
    pub(super) fn get(&self, variables: &VariableStore, path: &MemberPath) -> Result<Variable> {
        variables.get_member(&path.structure, &self.element, &path.members, &self.member)
    }

    /// Store a value in the member of `path` at these indices
    pub(super) fn set(
        &self,
        variables: &mut VariableStore,
        path: &MemberPath,
        value: Variable,
    ) -> Result<()> {
        variables.set_member(
            &path.structure,
            &self.element,
            &path.members,
            &self.member,
            value,
        )
    }
}

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
        self.dim_structure_members(name, members)
    }

    /// Declare the structure `name` with these members, for
    /// `execute_dim_structure` and for a prototype (issue #193)
    fn dim_structure_members(&mut self, name: &str, members: Members) -> Result<()> {
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
    /// structure. An element of an array of structures, such as `t{(3)}`, is
    /// an argument by reference too (issue #193).
    pub(super) fn structure_argument(&mut self, argument: &Expression) -> Result<StructureId> {
        match argument {
            Expression::Structure(name) => self.variables.structure_id(name),
            Expression::Member(path) if path.members.is_empty() => self.structure_of_path(path),
            _ => Err(BBCBasicError::BadStructure),
        }
    }

    /// The structure of a path with no members: a structure, such as `a{}`,
    /// or an element, such as `t{(1)}` (issue #193). A negative index gives
    /// Subscript out of range.
    fn structure_of_path(&mut self, path: &MemberPath) -> Result<StructureId> {
        let element = self.index_values(&path.element)?;
        self.variables
            .structure_or_element(&path.structure, &element)
    }

    /// Execute `DIM name{(dimensions) members}` (issue #193). The manual
    /// page is the page of `structure_argument`, section "Arrays of
    /// structures".
    pub(super) fn execute_dim_structure_array(
        &mut self,
        name: &str,
        dimensions: &[Expression],
        members: &[MemberDecl],
    ) -> Result<()> {
        let sizes = self.eval_dimensions(dimensions)?;
        let members = self.make_members(members)?;
        self.dim_structure_array(name, sizes, members)
    }

    /// Declare an array of structures. The manual, section "LOCAL and
    /// PRIVATE structures", permits a LOCAL array of structures, but this
    /// interpreter has no LOCAL form for one: `LOCAL t{}` declares a
    /// structure. So an array DIM of a LOCAL structure gives Bad DIM, and
    /// makes no global array.
    fn dim_structure_array(
        &mut self,
        name: &str,
        sizes: Vec<usize>,
        members: Members,
    ) -> Result<()> {
        if self.variables.is_local_structure_without_dim(name) {
            return Err(BBCBasicError::BadDim);
        }
        self.variables.dim_structure_array(name, sizes, members)
    }

    /// Execute `DIM name{} = a{}` or `DIM name{(dimensions)} = a{}` (issue
    /// #193). The manual page is the page of `structure_argument`, section
    /// "Structure prototypes".
    pub(super) fn execute_dim_prototype(
        &mut self,
        name: &str,
        dimensions: &[Expression],
        prototype: &MemberPath,
    ) -> Result<()> {
        if dimensions.is_empty() {
            let members = self.prototype_members(prototype)?;
            return self.dim_structure_members(name, members);
        }
        let sizes = self.eval_dimensions(dimensions)?;
        let members = self.prototype_members(prototype)?;
        self.dim_structure_array(name, sizes, members)
    }

    /// The members of a prototype, with each value at 0 or an empty string.
    /// The manual says that a LOCAL structure cannot be a prototype: BBC
    /// BASIC for SDL gives Bad DIM (bbexec.c, function `structure`).
    fn prototype_members(&mut self, prototype: &MemberPath) -> Result<Members> {
        let id = self.structure_of_path(prototype)?;
        let local = self
            .local_stack
            .iter()
            .any(|frame| frame.local_structures.contains(&id));
        if local {
            return Err(BBCBasicError::BadDim);
        }
        Ok(zero_members(self.variables.members_of(id)?))
    }

    /// Execute `target{} = source{}` (issue #193). The manual page is the
    /// page of `structure_argument`, section "Copying a structure". The copy
    /// is deep, also for string members. BBC BASIC for SDL gives Type
    /// mismatch for 2 structures of a different size (bbexec.c, LET): this
    /// interpreter gives it for 2 structures of a different format.
    pub(super) fn execute_structure_copy(
        &mut self,
        target: &MemberPath,
        source: &MemberPath,
    ) -> Result<()> {
        let target = self.structure_of_path(target)?;
        let source = self.structure_of_path(source)?;
        self.variables.copy_structure(target, source)
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
                    check_dim_space(&sizes, element_bytes(name))?;
                    MemberValue::array(name, sizes)
                }
                MemberDecl::Structure { members, .. } => {
                    MemberValue::Structure(self.make_members(members)?)
                }
                MemberDecl::Prototype { prototype, .. } => {
                    MemberValue::Structure(self.prototype_members(prototype)?)
                }
            };
            values.push((member.name().to_string(), value));
        }
        Ok(values)
    }

    /// The values of a list of indices. A negative index gives Subscript out
    /// of range, as for an array.
    fn index_values(&mut self, indices: &[Expression]) -> Result<Vec<usize>> {
        let mut values = Vec::with_capacity(indices.len());
        for index in indices {
            let value = self.eval_integer(index)?;
            if value < 0 {
                return Err(BBCBasicError::SubscriptOutOfRange);
            }
            values.push(value as usize);
        }
        Ok(values)
    }

    /// The index values of a member path: the element index first, then the
    /// indices of an array member (issue #193)
    pub(super) fn member_indices(&mut self, path: &MemberPath) -> Result<MemberIndices> {
        Ok(MemberIndices {
            element: self.index_values(&path.element)?,
            member: self.index_values(&path.indices)?,
        })
    }

    /// Store a value in a member, or in an element of an array member. The
    /// indices are the values that `member_indices` gave.
    pub(super) fn store_member(
        &mut self,
        path: &MemberPath,
        indices: &MemberIndices,
        value: Variable,
    ) -> Result<()> {
        indices.set(&mut self.variables, path, value)
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
        indices.get(&self.variables, target)?;
        let member = target.member();
        let value = if member.ends_with('%') {
            Variable::Integer(self.eval_integer(expression)?)
        } else if member.ends_with('$') {
            Variable::String(self.eval_string(expression)?)
        } else {
            Variable::Real(self.eval_real(expression)?)
        };
        indices.set(&mut self.variables, target, value)
    }

    /// The value of a member, or of an element of an array member
    pub(super) fn member_value(&mut self, path: &MemberPath) -> Result<Variable> {
        let indices = self.member_indices(path)?;
        indices.get(&self.variables, path)
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
    /// and a flag that is true for a number, as `format_expression` gives.
    /// A number uses the format that a variable uses: the format of `@%` for
    /// PRINT and the text that reads back for PRINT# (issue 202).
    pub(super) fn format_member(&mut self, path: &MemberPath) -> Result<(String, bool)> {
        Ok(match self.member_value(path)? {
            Variable::Integer(value) if self.exact_reals => (value.to_string(), true),
            Variable::Integer(value) => (self.number_format().format_integer(value), true),
            Variable::Real(value) if self.exact_reals => (format_real(value, true), true),
            Variable::Real(value) => (self.number_format().format_real(value), true),
            Variable::String(value) => (value, false),
            _ => return Err(BBCBasicError::TypeMismatch),
        })
    }

    /// Check that a member target of INPUT, READ or INPUT# exists, before
    /// the statement takes a reply, a DATA value or a record. Give the index
    /// values of the member, for the store: the indices are evaluated one
    /// time (issue 202). Another target needs no check and gives no indices.
    pub(super) fn check_member_target(&mut self, target: &InputItem) -> Result<MemberIndices> {
        let InputItem::Member(path) = target else {
            return Ok(MemberIndices::default());
        };
        let indices = self.member_indices(path)?;
        indices.get(&self.variables, path)?;
        Ok(indices)
    }

    /// The LVAR lines of the structures, in name order and in member order.
    /// A scalar member is one line, such as `point.x = 1.5`. An array member
    /// shows its top indices, such as `s.a(3)`. A sub-structure is a line
    /// such as `s.p{}`, and its members follow it with 2 more spaces of
    /// indent (issue #192). An array of structures is one line after them,
    /// in the form of its DIM, such as `t{(10) x,a(3),p{y}}` (issue #193).
    pub(super) fn structure_listing(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, members) in self.variables.structures() {
            list_members(name, members, "", &mut lines);
        }
        for (name, top_indices, members) in self.variables.structure_arrays() {
            let top_indices: Vec<String> = top_indices.iter().map(usize::to_string).collect();
            let format = member_format(members);
            lines.push(format!("{name}{{({}) {format}}}", top_indices.join(",")));
        }
        lines
    }
}

/// The member names of a structure, as in a DIM: `x,a(3),p{y}` (issue #193)
fn member_format(members: &Members) -> String {
    let names: Vec<String> = members
        .iter()
        .map(|(name, value)| match value {
            MemberValue::Structure(inner) => format!("{name}{{{}}}", member_format(inner)),
            MemberValue::Variable(variable) => match variable.dimensions() {
                Some(dimensions) => {
                    let top: Vec<String> = dimensions
                        .iter()
                        .map(|size| (size - 1).to_string())
                        .collect();
                    format!("{name}({})", top.join(","))
                }
                None => name.clone(),
            },
        })
        .collect();
    names.join(",")
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
