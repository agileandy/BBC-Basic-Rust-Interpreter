//! Variable storage system for BBC BASIC
//!
//! Manages all BBC BASIC variables including integers, reals, strings, and arrays
//! with proper type handling and memory allocation.

use crate::error::{BBCBasicError, Result};
use std::collections::HashMap;

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
    /// String up to 255 characters (A$, B$, etc.)
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
            Variable::IntegerArray { .. } | Variable::RealArray { .. } | Variable::StringArray { .. }
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
}

impl VariableStore {
    /// Create a new variable store
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
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
        if value.len() > 255 {
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

    /// Dimension an array
    pub fn dim_array(&mut self, name: String, dimensions: Vec<usize>, var_type: VarType) -> Result<()> {
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

        self.variables.insert(name, variable);
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
        let variable = self.get_variable(name)
            .ok_or(BBCBasicError::NoSuchVariable(name.to_string()))?;
        
        let linear_index = variable.calculate_index(indices)?;
        
        match variable {
            Variable::IntegerArray { values, .. } => Ok(Variable::Integer(values[linear_index])),
            Variable::RealArray { values, .. } => Ok(Variable::Real(values[linear_index])),
            Variable::StringArray { values, .. } => Ok(Variable::String(values[linear_index].clone())),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Set an array element (mutable)
    pub fn set_array_element(&mut self, name: &str, indices: &[usize], value: Variable) -> Result<()> {
        let variable = self.get_variable_mut(name)
            .ok_or(BBCBasicError::NoSuchVariable(name.to_string()))?;
        
        let linear_index = variable.calculate_index(indices)?;
        
        match (variable, value) {
            (Variable::IntegerArray { values, .. }, Variable::Integer(val)) => values[linear_index] = val,
            (Variable::RealArray { values, .. }, Variable::Real(val)) => values[linear_index] = val,
            (Variable::StringArray { values, .. }, Variable::String(val)) => values[linear_index] = val,
            _ => return Err(BBCBasicError::TypeMismatch),
        }
        
        Ok(())
    }

    /// Check if a variable exists
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Clear all variables
    pub fn clear(&mut self) {
        self.variables.clear();
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
        store.dim_array("A%(".to_string(), vec![3, 3], VarType::Integer).unwrap();
        store.set_array_element("A%(", &[1, 1], Variable::Integer(42)).unwrap();
        let result = store.get_array_element("A%(", &[1, 1]).unwrap();
        assert_eq!(result, Variable::Integer(42));
        
        // Test real array
        store.dim_array("B(".to_string(), vec![2, 2], VarType::Real).unwrap();
        store.set_array_element("B(", &[0, 1], Variable::Real(3.14)).unwrap();
        let result = store.get_array_element("B(", &[0, 1]).unwrap();
        assert_eq!(result, Variable::Real(3.14));
        
        // Test string array
        store.dim_array("C$(".to_string(), vec![2, 2], VarType::String).unwrap();
        store.set_array_element("C$(", &[1, 0], Variable::String("hello".to_string())).unwrap();
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
        
        store.set_string_var("C$".to_string(), "hello".to_string()).unwrap();
        assert_eq!(store.get_string_var("C$"), Some("hello"));
    }

    #[test]
    fn test_string_too_long() {
        let mut store = VariableStore::new();
        let long_string = "a".repeat(256);
        
        let result = store.set_string_var("A$".to_string(), long_string);
        assert!(matches!(result, Err(BBCBasicError::StringTooLong)));
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
            // Only test strings that are within BBC BASIC limits (255 characters)
            if value.len() > 255 {
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
            
            int_var.var_type() == VarType::Integer &&
            real_var.var_type() == VarType::Real &&
            string_var.var_type() == VarType::String
        }
        
        // Run with fewer iterations for faster testing
        let mut qc = quickcheck::QuickCheck::new().tests(10);
        qc.quickcheck(property as fn(i32) -> bool);
    }
}