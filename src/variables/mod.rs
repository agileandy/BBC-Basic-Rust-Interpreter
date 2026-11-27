//! Variable storage system for BBC BASIC
//!
//! Manages all BBC BASIC variables including integers, reals, strings, and arrays
//! with proper type handling and memory allocation.

use crate::error::{BBCBasicError, Result};
use std::collections::HashMap;

/// The maximum length of a string, the limit of BBC BASIC for Windows.
/// A longer string gives String too long (issue #102).
pub const MAX_STRING_LENGTH: usize = 65535;

/// Variable types supported by BBC BASIC
#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    Integer,
    Real,
    String,
}

/// Represents a BBC BASIC variable value
#[derive(Debug, Clone, PartialEq)]
pub enum Variable {
    /// 32-bit signed integer (A%, B%, etc.)
    Integer(i32),
    /// IEEE 754 double precision real (A, B, etc.)
    Real(f64),
    /// String up to `MAX_STRING_LENGTH` characters (A$, B$, etc.)
    String(String),
    /// Multi-dimensional integer array
    IntegerArray {
        values: Vec<i32>,
        dimensions: Vec<usize>,
    },
    /// Multi-dimensional real array
    RealArray {
        values: Vec<f64>,
        dimensions: Vec<usize>,
    },
    /// Multi-dimensional string array
    StringArray {
        values: Vec<String>,
        dimensions: Vec<usize>,
    },
}

impl Variable {
    /// Get the type of this variable
    pub fn var_type(&self) -> VarType {
        match self {
            Variable::Integer(_) => VarType::Integer,
            Variable::Real(_) => VarType::Real,
            Variable::String(_) => VarType::String,
            Variable::IntegerArray { .. } => VarType::Integer,
            Variable::RealArray { .. } => VarType::Real,
            Variable::StringArray { .. } => VarType::String,
        }
    }

    /// Check if this variable is an array
    pub fn is_array(&self) -> bool {
        matches!(
            self,
            Variable::IntegerArray { .. }
                | Variable::RealArray { .. }
                | Variable::StringArray { .. }
        )
    }

    /// Get array dimensions if this is an array
    pub fn dimensions(&self) -> Option<&[usize]> {
        match self {
            Variable::IntegerArray { dimensions, .. } => Some(dimensions),
            Variable::RealArray { dimensions, .. } => Some(dimensions),
            Variable::StringArray { dimensions, .. } => Some(dimensions),
            _ => None,
        }
    }

    /// Create a new integer array with given dimensions
    pub fn new_integer_array(dimensions: Vec<usize>) -> Self {
        let total_size = dimensions.iter().product();
        Variable::IntegerArray {
            values: vec![0; total_size],
            dimensions,
        }
    }

    /// Create a new real array with given dimensions
    pub fn new_real_array(dimensions: Vec<usize>) -> Self {
        let total_size = dimensions.iter().product();
        Variable::RealArray {
            values: vec![0.0; total_size],
            dimensions,
        }
    }

    /// Create a new string array with given dimensions
    pub fn new_string_array(dimensions: Vec<usize>) -> Self {
        let total_size = dimensions.iter().product();
        Variable::StringArray {
            values: vec![String::new(); total_size],
            dimensions,
        }
    }

    /// Calculate linear index from multi-dimensional indices
    pub fn calculate_index(&self, indices: &[usize]) -> Result<usize> {
        let dimensions = self.dimensions().ok_or(BBCBasicError::TypeMismatch)?;

        if indices.len() != dimensions.len() {
            return Err(BBCBasicError::SubscriptOutOfRange);
        }

        let mut linear_index = 0;
        let mut multiplier = 1;

        for (i, &index) in indices.iter().enumerate().rev() {
            if index >= dimensions[i] {
                return Err(BBCBasicError::SubscriptOutOfRange);
            }
            linear_index += index * multiplier;
            multiplier *= dimensions[i];
        }

        Ok(linear_index)
    }
}

/// Variable storage system
#[derive(Debug, Clone)]
pub struct VariableStore {
    variables: HashMap<String, Variable>,
    /// Arrays, apart from scalars of the same name. `None` is an array that
    /// LOCAL declared but no DIM has made yet.
    arrays: HashMap<String, Option<Variable>>,
    /// The structure of each name, apart from scalars and arrays of the same
    /// name (issue #191). `None` is a structure that LOCAL declared but no
    /// DIM has made yet. 2 names have the same id when one is a structure
    /// parameter for the other (issue #194).
    structure_names: HashMap<String, Option<StructureId>>,
    /// The members of each structure, in the order of the DIM
    structures: HashMap<StructureId, Members>,
    /// The id of the next structure that a DIM makes
    next_structure_id: StructureId,
    /// The arrays of structures, apart from the structures of the same name
    /// (issue #193)
    structure_arrays: HashMap<String, StructureArray>,
}

/// An array of structures (issue #193). Each element is a structure with
/// its own id, so that an element can be a structure argument.
#[derive(Debug, Clone)]
struct StructureArray {
    /// The element count of each dimension, as for `dim_array`
    dimensions: Vec<usize>,
    /// The structure of each element, in the order of `linear_index`
    elements: Vec<StructureId>,
}

/// The position of an element in a list with these dimensions. A wrong
/// count of indices or an index out of range gives Subscript out of range,
/// as `Variable::calculate_index` does for an array.
fn linear_index(dimensions: &[usize], indices: &[usize]) -> Result<usize> {
    if indices.len() != dimensions.len() {
        return Err(BBCBasicError::SubscriptOutOfRange);
    }
    let mut position = 0;
    for (&index, &size) in indices.iter().zip(dimensions) {
        if index >= size {
            return Err(BBCBasicError::SubscriptOutOfRange);
        }
        position = position * size + index;
    }
    Ok(position)
}

/// The members with the names and the format of these members, with each
/// value at 0 or an empty string: a structure from a prototype (issue #193)
pub fn zero_members(members: &Members) -> Members {
    members
        .iter()
        .map(|(name, value)| {
            let zero = match value {
                MemberValue::Structure(inner) => MemberValue::Structure(zero_members(inner)),
                MemberValue::Variable(variable) => match variable.dimensions() {
                    Some(dimensions) => MemberValue::array(name, dimensions.to_vec()),
                    None => MemberValue::scalar(name),
                },
            };
            (name.clone(), zero)
        })
        .collect()
}

/// True if the 2 member lists have the same format: the same count of
/// members, and in each position the same type, the same array dimensions
/// and the same format of a sub-structure. The names can be different.
fn same_format(first: &Members, second: &Members) -> bool {
    first.len() == second.len()
        && first
            .iter()
            .zip(second)
            .all(|((_, a), (_, b))| match (a, b) {
                (MemberValue::Structure(a), MemberValue::Structure(b)) => same_format(a, b),
                (MemberValue::Variable(a), MemberValue::Variable(b)) => {
                    std::mem::discriminant(a) == std::mem::discriminant(b)
                        && a.dimensions() == b.dimensions()
                }
                _ => false,
            })
}

/// Put the values of `source` in `target`, which has the same format. The
/// names of `target` do not change.
fn copy_values(target: &mut Members, source: Members) {
    for ((_, to), (_, from)) in target.iter_mut().zip(source) {
        match (to, from) {
            (MemberValue::Structure(to), MemberValue::Structure(from)) => copy_values(to, from),
            (to, from) => *to = from,
        }
    }
}

/// The text of a structure or an element in an error, such as `t{(1,2)}`
fn structure_label(structure: &str, element: &[usize]) -> String {
    if element.is_empty() {
        return structure.to_string();
    }
    let indices: Vec<String> = element.iter().map(usize::to_string).collect();
    format!("{structure}{{({})}}", indices.join(","))
}

/// The value of a structure member: a variable, which is a scalar or an
/// array, or a sub-structure (issue #192)
#[derive(Debug, Clone, PartialEq)]
pub enum MemberValue {
    Variable(Variable),
    Structure(Members),
}

/// The members of a structure or a sub-structure, in the order of the DIM
pub type Members = Vec<(String, MemberValue)>;

impl MemberValue {
    /// A member with the value 0 or an empty string, by the suffix of its name
    pub fn scalar(name: &str) -> Self {
        MemberValue::Variable(if name.ends_with('%') {
            Variable::Integer(0)
        } else if name.ends_with('$') {
            Variable::String(String::new())
        } else {
            Variable::Real(0.0)
        })
    }

    /// An array member, by the suffix of its name. Each value in
    /// `dimensions` is the element count of that dimension, as for
    /// `VariableStore::dim_array`.
    pub fn array(name: &str, dimensions: Vec<usize>) -> Self {
        MemberValue::Variable(if name.ends_with('%') {
            Variable::new_integer_array(dimensions)
        } else if name.ends_with('$') {
            Variable::new_string_array(dimensions)
        } else {
            Variable::new_real_array(dimensions)
        })
    }
}

/// The No such variable error of a member path that does not exist, such as
/// `s.p.z`
fn no_such_member(structure: &str, path: &[String]) -> BBCBasicError {
    BBCBasicError::NoSuchVariable(format!("{structure}.{}", path.join(".")))
}

/// The element of an array at these indices. An index out of range gives
/// Subscript out of range.
fn array_element(array: &Variable, indices: &[usize]) -> Result<Variable> {
    let linear_index = array.calculate_index(indices)?;
    match array {
        Variable::IntegerArray { values, .. } => Ok(Variable::Integer(values[linear_index])),
        Variable::RealArray { values, .. } => Ok(Variable::Real(values[linear_index])),
        Variable::StringArray { values, .. } => Ok(Variable::String(values[linear_index].clone())),
        _ => Err(BBCBasicError::TypeMismatch),
    }
}

/// Set the element of an array at these indices. The value must have the
/// type of the array.
fn store_element(array: &mut Variable, indices: &[usize], value: Variable) -> Result<()> {
    let linear_index = array.calculate_index(indices)?;
    match (array, value) {
        (Variable::IntegerArray { values, .. }, Variable::Integer(val)) => {
            values[linear_index] = val
        }
        (Variable::RealArray { values, .. }, Variable::Real(val)) => values[linear_index] = val,
        (Variable::StringArray { values, .. }, Variable::String(val)) => {
            if val.len() > MAX_STRING_LENGTH {
                return Err(BBCBasicError::StringTooLong);
            }
            values[linear_index] = val
        }
        _ => return Err(BBCBasicError::TypeMismatch),
    }
    Ok(())
}

/// The id of the members of a structure. 2 names share an id when one is a
/// structure parameter for the other (issue #194).
pub type StructureId = usize;

/// The name of the print format variable (issue 166)
pub const PRINT_FORMAT_NAME: &str = "@%";

/// The start value of `@%`: general format, 9 digits, a field of 10
pub const PRINT_FORMAT_DEFAULT: i32 = 0x90A;

/// True if the name is one of the resident integer variables @% and A% to Z%
pub fn is_resident_integer(name: &str) -> bool {
    matches!(name.as_bytes(), [b'@'..=b'Z', b'%'])
}

/// The start value of a resident integer variable: &90A for @%, 0 for A% to Z%
pub fn resident_default(name: &str) -> i32 {
    if name == PRINT_FORMAT_NAME {
        PRINT_FORMAT_DEFAULT
    } else {
        0
    }
}

impl VariableStore {
    /// Create a new variable store. The resident integer variables @% and A%
    /// to Z% exist from the start with their start values.
    pub fn new() -> Self {
        let variables = ('@'..='Z')
            .map(|letter| format!("{letter}%"))
            .map(|name| {
                let value = resident_default(&name);
                (name, Variable::Integer(value))
            })
            .collect();
        Self {
            variables,
            arrays: HashMap::new(),
            structure_names: HashMap::new(),
            structures: HashMap::new(),
            next_structure_id: 0,
            structure_arrays: HashMap::new(),
        }
    }

    /// Declare an array of structures (issue #193). Each value in
    /// `dimensions` is the element count of that dimension, and each
    /// element gets a copy of `members`. A second DIM gives Bad DIM.
    pub fn dim_structure_array(
        &mut self,
        name: &str,
        dimensions: Vec<usize>,
        members: Members,
    ) -> Result<()> {
        if self.structure_arrays.contains_key(name) {
            return Err(BBCBasicError::BadDim);
        }
        let count = dimensions
            .iter()
            .try_fold(1usize, |count, &size| count.checked_mul(size))
            .ok_or(BBCBasicError::BadDim)?;
        let first = self.next_structure_id;
        self.next_structure_id += count;
        let elements: Vec<StructureId> = (first..first + count).collect();
        for &id in &elements {
            self.structures.insert(id, members.clone());
        }
        let array = StructureArray {
            dimensions,
            elements,
        };
        self.structure_arrays.insert(name.to_string(), array);
        Ok(())
    }

    /// The id of an element of an array of structures (issue #193). A name
    /// that is not an array of structures gives Bad use of structure. A
    /// wrong count of indices or an index out of range gives Subscript out
    /// of range.
    pub fn element_id(&self, name: &str, element: &[usize]) -> Result<StructureId> {
        let array = self
            .structure_arrays
            .get(name)
            .ok_or(BBCBasicError::BadStructure)?;
        Ok(array.elements[linear_index(&array.dimensions, element)?])
    }

    /// The members of the structure of an id
    pub fn members_of(&self, id: StructureId) -> Result<&Members> {
        self.structures.get(&id).ok_or(BBCBasicError::BadStructure)
    }

    /// Copy each member value of the structure `source` to the structure
    /// `target`, as `target{} = source{}` (issue #193). The copy is deep. 2
    /// structures with a different format give Type mismatch.
    pub fn copy_structure(&mut self, target: StructureId, source: StructureId) -> Result<()> {
        let values = self.members_of(source)?.clone();
        let members = self
            .structures
            .get_mut(&target)
            .ok_or(BBCBasicError::BadStructure)?;
        if !same_format(members, &values) {
            return Err(BBCBasicError::TypeMismatch);
        }
        copy_values(members, values);
        Ok(())
    }

    /// The arrays of structures in name order, each with its top indices and
    /// the members of its first element (issue #193)
    pub fn structure_arrays(&self) -> Vec<(&str, Vec<usize>, &Members)> {
        let mut arrays: Vec<_> = self
            .structure_arrays
            .iter()
            .filter_map(|(name, array)| {
                let members = self.structures.get(array.elements.first()?)?;
                let top_indices = array.dimensions.iter().map(|size| size - 1).collect();
                Some((name.as_str(), top_indices, members))
            })
            .collect();
        arrays.sort_by_key(|(name, ..)| *name);
        arrays
    }

    /// Declare a structure with these members (issues #191 and #192), and
    /// give its id. A second DIM of a structure gives Bad DIM, as for an
    /// array. A structure that LOCAL declared can have one DIM. The parser
    /// checks the member names.
    pub fn dim_structure(&mut self, name: &str, members: Members) -> Result<StructureId> {
        if let Some(Some(_)) = self.structure_names.get(name) {
            return Err(BBCBasicError::BadDim);
        }
        let id = self.next_structure_id;
        self.next_structure_id += 1;
        self.structures.insert(id, members);
        self.structure_names.insert(name.to_string(), Some(id));
        Ok(id)
    }

    /// The id of the structure of a name. A name that is not a structure, or
    /// a LOCAL structure with no DIM, gives Bad use of structure.
    pub fn structure_id(&self, name: &str) -> Result<StructureId> {
        match self.structure_names.get(name) {
            Some(Some(id)) if self.structures.contains_key(id) => Ok(*id),
            _ => Err(BBCBasicError::BadStructure),
        }
    }

    /// Make a name a second name for the structure of an id: a structure
    /// parameter (issue #194)
    pub fn bind_structure(&mut self, name: &str, id: StructureId) {
        self.structure_names.insert(name.to_string(), Some(id));
    }

    /// Copy the structure entry of a name, for LOCAL and for a parameter.
    /// The outer `None` means no structure of this name exists.
    pub fn save_structure(&self, name: &str) -> Option<Option<StructureId>> {
        self.structure_names.get(name).copied()
    }

    /// Declare a LOCAL structure that has no DIM yet
    pub fn declare_local_structure(&mut self, name: &str) {
        self.structure_names.insert(name.to_string(), None);
    }

    /// True if the name is a LOCAL structure that has no DIM yet
    pub fn is_local_structure_without_dim(&self, name: &str) -> bool {
        matches!(self.structure_names.get(name), Some(None))
    }

    /// Put back a structure entry from `save_structure`, or remove the name
    /// if there was none
    pub fn restore_structure(&mut self, name: String, saved: Option<Option<StructureId>>) {
        match saved {
            Some(entry) => {
                self.structure_names.insert(name, entry);
            }
            None => {
                self.structure_names.remove(&name);
            }
        }
    }

    /// Remove the members of a LOCAL structure, at the end of its routine
    pub fn remove_structure(&mut self, id: StructureId) {
        self.structures.remove(&id);
    }

    /// The id of the structure of a name, or of an element of the array of
    /// structures of the name if `element` is not empty (issue #193)
    pub fn structure_or_element(&self, name: &str, element: &[usize]) -> Result<StructureId> {
        if element.is_empty() {
            self.structure_id(name)
        } else {
            self.element_id(name, element)
        }
    }

    /// The members of the structure of a name or of an element
    fn structure_members(&self, name: &str, element: &[usize]) -> Result<&Members> {
        let id = self.structure_or_element(name, element)?;
        self.structures.get(&id).ok_or(BBCBasicError::BadStructure)
    }

    /// The members of the structure of a name or of an element (mutable)
    fn structure_members_mut(&mut self, name: &str, element: &[usize]) -> Result<&mut Members> {
        let id = self.structure_or_element(name, element)?;
        self.structures
            .get_mut(&id)
            .ok_or(BBCBasicError::BadStructure)
    }

    /// The member at the end of a path, such as `["pos", "x"]` for
    /// `shape.pos.x` (issue #192). A name that is not a structure gives Bad
    /// use of structure, and so does a name after a member that is not a
    /// sub-structure. A member that does not exist gives No such variable.
    /// The structure is an element if `element` is not empty (issue #193).
    fn find_member(
        &self,
        structure: &str,
        element: &[usize],
        path: &[String],
    ) -> Result<&MemberValue> {
        let label = structure_label(structure, element);
        let mut members = self.structure_members(structure, element)?;
        for (depth, name) in path.iter().enumerate() {
            let (_, value) = members
                .iter()
                .find(|(member, _)| member == name)
                .ok_or_else(|| no_such_member(&label, &path[..=depth]))?;
            match value {
                _ if depth + 1 == path.len() => return Ok(value),
                MemberValue::Structure(inner) => members = inner,
                MemberValue::Variable(_) => return Err(BBCBasicError::BadStructure),
            }
        }
        Err(BBCBasicError::BadStructure)
    }

    /// The member at the end of a path (mutable), with the errors of
    /// `find_member`
    fn find_member_mut(
        &mut self,
        structure: &str,
        element: &[usize],
        path: &[String],
    ) -> Result<&mut Variable> {
        let label = structure_label(structure, element);
        let mut members = self.structure_members_mut(structure, element)?;
        for (depth, name) in path.iter().enumerate() {
            let (_, value) = members
                .iter_mut()
                .find(|(member, _)| member == name)
                .ok_or_else(|| no_such_member(&label, &path[..=depth]))?;
            match value {
                MemberValue::Variable(variable) if depth + 1 == path.len() => return Ok(variable),
                MemberValue::Structure(inner) if depth + 1 < path.len() => members = inner,
                _ => return Err(BBCBasicError::BadStructure),
            }
        }
        Err(BBCBasicError::BadStructure)
    }

    /// The value of a structure member, or of an element of an array member
    /// if `indices` is not empty (issue #192). The errors are the errors of
    /// `find_member`, and Bad use of structure for an index on a member that
    /// is not an array, for an array member with no index, and for a
    /// sub-structure. An index out of range gives Subscript out of range.
    /// The structure is an element if `element` is not empty (issue #193).
    pub fn get_member(
        &self,
        structure: &str,
        element: &[usize],
        path: &[String],
        indices: &[usize],
    ) -> Result<Variable> {
        let MemberValue::Variable(variable) = self.find_member(structure, element, path)? else {
            return Err(BBCBasicError::BadStructure);
        };
        match (variable.is_array(), indices.is_empty()) {
            (false, true) => Ok(variable.clone()),
            (true, false) => array_element(variable, indices),
            _ => Err(BBCBasicError::BadStructure),
        }
    }

    /// Set a structure member, or an element of an array member if `indices`
    /// is not empty. The value must have the type of the member. The errors
    /// are the errors of `get_member`, and Type mismatch.
    pub fn set_member(
        &mut self,
        structure: &str,
        element: &[usize],
        path: &[String],
        indices: &[usize],
        value: Variable,
    ) -> Result<()> {
        let slot = self.find_member_mut(structure, element, path)?;
        match (slot.is_array(), indices.is_empty()) {
            (false, true) => {}
            (true, false) => return store_element(slot, indices, value),
            _ => return Err(BBCBasicError::BadStructure),
        }
        match (&*slot, &value) {
            (Variable::Integer(_), Variable::Integer(_))
            | (Variable::Real(_), Variable::Real(_)) => {}
            (Variable::String(_), Variable::String(text)) if text.len() > MAX_STRING_LENGTH => {
                return Err(BBCBasicError::StringTooLong)
            }
            (Variable::String(_), Variable::String(_)) => {}
            _ => return Err(BBCBasicError::TypeMismatch),
        }
        *slot = value;
        Ok(())
    }

    /// The structures and their members, in name order. A structure
    /// parameter is in the list with its own name too.
    pub fn structures(&self) -> Vec<(&str, &Members)> {
        let mut structures: Vec<_> = self
            .structure_names
            .iter()
            .filter_map(|(name, id)| {
                let members = self.structures.get(id.as_ref()?)?;
                Some((name.as_str(), members))
            })
            .collect();
        structures.sort_by_key(|(name, _)| *name);
        structures
    }

    /// Set an integer variable
    pub fn set_integer_var(&mut self, name: String, value: i32) {
        self.variables.insert(name, Variable::Integer(value));
    }

    /// Get an integer variable
    pub fn get_integer_var(&self, name: &str) -> Option<i32> {
        match self.variables.get(name) {
            Some(Variable::Integer(value)) => Some(*value),
            _ => None,
        }
    }

    /// Set a real variable
    pub fn set_real_var(&mut self, name: String, value: f64) {
        self.variables.insert(name, Variable::Real(value));
    }

    /// Get a real variable
    pub fn get_real_var(&self, name: &str) -> Option<f64> {
        match self.variables.get(name) {
            Some(Variable::Real(value)) => Some(*value),
            _ => None,
        }
    }

    /// Set a string variable
    pub fn set_string_var(&mut self, name: String, value: String) -> Result<()> {
        if value.len() > MAX_STRING_LENGTH {
            return Err(BBCBasicError::StringTooLong);
        }
        self.variables.insert(name, Variable::String(value));
        Ok(())
    }

    /// Get a string variable
    pub fn get_string_var(&self, name: &str) -> Option<&str> {
        match self.variables.get(name) {
            Some(Variable::String(value)) => Some(value),
            _ => None,
        }
    }

    /// Dimension an array.
    ///
    /// Each value in `dimensions` is the element count of that dimension,
    /// not the top index. `DIM A(N)` must pass N + 1.
    ///
    /// An array that has dimensions gives Bad DIM. An array that LOCAL
    /// declared and that has no dimensions yet can be dimensioned.
    pub fn dim_array(
        &mut self,
        name: String,
        dimensions: Vec<usize>,
        var_type: VarType,
    ) -> Result<()> {
        if let Some(Some(_)) = self.arrays.get(&name) {
            return Err(BBCBasicError::BadDim);
        }
        if dimensions.is_empty() {
            return Err(BBCBasicError::SyntaxError {
                message: "Array must have at least one dimension".to_string(),
                line: None,
            });
        }

        let variable = match var_type {
            VarType::Integer => Variable::new_integer_array(dimensions),
            VarType::Real => Variable::new_real_array(dimensions),
            VarType::String => Variable::new_string_array(dimensions),
        };

        self.arrays.insert(name, Some(variable));
        Ok(())
    }

    /// Get a dimensioned array by name. An array with no DIM gives the Array error.
    pub fn get_array(&self, name: &str) -> Result<&Variable> {
        match self.arrays.get(name) {
            Some(Some(array)) => Ok(array),
            Some(None) | None => Err(BBCBasicError::Array),
        }
    }

    /// Get a dimensioned array by name (mutable)
    fn get_array_mut(&mut self, name: &str) -> Result<&mut Variable> {
        match self.arrays.get_mut(name) {
            Some(Some(array)) => Ok(array),
            Some(None) | None => Err(BBCBasicError::Array),
        }
    }

    /// Replace all the elements of a dimensioned array (issue #187). An array
    /// with no DIM gives the Array error.
    pub fn replace_array(&mut self, name: &str, array: Variable) -> Result<()> {
        *self.get_array_mut(name)? = array;
        Ok(())
    }

    /// Get a variable by name (immutable)
    pub fn get_variable(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }

    /// Get a mutable reference to a variable
    pub fn get_variable_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.variables.get_mut(name)
    }

    /// Get an array element (immutable)
    pub fn get_array_element(&self, name: &str, indices: &[usize]) -> Result<Variable> {
        array_element(self.get_array(name)?, indices)
    }

    /// Set an array element (mutable)
    pub fn set_array_element(
        &mut self,
        name: &str,
        indices: &[usize],
        value: Variable,
    ) -> Result<()> {
        store_element(self.get_array_mut(name)?, indices, value)
    }

    /// Check if a variable exists
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Check if an array of this name exists, with or without dimensions
    pub fn has_array(&self, name: &str) -> bool {
        self.arrays.contains_key(name)
    }

    /// Put back a saved scalar variable as it was
    pub fn restore_variable(&mut self, name: String, variable: Variable) {
        self.variables.insert(name, variable);
    }

    /// Copy the array entry of a name, for LOCAL. The outer `None` means no
    /// array of this name exists. The inner `None` means it is not dimensioned.
    pub fn save_array(&self, name: &str) -> Option<Option<Variable>> {
        self.arrays.get(name).cloned()
    }

    /// Declare a LOCAL array that is not dimensioned yet
    pub fn declare_local_array(&mut self, name: String) {
        self.arrays.insert(name, None);
    }

    /// Put back an array entry from `save_array`, or remove the array if there was none
    pub fn restore_array(&mut self, name: String, saved: Option<Option<Variable>>) {
        match saved {
            Some(entry) => {
                self.arrays.insert(name, entry);
            }
            None => {
                self.arrays.remove(&name);
            }
        }
    }

    /// The scalar variables, in name order
    pub fn scalars(&self) -> Vec<(&str, &Variable)> {
        let mut scalars: Vec<_> = self
            .variables
            .iter()
            .map(|(name, variable)| (name.as_str(), variable))
            .collect();
        scalars.sort_by_key(|(name, _)| *name);
        scalars
    }

    /// The dimensioned arrays, in name order. A LOCAL array with no DIM is not included.
    pub fn arrays(&self) -> Vec<(&str, &Variable)> {
        let mut arrays: Vec<_> = self
            .arrays
            .iter()
            .filter_map(|(name, array)| array.as_ref().map(|array| (name.as_str(), array)))
            .collect();
        arrays.sort_by_key(|(name, _)| *name);
        arrays
    }

    /// Clear all variables, arrays and structures. The resident integer
    /// variables @% and A% to Z% keep their values.
    pub fn clear(&mut self) {
        self.variables.retain(|name, _| is_resident_integer(name));
        self.arrays.clear();
        // The id count goes on, so that an id that a LOCAL frame saved
        // cannot name a new structure
        self.structure_names.clear();
        self.structures.clear();
        self.structure_arrays.clear();
    }
}

impl Default for VariableStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::TestResult;

    #[test]
    fn test_variable_types() {
        let int_var = Variable::Integer(42);
        let real_var = Variable::Real(3.14);
        let string_var = Variable::String("hello".to_string());

        assert_eq!(int_var.var_type(), VarType::Integer);
        assert_eq!(real_var.var_type(), VarType::Real);
        assert_eq!(string_var.var_type(), VarType::String);
    }

    #[test]
    fn test_array_creation() {
        let int_array = Variable::new_integer_array(vec![10, 20]);
        assert!(int_array.is_array());
        assert_eq!(int_array.dimensions(), Some([10, 20].as_slice()));
    }

    #[test]
    fn test_array_access() {
        let mut store = VariableStore::new();

        // Test integer array
        store
            .dim_array("A%(".to_string(), vec![3, 3], VarType::Integer)
            .unwrap();
        store
            .set_array_element("A%(", &[1, 1], Variable::Integer(42))
            .unwrap();
        let result = store.get_array_element("A%(", &[1, 1]).unwrap();
        assert_eq!(result, Variable::Integer(42));

        // Test real array
        store
            .dim_array("B(".to_string(), vec![2, 2], VarType::Real)
            .unwrap();
        store
            .set_array_element("B(", &[0, 1], Variable::Real(3.14))
            .unwrap();
        let result = store.get_array_element("B(", &[0, 1]).unwrap();
        assert_eq!(result, Variable::Real(3.14));

        // Test string array
        store
            .dim_array("C$(".to_string(), vec![2, 2], VarType::String)
            .unwrap();
        store
            .set_array_element("C$(", &[1, 0], Variable::String("hello".to_string()))
            .unwrap();
        let result = store.get_array_element("C$(", &[1, 0]).unwrap();
        assert_eq!(result, Variable::String("hello".to_string()));
    }

    #[test]
    fn test_variable_store() {
        let mut store = VariableStore::new();

        store.set_integer_var("A%".to_string(), 42);
        assert_eq!(store.get_integer_var("A%"), Some(42));

        store.set_real_var("B".to_string(), 3.14);
        assert_eq!(store.get_real_var("B"), Some(3.14));

        store
            .set_string_var("C$".to_string(), "hello".to_string())
            .unwrap();
        assert_eq!(store.get_string_var("C$"), Some("hello"));
    }

    #[test]
    fn test_string_too_long() {
        let mut store = VariableStore::new();
        let long_string = "a".repeat(MAX_STRING_LENGTH + 1);

        let result = store.set_string_var("A$".to_string(), long_string);
        assert!(matches!(result, Err(BBCBasicError::StringTooLong)));
    }

    #[test]
    fn test_structure_members() {
        let mut store = VariableStore::new();
        let members: Members = ["x", "n%", "s$"]
            .map(|name| (name.to_string(), MemberValue::scalar(name)))
            .to_vec();
        let path = |name: &str| [name.to_string()];
        store.dim_structure("p", members.clone()).unwrap();
        assert_eq!(
            store.get_member("p", &[], &path("n%"), &[]).unwrap(),
            Variable::Integer(0)
        );
        store
            .set_member("p", &[], &path("x"), &[], Variable::Real(1.5))
            .unwrap();
        assert_eq!(
            store.get_member("p", &[], &path("x"), &[]).unwrap(),
            Variable::Real(1.5)
        );
        assert!(matches!(
            store.set_member("p", &[], &path("s$"), &[], Variable::Integer(1)),
            Err(BBCBasicError::TypeMismatch)
        ));
        assert!(matches!(
            store.get_member("p", &[], &path("z"), &[]),
            Err(BBCBasicError::NoSuchVariable(name)) if name == "p.z"
        ));
        assert!(matches!(
            store.get_member("q", &[], &path("x"), &[]),
            Err(BBCBasicError::BadStructure)
        ));
        assert!(matches!(
            store.dim_structure("p", members),
            Err(BBCBasicError::BadDim)
        ));
        store.clear();
        assert!(store.structures().is_empty());
    }

    #[test]
    fn test_structure_alias_and_local_entry() {
        let mut store = VariableStore::new();
        let scalar = |name: &str| vec![(name.to_string(), MemberValue::scalar(name))];
        let x = ["x".to_string()];
        let id = store.dim_structure("point", scalar("x")).unwrap();

        // A parameter p{} is a second name for point
        let saved = store.save_structure("p");
        store.bind_structure("p", id);
        store
            .set_member("p", &[], &x, &[], Variable::Real(2.0))
            .unwrap();
        assert_eq!(
            store.get_member("point", &[], &x, &[]).unwrap(),
            Variable::Real(2.0)
        );
        store.restore_structure("p".to_string(), saved);
        assert!(matches!(
            store.get_member("p", &[], &x, &[]),
            Err(BBCBasicError::BadStructure)
        ));

        // LOCAL point{} hides point until the restore, and a DIM makes a new one
        let saved = store.save_structure("point");
        store.declare_local_structure("point");
        assert!(store.is_local_structure_without_dim("point"));
        assert!(matches!(
            store.structure_id("point"),
            Err(BBCBasicError::BadStructure)
        ));
        let local = store.dim_structure("point", scalar("y%")).unwrap();
        assert_ne!(local, id);
        store.restore_structure("point".to_string(), saved);
        store.remove_structure(local);
        assert_eq!(store.structure_id("point").unwrap(), id);
        assert_eq!(store.structures().len(), 1);
    }

    #[test]
    fn test_array_members_and_sub_structures() {
        // DIM s{a%(2), p{t$(1, 1)}} (issue #192)
        let mut store = VariableStore::new();
        let inner = vec![("t$".to_string(), MemberValue::array("t$", vec![2, 2]))];
        let members = vec![
            ("a%".to_string(), MemberValue::array("a%", vec![3])),
            ("p".to_string(), MemberValue::Structure(inner)),
        ];
        store.dim_structure("s", members).unwrap();
        let path =
            |names: &[&str]| -> Vec<String> { names.iter().map(|name| name.to_string()).collect() };
        let text = Variable::String("x".to_string());
        store
            .set_member("s", &[], &path(&["p", "t$"]), &[1, 0], text.clone())
            .unwrap();
        assert_eq!(
            store
                .get_member("s", &[], &path(&["p", "t$"]), &[1, 0])
                .unwrap(),
            text
        );
        store
            .set_member("s", &[], &path(&["a%"]), &[2], Variable::Integer(7))
            .unwrap();
        assert_eq!(
            store.get_member("s", &[], &path(&["a%"]), &[2]).unwrap(),
            Variable::Integer(7)
        );
        let errors = [
            (path(&["a%"]), vec![3], "SubscriptOutOfRange"),
            (path(&["a%"]), vec![], "BadStructure"),
            (path(&["p"]), vec![], "BadStructure"),
            (path(&["p"]), vec![0], "BadStructure"),
            (path(&["a%", "b"]), vec![], "BadStructure"),
            (path(&["p", "z"]), vec![], "NoSuchVariable(\"s.p.z\")"),
        ];
        for (members, indices, error) in errors {
            let found = store.get_member("s", &[], &members, &indices).unwrap_err();
            assert_eq!(format!("{found:?}"), error, "{members:?} {indices:?}");
            let found = store
                .set_member("s", &[], &members, &indices, Variable::Integer(1))
                .unwrap_err();
            assert_eq!(format!("{found:?}"), error, "{members:?} {indices:?}");
        }
    }

    // Property-Based Tests

    /// **Feature: bbc-basic-interpreter, Property 1: Variable Storage and Type Safety**
    /// **Validates: Requirements 2.1, 2.2, 2.3**
    #[test]
    fn prop_integer_variable_storage_roundtrip() {
        fn property(value: i32) -> bool {
            let mut store = VariableStore::new();
            let var_name = "TEST%".to_string();

            store.set_integer_var(var_name.clone(), value);
            store.get_integer_var(&var_name) == Some(value)
        }

        // Run with fewer iterations for faster testing
        let mut qc = quickcheck::QuickCheck::new().tests(10);
        qc.quickcheck(property as fn(i32) -> bool);
    }

    /// **Feature: bbc-basic-interpreter, Property 1: Variable Storage and Type Safety**
    /// **Validates: Requirements 2.1, 2.2, 2.3**
    #[test]
    fn prop_real_variable_storage_roundtrip() {
        fn property(value: f64) -> TestResult {
            // Skip NaN and infinite values as they don't have well-defined equality
            if !value.is_finite() {
                return TestResult::discard();
            }

            let mut store = VariableStore::new();
            let var_name = "TEST".to_string();

            store.set_real_var(var_name.clone(), value);
            let retrieved = store.get_real_var(&var_name);

            TestResult::from_bool(retrieved == Some(value))
        }

        // Run with fewer iterations for faster testing
        let mut qc = quickcheck::QuickCheck::new().tests(10);
        qc.quickcheck(property as fn(f64) -> TestResult);
    }

    /// **Feature: bbc-basic-interpreter, Property 1: Variable Storage and Type Safety**
    /// **Validates: Requirements 2.1, 2.2, 2.3**
    #[test]
    fn prop_string_variable_storage_roundtrip() {
        fn property(value: String) -> TestResult {
            // Only test strings that are within the string limit
            if value.len() > MAX_STRING_LENGTH {
                return TestResult::discard();
            }

            let mut store = VariableStore::new();
            let var_name = "TEST$".to_string();

            match store.set_string_var(var_name.clone(), value.clone()) {
                Ok(()) => {
                    let retrieved = store.get_string_var(&var_name);
                    TestResult::from_bool(retrieved == Some(value.as_str()))
                }
                Err(_) => TestResult::failed(), // Should not fail for valid strings
            }
        }

        // Run with fewer iterations for faster testing
        let mut qc = quickcheck::QuickCheck::new().tests(10);
        qc.quickcheck(property as fn(String) -> TestResult);
    }

    /// **Feature: bbc-basic-interpreter, Property 1: Variable Storage and Type Safety**
    /// **Validates: Requirements 2.1, 2.2, 2.3**
    #[test]
    fn prop_variable_type_consistency() {
        fn property(value: i32) -> bool {
            let int_var = Variable::Integer(value);
            let real_var = Variable::Real(value as f64);
            let string_var = Variable::String(value.to_string());

            int_var.var_type() == VarType::Integer
                && real_var.var_type() == VarType::Real
                && string_var.var_type() == VarType::String
        }

        // Run with fewer iterations for faster testing
        let mut qc = quickcheck::QuickCheck::new().tests(10);
        qc.quickcheck(property as fn(i32) -> bool);
    }
}
