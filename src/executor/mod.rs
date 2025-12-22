//! Execution engine for BBC BASIC statements
//! 
//! Executes parsed BBC BASIC statements with proper control flow handling.

use crate::error::{Result, BBCBasicError};
use crate::parser::{Statement, Expression};
use crate::variables::VariableStore;
use crate::memory::MemoryManager;

/// BBC BASIC statement executor
#[derive(Debug)]
pub struct Executor {
    variables: VariableStore,
    memory: MemoryManager,
    // Control flow stack for GOSUB/RETURN
    return_stack: Vec<u16>,
    // FOR loop state: (variable, end_value, step_value, loop_line)
    for_loops: Vec<(String, i32, i32, u16)>,
}

impl Executor {
    /// Create a new executor
    pub fn new() -> Self {
        Self {
            variables: VariableStore::new(),
            memory: MemoryManager::new(),
            return_stack: Vec::new(),
            for_loops: Vec::new(),
        }
    }

    /// Execute a statement
    pub fn execute_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Assignment { target, expression } => {
                self.execute_assignment(target, expression)
            }
            _ => {
                // Other statements not implemented yet
                Ok(())
            }
        }
    }
    
    /// Execute an assignment statement
    fn execute_assignment(&mut self, target: &str, expression: &Expression) -> Result<()> {
        // Determine variable type from suffix
        if target.ends_with('%') {
            let value = self.eval_integer(expression)?;
            self.variables.set_integer_var(target.to_string(), value);
            Ok(())
        } else if target.ends_with('$') {
            let value = self.eval_string(expression)?;
            self.variables.set_string_var(target.to_string(), value)?;
            Ok(())
        } else {
            let value = self.eval_real(expression)?;
            self.variables.set_real_var(target.to_string(), value);
            Ok(())
        }
    }
    
    /// Evaluate an expression to an integer value
    fn eval_integer(&self, expr: &Expression) -> Result<i32> {
        match expr {
            Expression::Integer(val) => Ok(*val),
            Expression::Real(val) => Ok(*val as i32),
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    self.variables.get_integer_var(name)
                        .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))
                } else {
                    let real_val = self.variables.get_real_var(name)
                        .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))?;
                    Ok(real_val as i32)
                }
            }
            Expression::BinaryOp { op, left, right } => {
                use crate::parser::BinaryOperator;
                let left_val = self.eval_integer(left)?;
                let right_val = self.eval_integer(right)?;
                
                match op {
                    BinaryOperator::Add => Ok(left_val + right_val),
                    BinaryOperator::Subtract => Ok(left_val - right_val),
                    BinaryOperator::Multiply => Ok(left_val * right_val),
                    BinaryOperator::Divide => {
                        if right_val == 0 {
                            Err(BBCBasicError::DivisionByZero)
                        } else {
                            Ok(left_val / right_val)
                        }
                    }
                    BinaryOperator::IntegerDivide => {
                        if right_val == 0 {
                            Err(BBCBasicError::DivisionByZero)
                        } else {
                            Ok(left_val / right_val)
                        }
                    }
                    BinaryOperator::Modulo => Ok(left_val % right_val),
                    BinaryOperator::Power => Ok(left_val.pow(right_val as u32)),
                    _ => Err(BBCBasicError::IllegalFunction),
                }
            }
            Expression::UnaryOp { op, operand } => {
                use crate::parser::UnaryOperator;
                let val = self.eval_integer(operand)?;
                match op {
                    UnaryOperator::Minus => Ok(-val),
                    UnaryOperator::Plus => Ok(val),
                    UnaryOperator::Not => Ok(if val == 0 { -1 } else { 0 }),
                }
            }
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }
    
    /// Evaluate an expression to a real value
    fn eval_real(&self, expr: &Expression) -> Result<f64> {
        match expr {
            Expression::Integer(val) => Ok(*val as f64),
            Expression::Real(val) => Ok(*val),
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    let int_val = self.variables.get_integer_var(name)
                        .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))?;
                    Ok(int_val as f64)
                } else {
                    self.variables.get_real_var(name)
                        .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))
                }
            }
            Expression::BinaryOp { op, left, right } => {
                use crate::parser::BinaryOperator;
                let left_val = self.eval_real(left)?;
                let right_val = self.eval_real(right)?;
                
                match op {
                    BinaryOperator::Add => Ok(left_val + right_val),
                    BinaryOperator::Subtract => Ok(left_val - right_val),
                    BinaryOperator::Multiply => Ok(left_val * right_val),
                    BinaryOperator::Divide => {
                        if right_val == 0.0 {
                            Err(BBCBasicError::DivisionByZero)
                        } else {
                            Ok(left_val / right_val)
                        }
                    }
                    BinaryOperator::Power => Ok(left_val.powf(right_val)),
                    _ => Err(BBCBasicError::IllegalFunction),
                }
            }
            Expression::UnaryOp { op, operand } => {
                use crate::parser::UnaryOperator;
                let val = self.eval_real(operand)?;
                match op {
                    UnaryOperator::Minus => Ok(-val),
                    UnaryOperator::Plus => Ok(val),
                    UnaryOperator::Not => Ok(if val == 0.0 { -1.0 } else { 0.0 }),
                }
            }
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }
    
    /// Evaluate an expression to a string value
    fn eval_string(&self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::String(val) => Ok(val.clone()),
            Expression::Variable(name) => {
                self.variables.get_string_var(name)
                    .map(|s| s.to_string())
                    .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))
            }
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }
    
    /// Get a variable value (for testing)
    #[cfg(test)]
    pub fn get_variable_int(&self, name: &str) -> Result<i32> {
        self.variables.get_integer_var(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(name.to_string()))
    }
    
    #[cfg(test)]
    pub fn get_variable_real(&self, name: &str) -> Result<f64> {
        self.variables.get_real_var(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(name.to_string()))
    }
    
    #[cfg(test)]
    pub fn get_variable_string(&self, name: &str) -> Result<String> {
        self.variables.get_string_var(name)
            .map(|s| s.to_string())
            .ok_or_else(|| BBCBasicError::NoSuchVariable(name.to_string()))
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_executor_creation() {
        // RED: Test creating an executor
        let executor = Executor::new();
        assert!(executor.return_stack.is_empty());
        assert!(executor.for_loops.is_empty());
    }
    
    #[test]
    fn test_execute_integer_assignment() {
        // RED: Test executing "A% = 42"
        let mut executor = Executor::new();
        let stmt = Statement::Assignment {
            target: "A%".to_string(),
            expression: Expression::Integer(42),
        };
        
        executor.execute_statement(&stmt).unwrap();
        assert_eq!(executor.get_variable_int("A%").unwrap(), 42);
    }
    
    #[test]
    fn test_execute_real_assignment() {
        // RED: Test executing "B = 3.14"
        let mut executor = Executor::new();
        let stmt = Statement::Assignment {
            target: "B".to_string(),
            expression: Expression::Real(3.14),
        };
        
        executor.execute_statement(&stmt).unwrap();
        assert_eq!(executor.get_variable_real("B").unwrap(), 3.14);
    }
    
    #[test]
    fn test_execute_string_assignment() {
        // RED: Test executing C$ = "HELLO"
        let mut executor = Executor::new();
        let stmt = Statement::Assignment {
            target: "C$".to_string(),
            expression: Expression::String("HELLO".to_string()),
        };
        
        executor.execute_statement(&stmt).unwrap();
        assert_eq!(executor.get_variable_string("C$").unwrap(), "HELLO");
    }
    
    #[test]
    fn test_evaluate_integer_expression() {
        // RED: Test evaluating "2 + 3 * 4" = 14
        use crate::parser::BinaryOperator;
        
        let executor = Executor::new();
        let expr = Expression::BinaryOp {
            op: BinaryOperator::Add,
            left: Box::new(Expression::Integer(2)),
            right: Box::new(Expression::BinaryOp {
                op: BinaryOperator::Multiply,
                left: Box::new(Expression::Integer(3)),
                right: Box::new(Expression::Integer(4)),
            }),
        };
        
        assert_eq!(executor.eval_integer(&expr).unwrap(), 14);
    }
}