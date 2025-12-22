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
            Statement::Input { variables } => {
                self.execute_input(variables)
            }
            Statement::Dim { arrays } => {
                self.execute_dim(arrays)
            }
            Statement::If { condition, then_part, else_part } => {
                self.execute_if(condition, then_part, else_part.as_ref())
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
    
    /// Execute INPUT statement
    fn execute_input(&mut self, _variables: &[String]) -> Result<()> {
        // In a real implementation, this would read from stdin
        // For now, we'll just set default values for testing
        // Full implementation requires I/O handling
        #[cfg(test)]
        {
            // In test mode, set variables to test values
            for var in _variables {
                if var.ends_with('%') {
                    self.variables.set_integer_var(var.clone(), 0);
                } else if var.ends_with('$') {
                    self.variables.set_string_var(var.clone(), String::new())?;
                } else {
                    self.variables.set_real_var(var.clone(), 0.0);
                }
            }
        }
        #[cfg(not(test))]
        {
            // Production mode: read from stdin
            use std::io::{self, Write};
            
            for var in _variables {
                print!("? ");
                io::stdout().flush().unwrap();
                
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                let input = input.trim();
                
                if var.ends_with('%') {
                    if let Ok(val) = input.parse::<i32>() {
                        self.variables.set_integer_var(var.clone(), val);
                    }
                } else if var.ends_with('$') {
                    self.variables.set_string_var(var.clone(), input.to_string())?;
                } else {
                    if let Ok(val) = input.parse::<f64>() {
                        self.variables.set_real_var(var.clone(), val);
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Execute DIM statement
    fn execute_dim(&mut self, arrays: &[(String, Vec<Expression>)]) -> Result<()> {
        for (name, dimensions) in arrays {
            // Evaluate dimension expressions
            let mut dim_sizes = Vec::new();
            for dim_expr in dimensions {
                let size = self.eval_integer(dim_expr)?;
                if size < 0 {
                    return Err(BBCBasicError::SubscriptOutOfRange);
                }
                dim_sizes.push(size as usize);
            }
            
            // Determine array type from variable name suffix
            use crate::variables::VarType;
            let var_type = if name.ends_with('%') {
                VarType::Integer
            } else if name.ends_with('$') {
                VarType::String
            } else {
                VarType::Real
            };
            
            // Create array in variable store
            self.variables.dim_array(name.clone(), dim_sizes, var_type)?;
        }
        Ok(())
    }
    
    /// Execute an IF statement
    fn execute_if(
        &mut self,
        condition: &Expression,
        then_part: &[Statement],
        else_part: Option<&Vec<Statement>>,
    ) -> Result<()> {
        // Evaluate condition - in BBC BASIC, 0 is false, non-zero is true
        let condition_value = self.eval_integer(condition)?;
        
        if condition_value != 0 {
            // Condition is true: execute then_part
            for stmt in then_part {
                self.execute_statement(stmt)?;
            }
        } else if let Some(else_statements) = else_part {
            // Condition is false and ELSE exists: execute else_part
            for stmt in else_statements {
                self.execute_statement(stmt)?;
            }
        }
        
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
                    // Comparison operators: return -1 for true, 0 for false (BBC BASIC convention)
                    BinaryOperator::Equal => Ok(if left_val == right_val { -1 } else { 0 }),
                    BinaryOperator::NotEqual => Ok(if left_val != right_val { -1 } else { 0 }),
                    BinaryOperator::LessThan => Ok(if left_val < right_val { -1 } else { 0 }),
                    BinaryOperator::LessThanOrEqual => Ok(if left_val <= right_val { -1 } else { 0 }),
                    BinaryOperator::GreaterThan => Ok(if left_val > right_val { -1 } else { 0 }),
                    BinaryOperator::GreaterThanOrEqual => Ok(if left_val >= right_val { -1 } else { 0 }),
                    // Logical operators
                    BinaryOperator::And => Ok(left_val & right_val),
                    BinaryOperator::Or => Ok(left_val | right_val),
                    BinaryOperator::Eor => Ok(left_val ^ right_val),
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
            Expression::FunctionCall { name, args } => {
                self.eval_function_int(name, args)
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
                } else if name.ends_with('$') {
                    // String variable can't be converted to real
                    Err(BBCBasicError::TypeMismatch)
                } else {
                    // Try as real variable first, then as integer
                    if let Some(real_val) = self.variables.get_real_var(name) {
                        Ok(real_val)
                    } else if let Some(int_val) = self.variables.get_integer_var(name) {
                        Ok(int_val as f64)
                    } else {
                        Err(BBCBasicError::NoSuchVariable(name.clone()))
                    }
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
            Expression::FunctionCall { name, args } => {
                self.eval_function_real(name, args)
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
    
    /// Evaluate a function call returning an integer
    fn eval_function_int(&self, name: &str, args: &[Expression]) -> Result<i32> {
        match name {
            "ABS" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ABS requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_integer(&args[0])?;
                Ok(val.abs())
            }
            "INT" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "INT requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                Ok(val.floor() as i32)
            }
            "SGN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "SGN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_integer(&args[0])?;
                Ok(if val < 0 { -1 } else if val > 0 { 1 } else { 0 })
            }
            // Real-only functions should not be called as integers
            "SIN" | "COS" | "TAN" | "ATN" | "SQR" | "EXP" | "LN" | "LOG" | "DEG" | "RAD" | "PI" | "RND" => {
                Err(BBCBasicError::TypeMismatch)
            }
            _ => {
                Err(BBCBasicError::SyntaxError {
                    message: format!("Unknown function: {}", name),
                    line: None,
                })
            }
        }
    }
    
    /// Evaluate a function call returning a real number
    fn eval_function_real(&self, name: &str, args: &[Expression]) -> Result<f64> {
        match name {
            "SIN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "SIN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let degrees = self.eval_real(&args[0])?;
                let radians = degrees.to_radians();
                Ok(radians.sin())
            }
            "COS" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "COS requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let degrees = self.eval_real(&args[0])?;
                let radians = degrees.to_radians();
                Ok(radians.cos())
            }
            "TAN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "TAN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let degrees = self.eval_real(&args[0])?;
                let radians = degrees.to_radians();
                Ok(radians.tan())
            }
            "ATN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ATN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                let radians = val.atan();
                Ok(radians.to_degrees())
            }
            "SQR" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "SQR requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                if val < 0.0 {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.sqrt())
            }
            "ABS" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ABS requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                Ok(val.abs())
            }
            "EXP" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "EXP requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                Ok(val.exp())
            }
            "LN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "LN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                if val <= 0.0 {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.ln())
            }
            "LOG" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "LOG requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                if val <= 0.0 {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.log10())
            }
            "DEG" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "DEG requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                Ok(val.to_degrees())
            }
            "RAD" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "RAD requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                Ok(val.to_radians())
            }
            "PI" => {
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "PI takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(std::f64::consts::PI)
            }
            "RND" => {
                // RND(n) returns random number
                // For now, just return 0.5 (deterministic for testing)
                // TODO: Implement proper random number generation
                Ok(0.5)
            }
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown function: {}", name),
                line: None,
            }),
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
    
    /// Check if the last NEXT caused a loop to continue (not complete)
    /// Returns Some(line_number) if should loop back, None if loop completed
    pub fn should_loop_back(&self) -> Option<u16> {
        // If there are active FOR loops, return the line number of the most recent one
        // This will be called after execute_next to determine if we should jump back
        self.for_loops.last().map(|(_, _, _, line)| *line)
    }
    
    /// Set the line number for a FOR loop (called when FOR is executed)
    pub fn set_for_loop_line(&mut self, line_number: u16) {
        if let Some(loop_state) = self.for_loops.last_mut() {
            loop_state.3 = line_number;
        }
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
    
    #[test]
    fn test_input_statement() {
        // RED: Test INPUT A%, B$, C
        let mut executor = Executor::new();
        let stmt = Statement::Input {
            variables: vec![
                "A%".to_string(),
                "B$".to_string(),
                "C".to_string(),
            ],
        };
        
        // In test mode, INPUT initializes variables to default values
        executor.execute_statement(&stmt).unwrap();
        
        // Variables should be initialized
        assert_eq!(executor.get_variable_int("A%").unwrap(), 0);
        assert_eq!(executor.get_variable_string("B$").unwrap(), "");
        assert_eq!(executor.get_variable_real("C").unwrap(), 0.0);
    }
    
    #[test]
    fn test_dim_integer_array() {
        // RED: Test DIM A%(10)
        let mut executor = Executor::new();
        let stmt = Statement::Dim {
            arrays: vec![(
                "A%".to_string(),
                vec![Expression::Integer(10)],
            )],
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Array should be created (we can verify by trying to use it)
        // This test just verifies no error occurs
    }
    
    #[test]
    fn test_dim_multi_dimensional_array() {
        // RED: Test DIM B%(5, 10)
        let mut executor = Executor::new();
        let stmt = Statement::Dim {
            arrays: vec![(
                "B%".to_string(),
                vec![Expression::Integer(5), Expression::Integer(10)],
            )],
        };
        
        executor.execute_statement(&stmt).unwrap();
        // 2D array should be created
    }
    
    #[test]
    fn test_dim_multiple_arrays() {
        // RED: Test DIM A%(10), B$(5)
        let mut executor = Executor::new();
        let stmt = Statement::Dim {
            arrays: vec![
                ("A%".to_string(), vec![Expression::Integer(10)]),
                ("B$".to_string(), vec![Expression::Integer(5)]),
            ],
        };
        
        executor.execute_statement(&stmt).unwrap();
        // Both arrays should be created
    }
    
    #[test]
    fn test_if_then_true_condition() {
        // RED: Test IF X% > 5 THEN Y% = 10
        let mut executor = Executor::new();
        
        // Set X% = 7
        executor.variables.set_integer_var("X%".to_string(), 7);
        
        let stmt = Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: crate::parser::BinaryOperator::GreaterThan,
                right: Box::new(Expression::Integer(5)),
            },
            then_part: vec![Statement::Assignment {
                target: "Y%".to_string(),
                expression: Expression::Integer(10),
            }],
            else_part: None,
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Y% should be set to 10 because condition is true
        assert_eq!(executor.get_variable_int("Y%").unwrap(), 10);
    }
    
    #[test]
    fn test_if_then_false_condition() {
        // RED: Test IF X% > 5 THEN Y% = 10 (with X% = 3)
        let mut executor = Executor::new();
        
        // Set X% = 3
        executor.variables.set_integer_var("X%".to_string(), 3);
        
        let stmt = Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: crate::parser::BinaryOperator::GreaterThan,
                right: Box::new(Expression::Integer(5)),
            },
            then_part: vec![Statement::Assignment {
                target: "Y%".to_string(),
                expression: Expression::Integer(10),
            }],
            else_part: None,
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Y% should not exist because condition is false
        assert!(executor.get_variable_int("Y%").is_err());
    }
    
    #[test]
    fn test_if_then_else_true_condition() {
        // RED: Test IF X% = 5 THEN Y% = 1 ELSE Y% = 2
        let mut executor = Executor::new();
        
        executor.variables.set_integer_var("X%".to_string(), 5);
        
        let stmt = Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: crate::parser::BinaryOperator::Equal,
                right: Box::new(Expression::Integer(5)),
            },
            then_part: vec![Statement::Assignment {
                target: "Y%".to_string(),
                expression: Expression::Integer(1),
            }],
            else_part: Some(vec![Statement::Assignment {
                target: "Y%".to_string(),
                expression: Expression::Integer(2),
            }]),
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Y% should be 1 because condition is true
        assert_eq!(executor.get_variable_int("Y%").unwrap(), 1);
    }
    
    #[test]
    fn test_if_then_else_false_condition() {
        // RED: Test IF X% = 5 THEN Y% = 1 ELSE Y% = 2 (with X% = 3)
        let mut executor = Executor::new();
        
        executor.variables.set_integer_var("X%".to_string(), 3);
        
        let stmt = Statement::If {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: crate::parser::BinaryOperator::Equal,
                right: Box::new(Expression::Integer(5)),
            },
            then_part: vec![Statement::Assignment {
                target: "Y%".to_string(),
                expression: Expression::Integer(1),
            }],
            else_part: Some(vec![Statement::Assignment {
                target: "Y%".to_string(),
                expression: Expression::Integer(2),
            }]),
        };
        
        executor.execute_statement(&stmt).unwrap();
        
        // Y% should be 2 because condition is false
        assert_eq!(executor.get_variable_int("Y%").unwrap(), 2);
    }
    
    // Built-in function tests
    
    #[test]
    fn test_sin_function() {
        // RED: Test SIN(90) = 1 (BBC BASIC uses degrees)
        let mut executor = Executor::new();
        
        let sin_90 = Expression::FunctionCall {
            name: "SIN".to_string(),
            args: vec![Expression::Real(90.0)],
        };
        
        let result = executor.eval_real(&sin_90).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }
    
    #[test]
    fn test_cos_function() {
        // RED: Test COS(0) = 1
        let mut executor = Executor::new();
        
        let cos_expr = Expression::FunctionCall {
            name: "COS".to_string(),
            args: vec![Expression::Real(0.0)],
        };
        
        let result = executor.eval_real(&cos_expr).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }
    
    #[test]
    fn test_abs_function() {
        // RED: Test ABS(-5) = 5, ABS(3.5) = 3.5
        let mut executor = Executor::new();
        
        let abs_int = Expression::FunctionCall {
            name: "ABS".to_string(),
            args: vec![Expression::Integer(-5)],
        };
        
        let result = executor.eval_integer(&abs_int).unwrap();
        assert_eq!(result, 5);
        
        let abs_real = Expression::FunctionCall {
            name: "ABS".to_string(),
            args: vec![Expression::Real(-3.5)],
        };
        
        let result = executor.eval_real(&abs_real).unwrap();
        assert!((result - 3.5).abs() < 0.0001);
    }
    
    #[test]
    fn test_sqr_function() {
        // RED: Test SQR(16) = 4, SQR(2) ≈ 1.414
        let mut executor = Executor::new();
        
        let sqr_expr = Expression::FunctionCall {
            name: "SQR".to_string(),
            args: vec![Expression::Integer(16)],
        };
        
        let result = executor.eval_real(&sqr_expr).unwrap();
        assert!((result - 4.0).abs() < 0.0001);
    }
    
    #[test]
    fn test_int_function() {
        // RED: Test INT(3.7) = 3, INT(-2.3) = -3
        let mut executor = Executor::new();
        
        let int_expr = Expression::FunctionCall {
            name: "INT".to_string(),
            args: vec![Expression::Real(3.7)],
        };
        
        let result = executor.eval_integer(&int_expr).unwrap();
        assert_eq!(result, 3);
        
        // INT floors toward negative infinity
        let int_neg = Expression::FunctionCall {
            name: "INT".to_string(),
            args: vec![Expression::Real(-2.3)],
        };
        
        let result = executor.eval_integer(&int_neg).unwrap();
        assert_eq!(result, -3);
    }
    
    #[test]
    fn test_sgn_function() {
        // RED: Test SGN(-5) = -1, SGN(0) = 0, SGN(10) = 1
        let mut executor = Executor::new();
        
        let sgn_neg = Expression::FunctionCall {
            name: "SGN".to_string(),
            args: vec![Expression::Integer(-5)],
        };
        assert_eq!(executor.eval_integer(&sgn_neg).unwrap(), -1);
        
        let sgn_zero = Expression::FunctionCall {
            name: "SGN".to_string(),
            args: vec![Expression::Integer(0)],
        };
        assert_eq!(executor.eval_integer(&sgn_zero).unwrap(), 0);
        
        let sgn_pos = Expression::FunctionCall {
            name: "SGN".to_string(),
            args: vec![Expression::Integer(10)],
        };
        assert_eq!(executor.eval_integer(&sgn_pos).unwrap(), 1);
    }
    
    #[test]
    fn test_pi_constant() {
        // RED: Test PI ≈ 3.14159
        let mut executor = Executor::new();
        
        let pi_expr = Expression::FunctionCall {
            name: "PI".to_string(),
            args: vec![],
        };
        
        let result = executor.eval_real(&pi_expr).unwrap();
        assert!((result - std::f64::consts::PI).abs() < 0.0001);
    }
    
    #[test]
    fn test_function_in_assignment() {
        // Test that functions work in assignments
        let mut executor = Executor::new();
        
        // X% = ABS(-5)
        let stmt = Statement::Assignment {
            target: "X%".to_string(),
            expression: Expression::FunctionCall {
                name: "ABS".to_string(),
                args: vec![Expression::Integer(-5)],
            },
        };
        
        executor.execute_statement(&stmt).unwrap();
        assert_eq!(executor.get_variable_int("X%").unwrap(), 5);
    }
}

