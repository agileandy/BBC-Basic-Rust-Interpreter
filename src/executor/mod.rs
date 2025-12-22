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
    // Output buffer (for testing)
    #[cfg(test)]
    output: String,
}

impl Executor {
    /// Create a new executor
    pub fn new() -> Self {
        Self {
            variables: VariableStore::new(),
            memory: MemoryManager::new(),
            return_stack: Vec::new(),
            for_loops: Vec::new(),
            #[cfg(test)]
            output: String::new(),
        }
    }

    /// Execute a statement
    pub fn execute_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Assignment { target, expression } => {
                self.execute_assignment(target, expression)
            }
            Statement::Print { items } => {
                self.execute_print(items)
            }
            Statement::End | Statement::Stop => {
                // END and STOP both stop execution
                // In a full program, this would signal the interpreter to halt
                Ok(())
            }
            Statement::Rem { .. } => {
                // Comments do nothing during execution
                Ok(())
            }
            Statement::Goto { line_number } => {
                self.execute_goto(*line_number)
            }
            Statement::Gosub { line_number } => {
                self.execute_gosub(*line_number)
            }
            Statement::Return => {
                self.execute_return()
            }
            Statement::For { variable, start, end, step } => {
                self.execute_for(variable, start, end, step.as_ref())
            }
            Statement::Next { variables } => {
                self.execute_next(variables)
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
    
    /// Execute a PRINT statement
    fn execute_print(&mut self, items: &[crate::parser::PrintItem]) -> Result<()> {
        use crate::parser::PrintItem;
        
        for item in items {
            match item {
                PrintItem::Expression(expr) => {
                    // Evaluate expression and print it
                    let output = self.format_expression(expr)?;
                    self.print_output(&output);
                }
                PrintItem::Semicolon => {
                    // Semicolon suppresses newline (do nothing)
                }
                PrintItem::Comma => {
                    // Comma moves to next tab position (TAB(10) intervals)
                    #[cfg(test)]
                    {
                        let current_len = self.output.len();
                        let next_tab = ((current_len / 10) + 1) * 10;
                        let spaces = next_tab - current_len;
                        self.output.push_str(&" ".repeat(spaces));
                    }
                    #[cfg(not(test))]
                    {
                        print!("\t");
                    }
                }
                PrintItem::Tab(expr) => {
                    let pos = self.eval_integer(expr)? as usize;
                    #[cfg(test)]
                    {
                        let current_len = self.output.len();
                        if pos > current_len {
                            self.output.push_str(&" ".repeat(pos - current_len));
                        }
                    }
                    #[cfg(not(test))]
                    {
                        // Move to absolute position (simplified)
                        print!("{}", " ".repeat(pos));
                    }
                }
                PrintItem::Spc(expr) => {
                    let count = self.eval_integer(expr)? as usize;
                    self.print_output(&" ".repeat(count));
                }
            }
        }
        
        // Add newline unless last item was semicolon
        if items.is_empty() || !matches!(items.last(), Some(PrintItem::Semicolon)) {
            #[cfg(test)]
            {
                self.output.push('\n');
            }
            #[cfg(not(test))]
            {
                println!();
            }
        }
        
        Ok(())
    }
    
    /// Format an expression for printing
    fn format_expression(&self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::Integer(_) => Ok(self.eval_integer(expr)?.to_string()),
            Expression::Real(_) => Ok(self.eval_real(expr)?.to_string()),
            Expression::String(_) => self.eval_string(expr),
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    Ok(self.eval_integer(expr)?.to_string())
                } else if name.ends_with('$') {
                    self.eval_string(expr)
                } else {
                    Ok(self.eval_real(expr)?.to_string())
                }
            }
            _ => {
                // Try to evaluate as different types
                if let Ok(val) = self.eval_integer(expr) {
                    Ok(val.to_string())
                } else if let Ok(val) = self.eval_real(expr) {
                    Ok(val.to_string())
                } else if let Ok(val) = self.eval_string(expr) {
                    Ok(val)
                } else {
                    Err(BBCBasicError::TypeMismatch)
                }
            }
        }
    }
    
    /// Print output (to buffer in test mode, to stdout in production)
    fn print_output(&mut self, text: &str) {
        #[cfg(test)]
        {
            self.output.push_str(text);
        }
        #[cfg(not(test))]
        {
            print!("{}", text);
        }
    }
    
    /// Get output buffer (for testing)
    #[cfg(test)]
    pub fn get_output(&self) -> &str {
        &self.output
    }
    
    /// Clear output buffer (for testing)
    #[cfg(test)]
    pub fn clear_output(&mut self) {
        self.output.clear();
    }
    
    /// Execute GOTO statement
    fn execute_goto(&mut self, _line_number: u16) -> Result<()> {
        // In a full program executor, this would change the program counter
        // For now, we just acknowledge the command
        // This will be fully implemented when we add program storage
        Ok(())
    }
    
    /// Execute GOSUB statement
    fn execute_gosub(&mut self, line_number: u16) -> Result<()> {
        // Push return address to stack
        // In a real implementation, we'd push the NEXT line after this GOSUB
        // For now, we push the target line (simplified)
        self.return_stack.push(line_number);
        Ok(())
    }
    
    /// Execute RETURN statement
    fn execute_return(&mut self) -> Result<()> {
        // Pop return address from stack
        if self.return_stack.is_empty() {
            Err(BBCBasicError::BadCall)
        } else {
            self.return_stack.pop();
            Ok(())
        }
    }
    
    /// Execute FOR statement
    fn execute_for(&mut self, variable: &str, start: &Expression, end: &Expression, step: Option<&Expression>) -> Result<()> {
        // Evaluate start, end, and step values
        let start_val = self.eval_integer(start)?;
        let end_val = self.eval_integer(end)?;
        let step_val = if let Some(step_expr) = step {
            self.eval_integer(step_expr)?
        } else {
            1  // Default step is 1
        };
        
        // Set loop variable to start value
        self.variables.set_integer_var(variable.to_string(), start_val);
        
        // Store loop state: (variable, end_value, step_value, loop_line)
        // loop_line would be the line number in a real program
        self.for_loops.push((variable.to_string(), end_val, step_val, 0));
        
        Ok(())
    }
    
    /// Execute NEXT statement
    fn execute_next(&mut self, variables: &[String]) -> Result<()> {
        // If no variables specified, use the most recent FOR loop
        let var_name = if variables.is_empty() {
            if let Some((name, _, _, _)) = self.for_loops.last() {
                name.clone()
            } else {
                return Err(BBCBasicError::BadCall);
            }
        } else {
            variables[0].clone()
        };
        
        // Find the matching FOR loop
        let loop_index = self.for_loops.iter()
            .rposition(|(name, _, _, _)| name == &var_name)
            .ok_or(BBCBasicError::BadCall)?;
        
        let (_, end_val, step_val, _) = self.for_loops[loop_index];
        
        // Get current loop variable value
        let current_val = self.variables.get_integer_var(&var_name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(var_name.clone()))?;
        
        // Increment the loop variable
        let next_val = current_val + step_val;
        self.variables.set_integer_var(var_name.clone(), next_val);
        
        // Check if loop is complete
        let loop_complete = if step_val > 0 {
            next_val > end_val
        } else {
            next_val < end_val
        };
        
        if loop_complete {
            // Remove the loop from the stack
            self.for_loops.remove(loop_index);
        }
        // In a real program, we'd jump back to the FOR statement line if not complete
        
        Ok(())
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
    
    #[test]
    fn test_print_integer() {
        // RED: Test PRINT 42
        use crate::parser::PrintItem;
        
        let mut executor = Executor::new();
        let stmt = Statement::Print {
            items: vec![PrintItem::Expression(Expression::Integer(42))],
        };
        
        executor.execute_statement(&stmt).unwrap();
        assert_eq!(executor.get_output(), "42\n");
    }
    
    #[test]
    fn test_print_string() {
        // RED: Test PRINT "HELLO"
        use crate::parser::PrintItem;
        
        let mut executor = Executor::new();
        let stmt = Statement::Print {
            items: vec![PrintItem::Expression(Expression::String("HELLO".to_string()))],
        };
        
        executor.execute_statement(&stmt).unwrap();
        assert_eq!(executor.get_output(), "HELLO\n");
    }
    
    #[test]
    fn test_print_variable() {
        // RED: Test PRINT A% (after A% = 100)
        use crate::parser::PrintItem;
        
        let mut executor = Executor::new();
        
        // Set A% = 100
        let assign = Statement::Assignment {
            target: "A%".to_string(),
            expression: Expression::Integer(100),
        };
        executor.execute_statement(&assign).unwrap();
        
        // PRINT A%
        let print = Statement::Print {
            items: vec![PrintItem::Expression(Expression::Variable("A%".to_string()))],
        };
        executor.execute_statement(&print).unwrap();
        
        assert_eq!(executor.get_output(), "100\n");
    }
    
    #[test]
    fn test_print_multiple_items() {
        // RED: Test PRINT "Value:"; A%
        use crate::parser::PrintItem;
        
        let mut executor = Executor::new();
        
        // Set A% = 42
        let assign = Statement::Assignment {
            target: "A%".to_string(),
            expression: Expression::Integer(42),
        };
        executor.execute_statement(&assign).unwrap();
        
        // PRINT "Value:"; A%
        let print = Statement::Print {
            items: vec![
                PrintItem::Expression(Expression::String("Value:".to_string())),
                PrintItem::Semicolon,
                PrintItem::Expression(Expression::Variable("A%".to_string())),
            ],
        };
        executor.execute_statement(&print).unwrap();
        
        assert_eq!(executor.get_output(), "Value:42\n");
    }
    
    #[test]
    fn test_print_with_comma() {
        // RED: Test PRINT "A", "B"
        use crate::parser::PrintItem;
        
        let mut executor = Executor::new();
        let stmt = Statement::Print {
            items: vec![
                PrintItem::Expression(Expression::String("A".to_string())),
                PrintItem::Comma,
                PrintItem::Expression(Expression::String("B".to_string())),
            ],
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Comma should add spaces to next tab stop (10-char intervals)
        let output = executor.get_output();
        assert!(output.contains("A"));
        assert!(output.contains("B"));
        assert!(output.ends_with("\n"));
    }
    
    #[test]
    fn test_end_statement() {
        // RED: Test END statement
        let mut executor = Executor::new();
        let stmt = Statement::End;
        
        // Should execute without error
        executor.execute_statement(&stmt).unwrap();
    }
    
    #[test]
    fn test_stop_statement() {
        // RED: Test STOP statement
        let mut executor = Executor::new();
        let stmt = Statement::Stop;
        
        // Should execute without error
        executor.execute_statement(&stmt).unwrap();
    }
    
    #[test]
    fn test_rem_statement() {
        // RED: Test REM statement
        let mut executor = Executor::new();
        let stmt = Statement::Rem {
            comment: "This is a comment".to_string(),
        };
        
        // Should execute without error (does nothing)
        executor.execute_statement(&stmt).unwrap();
    }
    
    #[test]
    fn test_gosub_return() {
        // RED: Test GOSUB/RETURN sequence
        let mut executor = Executor::new();
        
        // Initially, return stack should be empty
        assert!(executor.return_stack.is_empty());
        
        // Execute GOSUB 1000
        let gosub = Statement::Gosub { line_number: 1000 };
        executor.execute_statement(&gosub).unwrap();
        
        // Return stack should have one entry
        assert_eq!(executor.return_stack.len(), 1);
        
        // Execute RETURN
        let return_stmt = Statement::Return;
        executor.execute_statement(&return_stmt).unwrap();
        
        // Return stack should be empty again
        assert!(executor.return_stack.is_empty());
    }
    
    #[test]
    fn test_return_without_gosub() {
        // RED: Test RETURN without GOSUB should error
        let mut executor = Executor::new();
        let stmt = Statement::Return;
        
        // Should return an error
        let result = executor.execute_statement(&stmt);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BBCBasicError::BadCall));
    }
    
    #[test]
    fn test_goto_statement() {
        // RED: Test GOTO statement
        let mut executor = Executor::new();
        let stmt = Statement::Goto { line_number: 500 };
        
        // Should execute without error
        // (Full implementation requires program storage)
        executor.execute_statement(&stmt).unwrap();
    }
    
    #[test]
    fn test_for_loop_initialization() {
        // RED: Test FOR I% = 1 TO 10
        let mut executor = Executor::new();
        let stmt = Statement::For {
            variable: "I%".to_string(),
            start: Expression::Integer(1),
            end: Expression::Integer(10),
            step: None,
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Loop variable should be set to start value
        assert_eq!(executor.get_variable_int("I%").unwrap(), 1);
        
        // Loop should be on the stack
        assert_eq!(executor.for_loops.len(), 1);
        assert_eq!(executor.for_loops[0].0, "I%");
        assert_eq!(executor.for_loops[0].1, 10);  // end value
        assert_eq!(executor.for_loops[0].2, 1);   // step value
    }
    
    #[test]
    fn test_for_loop_with_step() {
        // RED: Test FOR I% = 10 TO 1 STEP -1
        let mut executor = Executor::new();
        let stmt = Statement::For {
            variable: "I%".to_string(),
            start: Expression::Integer(10),
            end: Expression::Integer(1),
            step: Some(Expression::Integer(-1)),
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Loop variable should be set to start value
        assert_eq!(executor.get_variable_int("I%").unwrap(), 10);
        
        // Loop should be on the stack with correct step
        assert_eq!(executor.for_loops.len(), 1);
        assert_eq!(executor.for_loops[0].2, -1);  // step value
    }
    
    #[test]
    fn test_next_statement() {
        // RED: Test FOR...NEXT loop execution
        let mut executor = Executor::new();
        
        // FOR I% = 1 TO 3
        let for_stmt = Statement::For {
            variable: "I%".to_string(),
            start: Expression::Integer(1),
            end: Expression::Integer(3),
            step: None,
        };
        executor.execute_statement(&for_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 1);
        
        // NEXT I%
        let next_stmt = Statement::Next {
            variables: vec!["I%".to_string()],
        };
        
        // First NEXT: I% should become 2
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 2);
        assert_eq!(executor.for_loops.len(), 1);  // Loop still active
        
        // Second NEXT: I% should become 3
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 3);
        assert_eq!(executor.for_loops.len(), 1);  // Loop still active
        
        // Third NEXT: I% should become 4, loop should complete
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 4);
        assert_eq!(executor.for_loops.len(), 0);  // Loop completed and removed
    }
    
    #[test]
    fn test_next_without_for() {
        // RED: Test NEXT without FOR should error
        let mut executor = Executor::new();
        let stmt = Statement::Next {
            variables: vec!["I%".to_string()],
        };
        
        let result = executor.execute_statement(&stmt);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BBCBasicError::BadCall));
    }
    
    #[test]
    fn test_for_loop_countdown() {
        // RED: Test FOR I% = 5 TO 1 STEP -1
        let mut executor = Executor::new();
        
        let for_stmt = Statement::For {
            variable: "I%".to_string(),
            start: Expression::Integer(5),
            end: Expression::Integer(1),
            step: Some(Expression::Integer(-1)),
        };
        executor.execute_statement(&for_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 5);
        
        let next_stmt = Statement::Next {
            variables: vec!["I%".to_string()],
        };
        
        // Countdown: 5, 4, 3, 2, 1
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 4);
        
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 3);
        
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 2);
        
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 1);
        
        // One more NEXT should exit the loop
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 0);
        assert_eq!(executor.for_loops.len(), 0);
    }
}

