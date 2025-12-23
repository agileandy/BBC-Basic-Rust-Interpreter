//! Execution engine for BBC BASIC statements
//!
//! Executes parsed BBC BASIC statements with proper control flow handling.

use crate::error::{BBCBasicError, Result};
use crate::memory::MemoryManager;
use crate::parser::{DataValue, Expression, Statement};
use crate::variables::{Variable, VariableStore};
use rand::Rng;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

/// File handle for file I/O operations
#[derive(Debug)]
enum FileHandle {
    /// File opened for reading (OPENIN)
    Input(BufReader<File>),
    /// File opened for writing (OPENOUT)
    Output(BufWriter<File>),
}

/// Local variable frame for procedure/function scoping
#[derive(Debug, Clone)]
struct LocalFrame {
    /// Saved variable values (variable name -> saved value)
    saved_variables: HashMap<String, Option<Variable>>,
}

impl LocalFrame {
    fn new() -> Self {
        Self {
            saved_variables: HashMap::new(),
        }
    }
}

/// Procedure definition
#[derive(Debug, Clone)]
pub struct ProcedureDefinition {
    pub line_number: u16,
    pub params: Vec<String>,
}

/// Function definition (DEF FN)
#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub params: Vec<String>,
    pub expression: Expression,
}

/// Error information for ON ERROR handling
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    /// Error number (ERR)
    pub error_number: i32,
    /// Line number where error occurred (ERL)
    pub error_line: u16,
    /// Error message
    pub message: String,
}

/// BBC BASIC statement executor
#[derive(Debug)]
pub struct Executor {
    variables: VariableStore,
    memory: MemoryManager,
    // Control flow stack for GOSUB/RETURN
    return_stack: Vec<u16>,
    // FOR loop state: (variable, end_value, step_value, loop_line)
    for_loops: Vec<(String, i32, i32, u16)>,
    // REPEAT loop stack: stores line numbers of REPEAT statements
    repeat_stack: Vec<u16>,
    // WHILE loop stack: stores line numbers of WHILE statements
    while_stack: Vec<u16>,
    // DATA storage: stores all DATA values in program order
    data_values: Vec<DataValue>,
    // DATA line numbers: tracks which line each DATA value came from (parallel to data_values)
    data_line_numbers: Vec<Option<u16>>,
    // DATA pointer: current index in data_values
    data_pointer: usize,
    // Current line number being executed (for DATA tracking)
    current_line: Option<u16>,
    // Random number generator for RND function (wrapped in RefCell for interior mutability)
    rng: RefCell<rand::rngs::ThreadRng>,
    // Procedure definitions: name -> (line_number, params)
    procedures: HashMap<String, ProcedureDefinition>,
    // Function definitions (DEF FN): name -> (params, expression)
    functions: HashMap<String, FunctionDefinition>,
    // Local variable stack for PROC/FN scoping
    local_stack: Vec<LocalFrame>,
    // Error handler: line number to jump to on error (None = no handler)
    error_handler: Option<u16>,
    // Last error information (for ERL and ERR functions)
    last_error: Option<ErrorInfo>,
    // Open file handles: handle number -> FileHandle
    open_files: HashMap<i32, FileHandle>,
    // Next file handle number to allocate
    next_file_handle: i32,
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
            repeat_stack: Vec::new(),
            while_stack: Vec::new(),
            data_values: Vec::new(),
            data_line_numbers: Vec::new(),
            data_pointer: 0,
            current_line: None,
            rng: RefCell::new(rand::thread_rng()),
            procedures: HashMap::new(),
            functions: HashMap::new(),
            local_stack: Vec::new(),
            error_handler: None,
            last_error: None,
            open_files: HashMap::new(),
            next_file_handle: 1,
            #[cfg(test)]
            output: String::new(),
        }
    }

    /// Set the current line number (for tests and program execution tracking)
    pub fn set_line_number(&mut self, line_number: Option<u16>) {
        self.current_line = line_number;
    }

    /// Execute a statement
    pub fn execute_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Assignment { target, expression } => {
                self.execute_assignment(target, expression)
            }
            Statement::Print { items } => self.execute_print(items),
            Statement::End | Statement::Stop | Statement::Quit => {
                // END, STOP, and QUIT all stop execution
                // In a full program, this would signal the interpreter to halt
                Ok(())
            }
            Statement::Rem { .. } => {
                // Comments do nothing during execution
                Ok(())
            }
            Statement::Goto { line_number } => self.execute_goto(*line_number),
            Statement::Gosub { .. } => {
                // GOSUB is handled as control flow in main.rs
                Ok(())
            }
            Statement::Return { value } => {
                // RETURN is handled as control flow in main.rs
                // TODO: Implement return value storage for multi-line functions
                // Currently, single-line DEF FN uses the expression directly
                // Multi-line functions with RETURN expression will need:
                // 1. Function context stack to store return values
                // 2. Return value retrieval in function call handler
                if value.is_some() {
                    // Expression is present but not yet evaluated/stored
                    // This will be implemented when adding multi-line function support
                }
                Ok(())
            }
            Statement::For {
                variable,
                start,
                end,
                step,
            } => self.execute_for(variable, start, end, step.as_ref()),
            Statement::Next { variables } => self.execute_next(variables),
            Statement::Input { variables } => self.execute_input(variables),
            Statement::Dim { arrays } => self.execute_dim(arrays),
            Statement::If {
                condition,
                then_part,
                else_part,
            } => self.execute_if(condition, then_part, else_part.as_ref()),
            Statement::Data { values } => self.execute_data(values),
            Statement::Read { variables } => self.execute_read(variables),
            Statement::Restore { line_number } => self.execute_restore(*line_number),
            Statement::Repeat => {
                // REPEAT is handled as control flow in main.rs
                Ok(())
            }
            Statement::Until { .. } => {
                // UNTIL is handled as control flow in main.rs
                Ok(())
            }
            Statement::While { .. } => {
                // WHILE is handled as control flow in main.rs
                Ok(())
            }
            Statement::EndWhile => {
                // ENDWHILE is handled as control flow in main.rs
                Ok(())
            }
            Statement::Cls => self.execute_cls(),
            Statement::DefProc { .. } => {
                // DEF PROC is handled during procedure collection in main.rs
                Ok(())
            }
            Statement::DefFn {
                name,
                params,
                expression,
            } => self.execute_def_fn(name, params, expression),
            Statement::EndProc => {
                // ENDPROC is handled as control flow in main.rs
                Ok(())
            }
            Statement::Local { variables } => self.execute_local(variables),
            Statement::ProcCall { .. } => {
                // PROC calls are handled as control flow in main.rs
                Ok(())
            }
            Statement::OnError { line_number } => {
                self.set_error_handler(*line_number);
                Ok(())
            }
            Statement::OnErrorOff => {
                self.clear_error_handler();
                Ok(())
            }
            Statement::PrintFile { handle, items } => self.execute_print_file(handle, items),
            Statement::InputFile { handle, variables } => {
                self.execute_input_file(handle, variables)
            }
            Statement::CloseFile { handle } => self.execute_close_file(handle),
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
                    // TAB accepts both integer and real, truncating real to integer
                    let pos = if let Ok(int_val) = self.eval_integer(expr) {
                        int_val as usize
                    } else {
                        let real_val = self.eval_real(expr)?;
                        real_val.floor().max(0.0) as usize
                    };
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
                    // SPC accepts both integer and real, truncating real to integer
                    let count = if let Ok(int_val) = self.eval_integer(expr) {
                        int_val as usize
                    } else {
                        let real_val = self.eval_real(expr)?;
                        real_val.floor().max(0.0) as usize
                    };
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
    fn format_expression(&mut self, expr: &Expression) -> Result<String> {
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
    fn execute_for(
        &mut self,
        variable: &str,
        start: &Expression,
        end: &Expression,
        step: Option<&Expression>,
    ) -> Result<()> {
        // Evaluate start, end, and step values
        let start_val = self.eval_integer(start)?;
        let end_val = self.eval_integer(end)?;
        let step_val = if let Some(step_expr) = step {
            self.eval_integer(step_expr)?
        } else {
            1 // Default step is 1
        };

        // Set loop variable to start value
        self.variables
            .set_integer_var(variable.to_string(), start_val);

        // Store loop state: (variable, end_value, step_value, loop_line)
        // loop_line would be the line number in a real program
        self.for_loops
            .push((variable.to_string(), end_val, step_val, 0));

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
        let loop_index = self
            .for_loops
            .iter()
            .rposition(|(name, _, _, _)| name == &var_name)
            .ok_or(BBCBasicError::BadCall)?;

        let (_, end_val, step_val, _) = self.for_loops[loop_index];

        // Get current loop variable value
        let current_val = self
            .variables
            .get_integer_var(&var_name)
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
                    self.variables
                        .set_string_var(var.clone(), input.to_string())?;
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
            self.variables
                .dim_array(name.clone(), dim_sizes, var_type)?;
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

    /// Execute DATA statement - stores data values for READ
    fn execute_data(&mut self, values: &[DataValue]) -> Result<()> {
        // DATA statements append values to the data pool
        // Track which line each value came from
        for value in values {
            self.data_values.push(value.clone());
            self.data_line_numbers.push(self.current_line);
        }
        Ok(())
    }

    /// Collect DATA statement values without executing (for program pre-processing)
    /// This is used to collect all DATA statements before program execution begins
    pub fn collect_data(&mut self, statement: &Statement) -> Result<()> {
        if let Statement::Data { values } = statement {
            for value in values {
                self.data_values.push(value.clone());
                self.data_line_numbers.push(self.current_line);
            }
        }
        Ok(())
    }

    /// Reset DATA pointer and optionally clear all DATA values
    /// Called at the start of RUN to prepare for fresh program execution
    pub fn reset_data(&mut self) {
        self.data_values.clear();
        self.data_line_numbers.clear();
        self.data_pointer = 0;
    }

    /// Execute READ statement - reads data into variables
    fn execute_read(&mut self, variables: &[String]) -> Result<()> {
        for var_name in variables {
            // Check if we've run out of data
            if self.data_pointer >= self.data_values.len() {
                return Err(BBCBasicError::SyntaxError {
                    message: "Out of DATA".to_string(),
                    line: None,
                });
            }

            // Get next data value
            let data_value = &self.data_values[self.data_pointer];
            self.data_pointer += 1;

            // Assign to variable based on type
            if var_name.ends_with('%') {
                // Integer variable
                let int_val = match data_value {
                    DataValue::Integer(v) => *v,
                    DataValue::Real(v) => *v as i32,
                    DataValue::String(_) => 0, // BBC BASIC: string to number = 0
                };
                self.variables.set_integer_var(var_name.clone(), int_val);
            } else if var_name.ends_with('$') {
                // String variable
                let str_val = match data_value {
                    DataValue::String(s) => s.clone(),
                    DataValue::Integer(v) => v.to_string(),
                    DataValue::Real(v) => v.to_string(),
                };
                self.variables.set_string_var(var_name.clone(), str_val)?;
            } else {
                // Real variable
                let real_val = match data_value {
                    DataValue::Real(v) => *v,
                    DataValue::Integer(v) => *v as f64,
                    DataValue::String(_) => 0.0, // BBC BASIC: string to number = 0
                };
                self.variables.set_real_var(var_name.clone(), real_val);
            }
        }
        Ok(())
    }

    /// Execute RESTORE statement - resets data pointer
    fn execute_restore(&mut self, line_number: Option<u16>) -> Result<()> {
        if let Some(target_line) = line_number {
            // Find the first DATA value at or after the target line
            for (i, data_line) in self.data_line_numbers.iter().enumerate() {
                if let Some(line) = data_line {
                    if *line >= target_line {
                        self.data_pointer = i;
                        return Ok(());
                    }
                }
            }
            // If no DATA found at or after target line, error
            return Err(BBCBasicError::SyntaxError {
                message: format!("No DATA at line {}", target_line),
                line: None,
            });
        } else {
            // No line number: reset to beginning
            self.data_pointer = 0;
        }
        Ok(())
    }

    /// Execute CLS statement - clear screen
    fn execute_cls(&mut self) -> Result<()> {
        // Output ANSI escape sequences to clear screen and move cursor to home
        // ESC[2J clears the entire screen
        // ESC[H moves cursor to home position (0,0)
        #[cfg(test)]
        {
            self.output.push_str("\x1b[2J\x1b[H");
        }
        #[cfg(not(test))]
        {
            print!("\x1b[2J\x1b[H");
        }
        Ok(())
    }

    /// Evaluate an expression to an integer value
    pub fn eval_integer(&mut self, expr: &Expression) -> Result<i32> {
        match expr {
            Expression::Integer(val) => Ok(*val),
            Expression::Real(val) => Ok(*val as i32),
            Expression::Variable(name) => {
                // Check for pseudo-variables first
                if name == "TIME" {
                    // TIME returns centiseconds since program start or system boot
                    // For now, use system time
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap();
                    // Convert to centiseconds (1/100th of a second)
                    let centiseconds = (now.as_millis() / 10) as i32;
                    return Ok(centiseconds);
                } else if name == "HIMEM" {
                    // HIMEM returns top of available memory
                    return Ok(self.memory.get_himem() as i32);
                } else if name == "LOMEM" {
                    // LOMEM returns bottom of user memory (PAGE)
                    return Ok(self.memory.get_page() as i32);
                } else if name == "ERR" {
                    // ERR returns the last error number (0 if no error)
                    return Ok(self.last_error.as_ref().map(|e| e.error_number).unwrap_or(0));
                } else if name == "ERL" {
                    // ERL returns the line number where the last error occurred (0 if no error)
                    return Ok(self.last_error.as_ref().map(|e| e.error_line as i32).unwrap_or(0));
                }

                if name.ends_with('%') {
                    self.variables
                        .get_integer_var(name)
                        .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))
                } else {
                    let real_val = self
                        .variables
                        .get_real_var(name)
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
                    BinaryOperator::LessThanOrEqual => {
                        Ok(if left_val <= right_val { -1 } else { 0 })
                    }
                    BinaryOperator::GreaterThan => Ok(if left_val > right_val { -1 } else { 0 }),
                    BinaryOperator::GreaterThanOrEqual => {
                        Ok(if left_val >= right_val { -1 } else { 0 })
                    }
                    // Logical operators
                    BinaryOperator::And => Ok(left_val & right_val),
                    BinaryOperator::Or => Ok(left_val | right_val),
                    BinaryOperator::Eor => Ok(left_val ^ right_val),
                    // Bitwise shift operators
                    BinaryOperator::LeftShift => {
                        if right_val < 0 {
                            return Err(BBCBasicError::IllegalFunction);
                        }
                        Ok(left_val << right_val)
                    }
                    BinaryOperator::RightShift => {
                        if right_val < 0 {
                            return Err(BBCBasicError::IllegalFunction);
                        }
                        Ok(left_val >> right_val)
                    }
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
            Expression::FunctionCall { name, args } => self.eval_function_int(name, args),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Evaluate an expression to a real value
    fn eval_real(&mut self, expr: &Expression) -> Result<f64> {
        match expr {
            Expression::Integer(val) => Ok(*val as f64),
            Expression::Real(val) => Ok(*val),
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    let int_val = self
                        .variables
                        .get_integer_var(name)
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
            Expression::FunctionCall { name, args } => self.eval_function_real(name, args),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Evaluate an expression to a string value
    fn eval_string(&mut self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::String(val) => Ok(val.clone()),
            Expression::Variable(name) => self
                .variables
                .get_string_var(name)
                .map(|s| s.to_string())
                .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone())),
            Expression::FunctionCall { name, args } => self.eval_function_string(name, args),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Evaluate a function call returning an integer
    fn eval_function_int(&mut self, name: &str, args: &[Expression]) -> Result<i32> {
        // Check if this is a user-defined function first
        if self.functions.contains_key(name) {
            return self.call_function_int(name, args);
        }

        // Otherwise, it's a built-in function
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
                Ok(if val < 0 {
                    -1
                } else if val > 0 {
                    1
                } else {
                    0
                })
            }
            "ASC" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ASC requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                if s.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ASC requires non-empty string".to_string(),
                        line: None,
                    });
                }
                Ok(s.chars().next().unwrap() as i32)
            }
            "LEN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "LEN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                Ok(s.len() as i32)
            }
            "VAL" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "VAL requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                s.trim().parse::<i32>().or_else(|_| Ok(0)) // BBC BASIC returns 0 for non-numeric strings
            }
            "ERL" => {
                // Error line number - returns 0 if no error has occurred
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ERL takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(self.get_error_line())
            }
            "ERR" => {
                // Error number - returns 0 if no error has occurred
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ERR takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(self.get_error_number())
            }
            "OPENIN" => {
                // Open file for reading, returns file handle
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "OPENIN requires 1 argument (filename)".to_string(),
                        line: None,
                    });
                }
                let filename = self.eval_string(&args[0])?;
                self.open_file_for_reading(&filename)
            }
            "OPENOUT" => {
                // Open file for writing, returns file handle
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "OPENOUT requires 1 argument (filename)".to_string(),
                        line: None,
                    });
                }
                let filename = self.eval_string(&args[0])?;
                self.open_file_for_writing(&filename)
            }
            "EOF" => {
                // Test for end of file, returns -1 (TRUE) if EOF, 0 (FALSE) otherwise
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "EOF requires 1 argument (file handle)".to_string(),
                        line: None,
                    });
                }
                let handle = self.eval_integer(&args[0])?;
                self.check_eof(handle)
            }
            "TRUE" => {
                // TRUE constant = -1 (BBC BASIC convention)
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "TRUE takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(-1)
            }
            "FALSE" => {
                // FALSE constant = 0 (BBC BASIC convention)
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "FALSE takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(0)
            }
            "INSTR" => {
                // INSTR(haystack, needle) or INSTR(haystack, needle, start)
                // Returns 1-based position of needle in haystack, or 0 if not found
                if args.len() < 2 || args.len() > 3 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "INSTR requires 2 or 3 arguments".to_string(),
                        line: None,
                    });
                }

                let haystack = self.eval_string(&args[0])?;
                let needle = self.eval_string(&args[1])?;

                let start_pos = if args.len() == 3 {
                    let start = self.eval_integer(&args[2])? as usize;
                    // BBC BASIC uses 1-based indexing
                    if start < 1 {
                        return Ok(0);
                    }
                    start - 1
                } else {
                    0
                };

                // Search for needle in haystack starting from start_pos
                if let Some(pos) = haystack[start_pos.min(haystack.len())..].find(&needle) {
                    // Return 1-based position relative to start of string
                    Ok((start_pos + pos + 1) as i32)
                } else {
                    Ok(0)
                }
            }
            // Real-only functions should not be called as integers
            "SIN" | "COS" | "TAN" | "ATN" | "SQR" | "SQRT" | "ACS" | "ASN" | "EXP" | "LN" | "LOG"
            | "DEG" | "RAD" | "PI" | "RND" => Err(BBCBasicError::TypeMismatch),
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown function: {}", name),
                line: None,
            }),
        }
    }

    /// Evaluate a function call returning a real number
    fn eval_function_real(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        // Check if this is a user-defined function first
        if self.functions.contains_key(name) {
            return self.call_function_real(name, args);
        }

        // Otherwise, it's a built-in function
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
                // BBC BASIC behavior:
                // - RND(1) returns random float in range [0, 1)
                // - RND(n) where n > 1 returns random integer in range [1, n]
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "RND requires exactly 1 argument".to_string(),
                        line: None,
                    });
                }

                let arg_value = self.eval_real(&args[0])?;

                if (arg_value - 1.0).abs() < 0.0001 {
                    // RND(1) - return random float [0, 1)
                    Ok(self.rng.borrow_mut().gen::<f64>())
                } else if arg_value > 1.0 {
                    // RND(n) - return random integer [1, n]
                    let n = arg_value as i32;
                    let random_int = self.rng.borrow_mut().gen_range(1..=n);
                    Ok(random_int as f64)
                } else {
                    // For other values, BBC BASIC behavior is undefined
                    // We'll return random [0, 1) as a sensible default
                    Ok(self.rng.borrow_mut().gen::<f64>())
                }
            }
            "VAL" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "VAL requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                s.trim().parse::<f64>().or_else(|_| Ok(0.0)) // BBC BASIC returns 0 for non-numeric strings
            }
            "SQRT" => {
                // SQRT is an alias for SQR in BBC BASIC
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "SQRT requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                if val < 0.0 {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.sqrt())
            }
            "ACS" => {
                // ACS(x) = arccosine in radians
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ACS requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                if val < -1.0 || val > 1.0 {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.acos())
            }
            "ASN" => {
                // ASN(x) = arcsine in radians
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ASN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let val = self.eval_real(&args[0])?;
                if val < -1.0 || val > 1.0 {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.asin())
            }
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown function: {}", name),
                line: None,
            }),
        }
    }

    /// Evaluate a function call returning a string
    fn eval_function_string(&mut self, name: &str, args: &[Expression]) -> Result<String> {
        // Check if this is a user-defined function first
        if self.functions.contains_key(name) {
            return self.call_function_string(name, args);
        }

        // Otherwise, it's a built-in function
        match name {
            "CHR$" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "CHR$ requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let code = self.eval_integer(&args[0])?;
                if code < 0 || code > 255 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "CHR$ argument must be 0-255".to_string(),
                        line: None,
                    });
                }
                Ok((code as u8 as char).to_string())
            }
            "LEFT$" => {
                if args.len() != 2 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "LEFT$ requires 2 arguments".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                let n = self.eval_integer(&args[1])? as usize;
                Ok(s.chars().take(n).collect())
            }
            "RIGHT$" => {
                if args.len() != 2 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "RIGHT$ requires 2 arguments".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                let n = self.eval_integer(&args[1])? as usize;
                let len = s.chars().count();
                if n >= len {
                    Ok(s)
                } else {
                    Ok(s.chars().skip(len - n).collect())
                }
            }
            "MID$" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "MID$ requires 2 or 3 arguments".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                let start = self.eval_integer(&args[1])? as usize;

                // BBC BASIC uses 1-based indexing
                if start < 1 {
                    return Ok(String::new());
                }

                let chars: Vec<char> = s.chars().collect();
                let start_idx = start - 1;

                if start_idx >= chars.len() {
                    return Ok(String::new());
                }

                if args.len() == 3 {
                    let len = self.eval_integer(&args[2])? as usize;
                    Ok(chars.iter().skip(start_idx).take(len).collect())
                } else {
                    // If length not specified, take rest of string
                    Ok(chars.iter().skip(start_idx).collect())
                }
            }
            "STR$" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "STR$ requires 1 argument".to_string(),
                        line: None,
                    });
                }
                // Check if the expression is explicitly a Real or contains decimal point
                match &args[0] {
                    Expression::Real(val) => Ok(val.to_string()),
                    Expression::Integer(val) => Ok(val.to_string()),
                    _ => {
                        // Try to evaluate - prefer real if it works
                        if let Ok(real_val) = self.eval_real(&args[0]) {
                            // Check if it's actually an integer value
                            if real_val.fract() == 0.0 {
                                Ok((real_val as i32).to_string())
                            } else {
                                Ok(real_val.to_string())
                            }
                        } else if let Ok(int_val) = self.eval_integer(&args[0]) {
                            Ok(int_val.to_string())
                        } else {
                            Err(BBCBasicError::TypeMismatch)
                        }
                    }
                }
            }
            "UPPER$" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "UPPER$ requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                Ok(s.to_uppercase())
            }
            "LOWER$" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "LOWER$ requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let s = self.eval_string(&args[0])?;
                Ok(s.to_lowercase())
            }
            "STRING$" => {
                if args.len() != 2 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "STRING$ requires 2 arguments (count, string)".to_string(),
                        line: None,
                    });
                }
                let count = self.eval_integer(&args[0])? as usize;
                let s = self.eval_string(&args[1])?;

                // BBC BASIC STRING$(n, string) repeats first character n times
                if let Some(first_char) = s.chars().next() {
                    Ok(first_char.to_string().repeat(count))
                } else {
                    Ok(String::new())
                }
            }
            "REPORT$" => {
                // REPORT$ returns the last error message (empty string if no error)
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "REPORT$ takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(self.last_error.as_ref().map(|e| e.message.clone()).unwrap_or_default())
            }
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown string function: {}", name),
                line: None,
            }),
        }
    }

    /// Get a variable value (for testing)
    #[cfg(test)]
    pub fn get_variable_int(&self, name: &str) -> Result<i32> {
        self.variables
            .get_integer_var(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(name.to_string()))
    }

    #[cfg(test)]
    pub fn get_variable_real(&self, name: &str) -> Result<f64> {
        self.variables
            .get_real_var(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(name.to_string()))
    }

    #[cfg(test)]
    pub fn get_variable_string(&self, name: &str) -> Result<String> {
        self.variables
            .get_string_var(name)
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

    /// Push a REPEAT line number onto the repeat stack
    pub fn push_repeat(&mut self, line_number: u16) {
        self.repeat_stack.push(line_number);
    }

    /// Evaluate UNTIL condition and return the REPEAT line if we should loop back
    pub fn check_until(&mut self, condition: &Expression) -> Result<Option<u16>> {
        // Evaluate the condition
        let result = self.eval_integer(condition)?;

        if result == 0 {
            // Condition is false - loop back to REPEAT
            // Return the REPEAT line number but keep it on stack (don't pop yet)
            Ok(self.repeat_stack.last().copied())
        } else {
            // Condition is true - exit loop
            self.repeat_stack.pop();
            Ok(None)
        }
    }

    /// Push a WHILE line number onto the while stack and check condition
    /// Returns Some(line_number) if condition is TRUE (continue to loop body)
    /// Returns None if condition is FALSE (skip loop body)
    pub fn push_while(&mut self, line_number: u16, condition: &Expression) -> Result<Option<u16>> {
        // Evaluate the condition
        let result = self.eval_integer(condition)?;

        if result != 0 {
            // Condition is true - enter loop body
            self.while_stack.push(line_number);
            Ok(Some(line_number))
        } else {
            // Condition is false - skip loop body
            Ok(None)
        }
    }

    /// Handle ENDWHILE - return the WHILE line if we should loop back
    pub fn check_endwhile(&mut self, condition: &Expression) -> Result<Option<u16>> {
        // Evaluate the condition
        let result = self.eval_integer(condition)?;

        if result != 0 {
            // Condition is still true - loop back to WHILE
            // Return the WHILE line number but keep it on stack (don't pop yet)
            Ok(self.while_stack.last().copied())
        } else {
            // Condition is false - exit loop
            self.while_stack.pop();
            Ok(None)
        }
    }

    /// Get the current WHILE line number without popping (for ENDWHILE to retrieve condition)
    pub fn check_endwhile_get_while_line(&self) -> Option<u16> {
        self.while_stack.last().copied()
    }

    /// Push a return address onto the GOSUB stack
    pub fn push_gosub_return(&mut self, line_number: u16) {
        self.return_stack.push(line_number);
    }

    /// Pop a return address from the GOSUB stack
    pub fn pop_gosub_return(&mut self) -> Result<u16> {
        self.return_stack.pop().ok_or(BBCBasicError::BadCall)
    }

    /// Define a procedure
    pub fn define_procedure(&mut self, name: String, line_number: u16, params: Vec<String>) {
        self.procedures.insert(
            name,
            ProcedureDefinition {
                line_number,
                params,
            },
        );
    }

    /// Get procedure definition
    pub fn get_procedure(&self, name: &str) -> Option<&ProcedureDefinition> {
        self.procedures.get(name)
    }

    /// Enter a new local scope (called on PROC/FN entry)
    pub fn enter_local_scope(&mut self) {
        self.local_stack.push(LocalFrame::new());
    }

    /// Declare a local variable (called on LOCAL statement)
    pub fn declare_local(&mut self, name: &str) -> Result<()> {
        let frame = self
            .local_stack
            .last_mut()
            .ok_or_else(|| BBCBasicError::SyntaxError {
                message: "LOCAL outside of procedure".to_string(),
                line: None,
            })?;

        // Save current value (or None if doesn't exist)
        let current_value = self.variables.get_variable(name).cloned();
        frame
            .saved_variables
            .insert(name.to_string(), current_value);

        // Remove the variable from the main scope (creating a new local binding)
        // We'll set it to a default value for its type
        if name.ends_with('%') {
            self.variables.set_integer_var(name.to_string(), 0);
        } else if name.ends_with('$') {
            self.variables
                .set_string_var(name.to_string(), String::new())?;
        } else {
            self.variables.set_real_var(name.to_string(), 0.0);
        }

        Ok(())
    }

    /// Exit local scope and restore saved variables (called on ENDPROC/ENDFN)
    pub fn exit_local_scope(&mut self) -> Result<()> {
        let frame = self
            .local_stack
            .pop()
            .ok_or_else(|| BBCBasicError::SyntaxError {
                message: "No local scope to exit".to_string(),
                line: None,
            })?;

        // Restore all saved variables
        for (name, saved_value) in frame.saved_variables {
            match saved_value {
                Some(var) => {
                    // Restore previous value
                    match var {
                        Variable::Integer(v) => self.variables.set_integer_var(name, v),
                        Variable::Real(v) => self.variables.set_real_var(name, v),
                        Variable::String(v) => {
                            let _ = self.variables.set_string_var(name, v);
                        }
                        Variable::IntegerArray { .. }
                        | Variable::RealArray { .. }
                        | Variable::StringArray { .. } => {
                            // For arrays, we need to restore them via dim_array
                            // This is complex, so for now we'll just leave them
                            // TODO: Proper array restoration
                        }
                    }
                }
                None => {
                    // Variable didn't exist before - ideally we'd remove it
                    // For now, just leave it (BBC BASIC allows this)
                }
            }
        }

        Ok(())
    }

    /// Execute LOCAL statement
    fn execute_local(&mut self, variables: &[String]) -> Result<()> {
        for var in variables {
            self.declare_local(var)?;
        }
        Ok(())
    }

    /// Helper method for tests: set integer variable
    #[cfg(test)]
    pub fn set_variable_int(&mut self, name: &str, value: i32) {
        self.variables.set_integer_var(name.to_string(), value);
    }

    /// Clear all procedure definitions (used when loading new program)
    pub fn clear_procedures(&mut self) {
        self.procedures.clear();
    }

    /// Set error handler (ON ERROR GOTO line)
    pub fn set_error_handler(&mut self, line_number: u16) {
        self.error_handler = Some(line_number);
    }

    /// Clear error handler (ON ERROR OFF)
    pub fn clear_error_handler(&mut self) {
        self.error_handler = None;
    }

    /// Get error handler line number (returns None if no handler set)
    pub fn get_error_handler(&self) -> Option<u16> {
        self.error_handler
    }

    /// Set last error information
    pub fn set_last_error(&mut self, error_number: i32, error_line: u16, message: String) {
        self.last_error = Some(ErrorInfo {
            error_number,
            error_line,
            message,
        });
    }

    /// Get error line number (ERL)
    pub fn get_error_line(&self) -> i32 {
        self.last_error
            .as_ref()
            .map(|e| e.error_line as i32)
            .unwrap_or(0)
    }

    /// Get error number (ERR)
    pub fn get_error_number(&self) -> i32 {
        self.last_error
            .as_ref()
            .map(|e| e.error_number)
            .unwrap_or(0)
    }

    /// Execute DEF FN statement - define a function
    fn execute_def_fn(
        &mut self,
        name: &str,
        params: &[String],
        expression: &Expression,
    ) -> Result<()> {
        self.functions.insert(
            name.to_string(),
            FunctionDefinition {
                params: params.to_vec(),
                expression: expression.clone(),
            },
        );
        Ok(())
    }

    /// Call a function and return integer result
    fn call_function_int(&mut self, name: &str, args: &[Expression]) -> Result<i32> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(format!("Function {} not defined", name)))?
            .clone();

        // Check parameter count
        if args.len() != func.params.len() {
            return Err(BBCBasicError::SyntaxError {
                message: format!(
                    "Function {} expects {} parameters, got {}",
                    name,
                    func.params.len(),
                    args.len()
                ),
                line: None,
            });
        }

        // Enter local scope for function
        self.enter_local_scope();

        // Evaluate arguments and bind to parameters
        for (param_name, arg_expr) in func.params.iter().zip(args.iter()) {
            // Declare parameter as local
            self.declare_local(param_name)?;

            // Evaluate argument and assign to parameter
            if param_name.ends_with('%') {
                let value = self.eval_integer(arg_expr)?;
                self.variables.set_integer_var(param_name.clone(), value);
            } else if param_name.ends_with('$') {
                let value = self.eval_string(arg_expr)?;
                self.variables.set_string_var(param_name.clone(), value)?;
            } else {
                let value = self.eval_real(arg_expr)?;
                self.variables.set_real_var(param_name.clone(), value);
            }
        }

        // Evaluate function expression
        let result = self.eval_integer(&func.expression)?;

        // Exit local scope (restore variables)
        self.exit_local_scope()?;

        Ok(result)
    }

    /// Call a function and return real result
    fn call_function_real(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(format!("Function {} not defined", name)))?
            .clone();

        // Check parameter count
        if args.len() != func.params.len() {
            return Err(BBCBasicError::SyntaxError {
                message: format!(
                    "Function {} expects {} parameters, got {}",
                    name,
                    func.params.len(),
                    args.len()
                ),
                line: None,
            });
        }

        // Enter local scope for function
        self.enter_local_scope();

        // Evaluate arguments and bind to parameters
        for (param_name, arg_expr) in func.params.iter().zip(args.iter()) {
            // Declare parameter as local
            self.declare_local(param_name)?;

            // Evaluate argument and assign to parameter
            if param_name.ends_with('%') {
                let value = self.eval_integer(arg_expr)?;
                self.variables.set_integer_var(param_name.clone(), value);
            } else if param_name.ends_with('$') {
                let value = self.eval_string(arg_expr)?;
                self.variables.set_string_var(param_name.clone(), value)?;
            } else {
                let value = self.eval_real(arg_expr)?;
                self.variables.set_real_var(param_name.clone(), value);
            }
        }

        // Evaluate function expression
        let result = self.eval_real(&func.expression)?;

        // Exit local scope (restore variables)
        self.exit_local_scope()?;

        Ok(result)
    }

    /// Call a function and return string result
    fn call_function_string(&mut self, name: &str, args: &[Expression]) -> Result<String> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(format!("Function {} not defined", name)))?
            .clone();

        // Check parameter count
        if args.len() != func.params.len() {
            return Err(BBCBasicError::SyntaxError {
                message: format!(
                    "Function {} expects {} parameters, got {}",
                    name,
                    func.params.len(),
                    args.len()
                ),
                line: None,
            });
        }

        // Enter local scope for function
        self.enter_local_scope();

        // Evaluate arguments and bind to parameters
        for (param_name, arg_expr) in func.params.iter().zip(args.iter()) {
            // Declare parameter as local
            self.declare_local(param_name)?;

            // Evaluate argument and assign to parameter
            if param_name.ends_with('%') {
                let value = self.eval_integer(arg_expr)?;
                self.variables.set_integer_var(param_name.clone(), value);
            } else if param_name.ends_with('$') {
                let value = self.eval_string(arg_expr)?;
                self.variables.set_string_var(param_name.clone(), value)?;
            } else {
                let value = self.eval_real(arg_expr)?;
                self.variables.set_real_var(param_name.clone(), value);
            }
        }

        // Evaluate function expression
        let result = self.eval_string(&func.expression)?;

        // Exit local scope (restore variables)
        self.exit_local_scope()?;

        Ok(result)
    }

    /// Open a file for reading (OPENIN)
    fn open_file_for_reading(&mut self, filename: &str) -> Result<i32> {
        // Check if we've hit the maximum number of open files (BBC BASIC limit: ~255)
        if self.open_files.len() >= 255 {
            return Err(BBCBasicError::TooManyOpenFiles);
        }

        // Try to open the file
        let file = File::open(filename)
            .map_err(|_| BBCBasicError::FileNotFound(filename.to_string()))?;
        let reader = BufReader::new(file);

        // Allocate a handle
        let handle = self.next_file_handle;
        self.next_file_handle += 1;

        // Store the file handle
        self.open_files.insert(handle, FileHandle::Input(reader));

        Ok(handle)
    }

    /// Open a file for writing (OPENOUT)
    fn open_file_for_writing(&mut self, filename: &str) -> Result<i32> {
        // Check if we've hit the maximum number of open files
        if self.open_files.len() >= 255 {
            return Err(BBCBasicError::TooManyOpenFiles);
        }

        // Try to create/truncate the file
        let file = File::create(filename)
            .map_err(|e| BBCBasicError::DiskError(format!("Cannot create file: {}", e)))?;
        let writer = BufWriter::new(file);

        // Allocate a handle
        let handle = self.next_file_handle;
        self.next_file_handle += 1;

        // Store the file handle
        self.open_files.insert(handle, FileHandle::Output(writer));

        Ok(handle)
    }

    /// Check if file is at end of file (EOF#)
    fn check_eof(&mut self, handle: i32) -> Result<i32> {
        // Get the file handle
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Only input files can be tested for EOF
        match file_handle {
            FileHandle::Input(reader) => {
                // Check if we're at EOF by trying to fill the buffer
                // BBC BASIC EOF# returns -1 (TRUE) if at EOF, 0 (FALSE) otherwise
                Ok(if reader.fill_buf().map(|b| b.is_empty()).unwrap_or(true) {
                    -1 // TRUE in BBC BASIC
                } else {
                    0 // FALSE in BBC BASIC
                })
            }
            FileHandle::Output(_) => {
                // Can't check EOF on output files
                Err(BBCBasicError::BadCall)
            }
        }
    }

    /// Execute PRINT# statement - write to file
    fn execute_print_file(&mut self, handle_expr: &Expression, items: &[crate::parser::PrintItem]) -> Result<()> {
        // Evaluate the handle
        let handle = self.eval_integer(handle_expr)?;

        // Process print items and build output string first (to avoid borrow issues)
        use crate::parser::PrintItem;
        let mut output = String::new();
        
        for item in items {
            match item {
                PrintItem::Expression(expr) => {
                    output.push_str(&self.format_expression(expr)?);
                }
                PrintItem::Tab(_) | PrintItem::Spc(_) => {
                    // TAB and SPC not typically used in file I/O, but we can support them
                    // For simplicity, ignore them in file output
                }
                PrintItem::Semicolon => {
                    // Semicolon suppresses newline - do nothing
                }
                PrintItem::Comma => {
                    // Comma outputs a tab
                    output.push('\t');
                }
            }
        }

        // Add newline unless last item was semicolon
        if !items.is_empty() {
            if let Some(last) = items.last() {
                if !matches!(last, PrintItem::Semicolon) {
                    output.push('\n');
                }
            }
        } else {
            output.push('\n');
        }

        // Now get the file handle and write
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Only output files can be written to
        let writer = match file_handle {
            FileHandle::Output(writer) => writer,
            FileHandle::Input(_) => return Err(BBCBasicError::BadCall),
        };

        // Write the output
        write!(writer, "{}", output)
            .map_err(|e| BBCBasicError::DiskError(format!("Write error: {}", e)))?;

        // Flush to ensure data is written
        writer
            .flush()
            .map_err(|e| BBCBasicError::DiskError(format!("Flush error: {}", e)))?;

        Ok(())
    }

    /// Execute INPUT# statement - read from file
    fn execute_input_file(&mut self, handle_expr: &Expression, variables: &[String]) -> Result<()> {
        // Evaluate the handle
        let handle = self.eval_integer(handle_expr)?;

        // Get the file handle
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Only input files can be read from
        let reader = match file_handle {
            FileHandle::Input(reader) => reader,
            FileHandle::Output(_) => return Err(BBCBasicError::BadCall),
        };

        // Read a line from the file
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| BBCBasicError::DiskError(format!("Read error: {}", e)))?;

        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        // Parse the line and assign to variables (simplified - just split by commas)
        let values: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

        for (i, var_name) in variables.iter().enumerate() {
            let value_str = values.get(i).unwrap_or(&"");

            // Assign based on variable type
            if var_name.ends_with('%') {
                // Integer variable
                let value = value_str.parse::<i32>().unwrap_or(0);
                self.variables.set_integer_var(var_name.clone(), value);
            } else if var_name.ends_with('$') {
                // String variable
                self.variables.set_string_var(var_name.clone(), value_str.to_string())?;
            } else {
                // Real variable
                let value = value_str.parse::<f64>().unwrap_or(0.0);
                self.variables.set_real_var(var_name.clone(), value);
            }
        }

        Ok(())
    }

    /// Execute CLOSE# statement - close file
    fn execute_close_file(&mut self, handle_expr: &Expression) -> Result<()> {
        // Evaluate the handle
        let handle = self.eval_integer(handle_expr)?;

        // Remove the file handle (this closes the file)
        self.open_files
            .remove(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        Ok(())
    }

    /// BGET# function - Read a single byte from file
    /// Returns the byte value (0-255) or -1 at EOF
    pub fn bget(&mut self, handle: i32) -> Result<i32> {
        use std::io::Read;

        // Get the file handle
        let file_handle = self.open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // BGET# only works on input files
        match file_handle {
            FileHandle::Input(reader) => {
                // Read a single byte
                let mut buf = [0u8; 1];
                match reader.read_exact(&mut buf) {
                    Ok(_) => Ok(buf[0] as i32),
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        // EOF reached - return -1
                        Ok(-1)
                    }
                    Err(e) => Err(BBCBasicError::DiskError(e.to_string())),
                }
            }
            FileHandle::Output(_) => {
                Err(BBCBasicError::TypeMismatch) // Cannot read from output file
            }
        }
    }

    /// BPUT# statement - Write a single byte to file
    /// Values > 255 are wrapped using MOD 256
    pub fn bput(&mut self, handle: i32, value: i32) -> Result<()> {
        use std::io::Write;

        // Get the file handle
        let file_handle = self.open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // BPUT# only works on output files
        match file_handle {
            FileHandle::Output(writer) => {
                // Convert value to byte (MOD 256)
                let byte = (value % 256) as u8;

                // Write the byte
                writer.write_all(&[byte])
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Flush to ensure byte is written
                writer.flush()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                Ok(())
            }
            FileHandle::Input(_) => {
                Err(BBCBasicError::TypeMismatch) // Cannot write to input file
            }
        }
    }

    /// PTR# function - Get current file position
    pub fn get_ptr(&mut self, handle: i32) -> Result<i32> {
        use std::io::Seek;

        // Get the file handle
        let file_handle = self.open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Get current position from the underlying file
        match file_handle {
            FileHandle::Input(reader) => {
                let pos = reader.stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(pos as i32)
            }
            FileHandle::Output(writer) => {
                let pos = writer.stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(pos as i32)
            }
        }
    }

    /// PTR# assignment - Set file position
    pub fn set_ptr(&mut self, handle: i32, position: i32) -> Result<()> {
        use std::io::Seek;

        // Get the file handle
        let file_handle = self.open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Seek to the specified position
        let pos = position as u64;
        match file_handle {
            FileHandle::Input(reader) => {
                reader.seek(std::io::SeekFrom::Start(pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(())
            }
            FileHandle::Output(writer) => {
                writer.seek(std::io::SeekFrom::Start(pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(())
            }
        }
    }

    /// EXT# function - Get file size
    pub fn get_ext(&mut self, handle: i32) -> Result<i32> {
        use std::io::Seek;

        // Get the file handle
        let file_handle = self.open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Get file size by seeking to end
        match file_handle {
            FileHandle::Input(reader) => {
                // Save current position
                let current_pos = reader.stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Seek to end to get size
                let size = reader.seek(std::io::SeekFrom::End(0))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Restore original position
                reader.seek(std::io::SeekFrom::Start(current_pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                Ok(size as i32)
            }
            FileHandle::Output(writer) => {
                // Save current position
                let current_pos = writer.stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Seek to end to get size
                let size = writer.seek(std::io::SeekFrom::End(0))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Restore original position
                writer.seek(std::io::SeekFrom::Start(current_pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                Ok(size as i32)
            }
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
    use crate::parser::BinaryOperator;

    #[test]
    fn test_executor_creation() {
        // RED: Test creating an executor
        let mut executor = Executor::new();
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

        let mut executor = Executor::new();
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
            items: vec![PrintItem::Expression(Expression::String(
                "HELLO".to_string(),
            ))],
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
            items: vec![PrintItem::Expression(Expression::Variable(
                "A%".to_string(),
            ))],
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

    // OLD TESTS REMOVED: test_gosub_return and test_return_without_gosub
    // GOSUB/RETURN are now handled as control flow in main.rs, not in executor
    // New tests: test_gosub_return_call_stack and test_gosub_return_nested

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
        assert_eq!(executor.for_loops[0].1, 10); // end value
        assert_eq!(executor.for_loops[0].2, 1); // step value
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
        assert_eq!(executor.for_loops[0].2, -1); // step value
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
        assert_eq!(executor.for_loops.len(), 1); // Loop still active

        // Second NEXT: I% should become 3
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 3);
        assert_eq!(executor.for_loops.len(), 1); // Loop still active

        // Third NEXT: I% should become 4, loop should complete
        executor.execute_statement(&next_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 4);
        assert_eq!(executor.for_loops.len(), 0); // Loop completed and removed
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
            variables: vec!["A%".to_string(), "B$".to_string(), "C".to_string()],
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
            arrays: vec![("A%".to_string(), vec![Expression::Integer(10)])],
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
    fn test_tan_function() {
        // RED: Test TAN(45) ≈ 1.0 (BBC BASIC uses degrees)
        let mut executor = Executor::new();

        let tan_45 = Expression::FunctionCall {
            name: "TAN".to_string(),
            args: vec![Expression::Real(45.0)],
        };

        let result = executor.eval_real(&tan_45).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_atn_function() {
        // RED: Test ATN(1) = 45 degrees
        let mut executor = Executor::new();

        let atn_expr = Expression::FunctionCall {
            name: "ATN".to_string(),
            args: vec![Expression::Real(1.0)],
        };

        let result = executor.eval_real(&atn_expr).unwrap();
        assert!((result - 45.0).abs() < 0.0001);
    }

    #[test]
    fn test_log_function() {
        // RED: Test LOG(100) = 2 (base 10 logarithm)
        let mut executor = Executor::new();

        let log_100 = Expression::FunctionCall {
            name: "LOG".to_string(),
            args: vec![Expression::Real(100.0)],
        };

        let result = executor.eval_real(&log_100).unwrap();
        assert!((result - 2.0).abs() < 0.0001);
    }

    #[test]
    fn test_ln_function() {
        // RED: Test LN(E) ≈ 1.0 (natural logarithm)
        let mut executor = Executor::new();

        let e = std::f64::consts::E;
        let ln_e = Expression::FunctionCall {
            name: "LN".to_string(),
            args: vec![Expression::Real(e)],
        };

        let result = executor.eval_real(&ln_e).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_exp_function() {
        // RED: Test EXP(1) ≈ E (e^1)
        let mut executor = Executor::new();

        let exp_1 = Expression::FunctionCall {
            name: "EXP".to_string(),
            args: vec![Expression::Real(1.0)],
        };

        let result = executor.eval_real(&exp_1).unwrap();
        let e = std::f64::consts::E;
        assert!((result - e).abs() < 0.0001);
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
    fn test_sqrt_asc_asn_functions() {
        // RED: Test SQRT (alias for SQR), ACS (arccosine), ASN (arcsine)
        let mut executor = Executor::new();

        // Test SQRT(16) = 4 (alias for SQR)
        let sqrt_expr = Expression::FunctionCall {
            name: "SQRT".to_string(),
            args: vec![Expression::Integer(16)],
        };
        let result = executor.eval_real(&sqrt_expr).unwrap();
        assert!((result - 4.0).abs() < 0.0001);

        // Test ACS(0.5) ≈ 1.047 radians (60 degrees)
        let acs_expr = Expression::FunctionCall {
            name: "ACS".to_string(),
            args: vec![Expression::Real(0.5)],
        };
        let result = executor.eval_real(&acs_expr).unwrap();
        assert!((result - 1.047).abs() < 0.001);

        // Test ASN(0.5) ≈ 0.524 radians (30 degrees)
        let asn_expr = Expression::FunctionCall {
            name: "ASN".to_string(),
            args: vec![Expression::Real(0.5)],
        };
        let result = executor.eval_real(&asn_expr).unwrap();
        assert!((result - 0.524).abs() < 0.001);

        // Test ACS(1) = 0 (cos(0) = 1)
        let acs_one = Expression::FunctionCall {
            name: "ACS".to_string(),
            args: vec![Expression::Integer(1)],
        };
        let result = executor.eval_real(&acs_one).unwrap();
        assert!((result - 0.0).abs() < 0.0001);

        // Test ASN(0) = 0 (sin(0) = 0)
        let asn_zero = Expression::FunctionCall {
            name: "ASN".to_string(),
            args: vec![Expression::Integer(0)],
        };
        let result = executor.eval_real(&asn_zero).unwrap();
        assert!((result - 0.0).abs() < 0.0001);
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
    fn test_true_false_constants() {
        // RED: Test TRUE = -1, FALSE = 0 (BBC BASIC convention)
        let mut executor = Executor::new();

        let true_expr = Expression::FunctionCall {
            name: "TRUE".to_string(),
            args: vec![],
        };

        let false_expr = Expression::FunctionCall {
            name: "FALSE".to_string(),
            args: vec![],
        };

        assert_eq!(executor.eval_integer(&true_expr).unwrap(), -1);
        assert_eq!(executor.eval_integer(&false_expr).unwrap(), 0);
    }

    #[test]
    fn test_bitwise_shift_operators() {
        // RED: Test << (left shift) and >> (right shift) operators
        let mut executor = Executor::new();

        // Test 5 << 2 = 20 (5 * 4)
        let left_shift = Expression::BinaryOp {
            op: BinaryOperator::LeftShift,
            left: Box::new(Expression::Integer(5)),
            right: Box::new(Expression::Integer(2)),
        };
        assert_eq!(executor.eval_integer(&left_shift).unwrap(), 20);

        // Test 20 >> 2 = 5 (20 / 4)
        let right_shift = Expression::BinaryOp {
            op: BinaryOperator::RightShift,
            left: Box::new(Expression::Integer(20)),
            right: Box::new(Expression::Integer(2)),
        };
        assert_eq!(executor.eval_integer(&right_shift).unwrap(), 5);

        // Test 1 << 8 = 256
        let left_shift_large = Expression::BinaryOp {
            op: BinaryOperator::LeftShift,
            left: Box::new(Expression::Integer(1)),
            right: Box::new(Expression::Integer(8)),
        };
        assert_eq!(executor.eval_integer(&left_shift_large).unwrap(), 256);

        // Test -8 >> 2 = -2 (arithmetic right shift preserves sign)
        let right_shift_negative = Expression::BinaryOp {
            op: BinaryOperator::RightShift,
            left: Box::new(Expression::Integer(-8)),
            right: Box::new(Expression::Integer(2)),
        };
        assert_eq!(executor.eval_integer(&right_shift_negative).unwrap(), -2);
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

    #[test]
    fn test_chr_function() {
        // RED: Test CHR$(65) = "A", CHR$(42) = "*"
        let mut executor = Executor::new();

        let chr_a = Expression::FunctionCall {
            name: "CHR$".to_string(),
            args: vec![Expression::Integer(65)],
        };

        let result = executor.eval_string(&chr_a).unwrap();
        assert_eq!(result, "A");

        let chr_star = Expression::FunctionCall {
            name: "CHR$".to_string(),
            args: vec![Expression::Integer(42)],
        };

        let result = executor.eval_string(&chr_star).unwrap();
        assert_eq!(result, "*");
    }

    #[test]
    fn test_asc_function() {
        // RED: Test ASC("A") = 65, ASC("Hello") = 72
        let mut executor = Executor::new();

        let asc_a = Expression::FunctionCall {
            name: "ASC".to_string(),
            args: vec![Expression::String("A".to_string())],
        };

        let result = executor.eval_integer(&asc_a).unwrap();
        assert_eq!(result, 65);

        let asc_hello = Expression::FunctionCall {
            name: "ASC".to_string(),
            args: vec![Expression::String("Hello".to_string())],
        };

        let result = executor.eval_integer(&asc_hello).unwrap();
        assert_eq!(result, 72); // 'H'
    }

    #[test]
    fn test_len_function() {
        // RED: Test LEN("Hello") = 5, LEN("") = 0
        let mut executor = Executor::new();

        let len_hello = Expression::FunctionCall {
            name: "LEN".to_string(),
            args: vec![Expression::String("Hello".to_string())],
        };

        let result = executor.eval_integer(&len_hello).unwrap();
        assert_eq!(result, 5);

        let len_empty = Expression::FunctionCall {
            name: "LEN".to_string(),
            args: vec![Expression::String("".to_string())],
        };

        let result = executor.eval_integer(&len_empty).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_left_function() {
        // RED: Test LEFT$("Hello", 3) = "Hel"
        let mut executor = Executor::new();

        let left_expr = Expression::FunctionCall {
            name: "LEFT$".to_string(),
            args: vec![
                Expression::String("Hello".to_string()),
                Expression::Integer(3),
            ],
        };

        let result = executor.eval_string(&left_expr).unwrap();
        assert_eq!(result, "Hel");
    }

    #[test]
    fn test_right_function() {
        // RED: Test RIGHT$("Hello", 3) = "llo"
        let mut executor = Executor::new();

        let right_expr = Expression::FunctionCall {
            name: "RIGHT$".to_string(),
            args: vec![
                Expression::String("Hello".to_string()),
                Expression::Integer(3),
            ],
        };

        let result = executor.eval_string(&right_expr).unwrap();
        assert_eq!(result, "llo");
    }

    #[test]
    fn test_mid_function() {
        // RED: Test MID$("Hello", 2, 3) = "ell" (1-based indexing in BBC BASIC)
        let mut executor = Executor::new();

        let mid_expr = Expression::FunctionCall {
            name: "MID$".to_string(),
            args: vec![
                Expression::String("Hello".to_string()),
                Expression::Integer(2),
                Expression::Integer(3),
            ],
        };

        let result = executor.eval_string(&mid_expr).unwrap();
        assert_eq!(result, "ell");
    }

    #[test]
    fn test_str_function() {
        // RED: Test STR$(42) = "42", STR$(3.14) = "3.14"
        let mut executor = Executor::new();

        let str_int = Expression::FunctionCall {
            name: "STR$".to_string(),
            args: vec![Expression::Integer(42)],
        };

        let result = executor.eval_string(&str_int).unwrap();
        assert_eq!(result, "42");

        let str_real = Expression::FunctionCall {
            name: "STR$".to_string(),
            args: vec![Expression::Real(3.14)],
        };

        let result = executor.eval_string(&str_real).unwrap();
        assert_eq!(result, "3.14");
    }

    #[test]
    fn test_val_function() {
        // RED: Test VAL("42") = 42, VAL("3.14") = 3.14
        let mut executor = Executor::new();

        let val_int = Expression::FunctionCall {
            name: "VAL".to_string(),
            args: vec![Expression::String("42".to_string())],
        };

        let result = executor.eval_integer(&val_int).unwrap();
        assert_eq!(result, 42);

        let val_real = Expression::FunctionCall {
            name: "VAL".to_string(),
            args: vec![Expression::String("3.14".to_string())],
        };

        let result = executor.eval_real(&val_real).unwrap();
        assert!((result - 3.14).abs() < 0.0001);
    }

    #[test]
    fn test_upper_lower_string_functions() {
        // RED: Test UPPER$, LOWER$, and STRING$ functions
        let mut executor = Executor::new();

        // Test UPPER$("hello") = "HELLO"
        let upper_expr = Expression::FunctionCall {
            name: "UPPER$".to_string(),
            args: vec![Expression::String("hello".to_string())],
        };
        assert_eq!(executor.eval_string(&upper_expr).unwrap(), "HELLO");

        // Test LOWER$("WORLD") = "world"
        let lower_expr = Expression::FunctionCall {
            name: "LOWER$".to_string(),
            args: vec![Expression::String("WORLD".to_string())],
        };
        assert_eq!(executor.eval_string(&lower_expr).unwrap(), "world");

        // Test STRING$(5, "*") = "*****"
        let string_expr = Expression::FunctionCall {
            name: "STRING$".to_string(),
            args: vec![
                Expression::Integer(5),
                Expression::String("*".to_string()),
            ],
        };
        assert_eq!(executor.eval_string(&string_expr).unwrap(), "*****");

        // Test STRING$(3, "ABC") - should repeat first char only
        let string_expr2 = Expression::FunctionCall {
            name: "STRING$".to_string(),
            args: vec![
                Expression::Integer(3),
                Expression::String("ABC".to_string()),
            ],
        };
        assert_eq!(executor.eval_string(&string_expr2).unwrap(), "AAA");
    }

    #[test]
    fn test_instr_function() {
        // RED: Test INSTR for substring search
        let mut executor = Executor::new();

        // Test INSTR("Hello World", "World") = 7 (1-based)
        let instr1 = Expression::FunctionCall {
            name: "INSTR".to_string(),
            args: vec![
                Expression::String("Hello World".to_string()),
                Expression::String("World".to_string()),
            ],
        };
        assert_eq!(executor.eval_integer(&instr1).unwrap(), 7);

        // Test INSTR("Hello World", "o") = 5 (first occurrence)
        let instr2 = Expression::FunctionCall {
            name: "INSTR".to_string(),
            args: vec![
                Expression::String("Hello World".to_string()),
                Expression::String("o".to_string()),
            ],
        };
        assert_eq!(executor.eval_integer(&instr2).unwrap(), 5);

        // Test INSTR("Hello World", "o", 6) = 8 (search from position 6)
        let instr3 = Expression::FunctionCall {
            name: "INSTR".to_string(),
            args: vec![
                Expression::String("Hello World".to_string()),
                Expression::String("o".to_string()),
                Expression::Integer(6),
            ],
        };
        assert_eq!(executor.eval_integer(&instr3).unwrap(), 8);

        // Test INSTR("Hello", "xyz") = 0 (not found)
        let instr4 = Expression::FunctionCall {
            name: "INSTR".to_string(),
            args: vec![
                Expression::String("Hello".to_string()),
                Expression::String("xyz".to_string()),
            ],
        };
        assert_eq!(executor.eval_integer(&instr4).unwrap(), 0);
    }

    #[test]
    fn test_data_read_integers() {
        // RED: Test DATA with integers and READ into integer variables
        let mut executor = Executor::new();

        // DATA 10, 20, 30
        let data_stmt = Statement::Data {
            values: vec![
                DataValue::Integer(10),
                DataValue::Integer(20),
                DataValue::Integer(30),
            ],
        };
        executor.execute_statement(&data_stmt).unwrap();

        // READ A%, B%, C%
        let read_stmt = Statement::Read {
            variables: vec!["A%".to_string(), "B%".to_string(), "C%".to_string()],
        };
        executor.execute_statement(&read_stmt).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 10);
        assert_eq!(executor.get_variable_int("B%").unwrap(), 20);
        assert_eq!(executor.get_variable_int("C%").unwrap(), 30);
    }

    #[test]
    fn test_data_read_strings() {
        // RED: Test DATA with strings and READ into string variables
        let mut executor = Executor::new();

        // DATA "Hello", "World", "Test"
        let data_stmt = Statement::Data {
            values: vec![
                DataValue::String("Hello".to_string()),
                DataValue::String("World".to_string()),
                DataValue::String("Test".to_string()),
            ],
        };
        executor.execute_statement(&data_stmt).unwrap();

        // READ A$, B$, C$
        let read_stmt = Statement::Read {
            variables: vec!["A$".to_string(), "B$".to_string(), "C$".to_string()],
        };
        executor.execute_statement(&read_stmt).unwrap();

        assert_eq!(executor.get_variable_string("A$").unwrap(), "Hello");
        assert_eq!(executor.get_variable_string("B$").unwrap(), "World");
        assert_eq!(executor.get_variable_string("C$").unwrap(), "Test");
    }

    #[test]
    fn test_data_read_mixed_types() {
        // RED: Test DATA with mixed types
        let mut executor = Executor::new();

        // DATA 42, 3.14, "Hello"
        let data_stmt = Statement::Data {
            values: vec![
                DataValue::Integer(42),
                DataValue::Real(3.14),
                DataValue::String("Hello".to_string()),
            ],
        };
        executor.execute_statement(&data_stmt).unwrap();

        // READ A%, B, C$
        let read_stmt = Statement::Read {
            variables: vec!["A%".to_string(), "B".to_string(), "C$".to_string()],
        };
        executor.execute_statement(&read_stmt).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 42);
        assert!((executor.get_variable_real("B").unwrap() - 3.14).abs() < 0.0001);
        assert_eq!(executor.get_variable_string("C$").unwrap(), "Hello");
    }

    #[test]
    fn test_restore() {
        // RED: Test RESTORE resets data pointer
        let mut executor = Executor::new();

        // DATA 10, 20, 30
        let data_stmt = Statement::Data {
            values: vec![
                DataValue::Integer(10),
                DataValue::Integer(20),
                DataValue::Integer(30),
            ],
        };
        executor.execute_statement(&data_stmt).unwrap();

        // READ A%, B%
        let read_stmt1 = Statement::Read {
            variables: vec!["A%".to_string(), "B%".to_string()],
        };
        executor.execute_statement(&read_stmt1).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 10);
        assert_eq!(executor.get_variable_int("B%").unwrap(), 20);

        // RESTORE
        let restore_stmt = Statement::Restore { line_number: None };
        executor.execute_statement(&restore_stmt).unwrap();

        // READ C%, D%
        let read_stmt2 = Statement::Read {
            variables: vec!["C%".to_string(), "D%".to_string()],
        };
        executor.execute_statement(&read_stmt2).unwrap();

        // After RESTORE, should read from beginning again
        assert_eq!(executor.get_variable_int("C%").unwrap(), 10);
        assert_eq!(executor.get_variable_int("D%").unwrap(), 20);
    }

    #[test]
    fn test_restore_with_line_number() {
        // RED: Test RESTORE line_number jumps to specific DATA statement
        let mut executor = Executor::new();

        // Simulate:
        // 10 DATA 100, 200
        // 20 DATA 300, 400
        // 30 READ A%, B%
        // 40 RESTORE 20
        // 50 READ C%, D%

        // Line 10: DATA 100, 200
        executor.set_line_number(Some(10));
        let data_stmt1 = Statement::Data {
            values: vec![DataValue::Integer(100), DataValue::Integer(200)],
        };
        executor.execute_statement(&data_stmt1).unwrap();

        // Line 20: DATA 300, 400
        executor.set_line_number(Some(20));
        let data_stmt2 = Statement::Data {
            values: vec![DataValue::Integer(300), DataValue::Integer(400)],
        };
        executor.execute_statement(&data_stmt2).unwrap();

        // READ A%, B% (should get 100, 200)
        let read_stmt1 = Statement::Read {
            variables: vec!["A%".to_string(), "B%".to_string()],
        };
        executor.execute_statement(&read_stmt1).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 100);
        assert_eq!(executor.get_variable_int("B%").unwrap(), 200);

        // RESTORE 20 (jump to line 20's DATA)
        let restore_stmt = Statement::Restore {
            line_number: Some(20),
        };
        executor.execute_statement(&restore_stmt).unwrap();

        // READ C%, D% (should get 300, 400 from line 20)
        let read_stmt2 = Statement::Read {
            variables: vec!["C%".to_string(), "D%".to_string()],
        };
        executor.execute_statement(&read_stmt2).unwrap();

        assert_eq!(executor.get_variable_int("C%").unwrap(), 300);
        assert_eq!(executor.get_variable_int("D%").unwrap(), 400);
    }

    #[test]
    fn test_multiple_data_statements() {
        // RED: Test multiple DATA statements accumulate
        let mut executor = Executor::new();

        // DATA 10, 20
        let data_stmt1 = Statement::Data {
            values: vec![DataValue::Integer(10), DataValue::Integer(20)],
        };
        executor.execute_statement(&data_stmt1).unwrap();

        // DATA 30, 40
        let data_stmt2 = Statement::Data {
            values: vec![DataValue::Integer(30), DataValue::Integer(40)],
        };
        executor.execute_statement(&data_stmt2).unwrap();

        // READ A%, B%, C%, D%
        let read_stmt = Statement::Read {
            variables: vec![
                "A%".to_string(),
                "B%".to_string(),
                "C%".to_string(),
                "D%".to_string(),
            ],
        };
        executor.execute_statement(&read_stmt).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 10);
        assert_eq!(executor.get_variable_int("B%").unwrap(), 20);
        assert_eq!(executor.get_variable_int("C%").unwrap(), 30);
        assert_eq!(executor.get_variable_int("D%").unwrap(), 40);
    }

    #[test]
    fn test_read_out_of_data() {
        // RED: Test reading more variables than data available
        let mut executor = Executor::new();

        // DATA 10
        let data_stmt = Statement::Data {
            values: vec![DataValue::Integer(10)],
        };
        executor.execute_statement(&data_stmt).unwrap();

        // READ A%, B% - should fail on B%
        let read_stmt = Statement::Read {
            variables: vec!["A%".to_string(), "B%".to_string()],
        };
        let result = executor.execute_statement(&read_stmt);

        assert!(result.is_err());
        // A% should have been set before error
        assert_eq!(executor.get_variable_int("A%").unwrap(), 10);
    }

    #[test]
    fn test_data_collection_with_goto() {
        // RED: Test that DATA statements are collected even when skipped by GOTO
        // This simulates a program like:
        // 10 GOTO 40
        // 20 DATA 100, 200, 300
        // 30 END
        // 40 READ A%, B%, C%
        // 50 END

        let mut executor = Executor::new();

        // First, we need to "collect" the DATA statement at line 20
        // even though execution jumps from line 10 to line 40
        let data_stmt = Statement::Data {
            values: vec![
                DataValue::Integer(100),
                DataValue::Integer(200),
                DataValue::Integer(300),
            ],
        };

        // In proper implementation, DATA should be collected BEFORE execution
        // For now, this test will fail because we need a new method to pre-collect DATA

        // Simulate what should happen: all DATA is collected first
        executor.collect_data(&data_stmt).unwrap();

        // Now READ should work even though we never "executed" line 20
        let read_stmt = Statement::Read {
            variables: vec!["A%".to_string(), "B%".to_string(), "C%".to_string()],
        };
        executor.execute_statement(&read_stmt).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 100);
        assert_eq!(executor.get_variable_int("B%").unwrap(), 200);
        assert_eq!(executor.get_variable_int("C%").unwrap(), 300);
    }

    #[test]
    fn test_rnd_range() {
        // RED: Test RND(1) returns value between 0 and 1
        let mut executor = Executor::new();

        let rnd_expr = Expression::FunctionCall {
            name: "RND".to_string(),
            args: vec![Expression::Integer(1)],
        };

        // Run RND(1) multiple times to verify it's in range and varies
        let mut values = Vec::new();
        for _ in 0..10 {
            let result = executor.eval_real(&rnd_expr).unwrap();
            assert!(
                result >= 0.0 && result < 1.0,
                "RND(1) should be in range [0, 1)"
            );
            values.push(result);
        }

        // Check that we get at least some variation (not all the same)
        let first = values[0];
        let has_variation = values.iter().any(|&v| (v - first).abs() > 0.001);
        assert!(has_variation, "RND(1) should produce varying values");
    }

    #[test]
    fn test_rnd_integer_range() {
        // RED: Test RND(n) returns integer between 1 and n
        let mut executor = Executor::new();

        let rnd_10 = Expression::FunctionCall {
            name: "RND".to_string(),
            args: vec![Expression::Integer(10)],
        };

        // Run RND(10) multiple times to verify range
        for _ in 0..20 {
            let result = executor.eval_real(&rnd_10).unwrap();
            let as_int = result as i32;
            assert!(
                as_int >= 1 && as_int <= 10,
                "RND(10) should return values 1-10, got {}",
                result
            );
        }
    }

    #[test]
    fn test_time_function() {
        // RED: Test TIME returns centiseconds
        let mut executor = Executor::new();

        // TIME is a pseudo-variable that returns current time
        let time_var = Expression::Variable("TIME".to_string());

        let result1 = executor.eval_integer(&time_var).unwrap();

        // Sleep a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result2 = executor.eval_integer(&time_var).unwrap();

        // Second reading should be >= first (time moves forward)
        assert!(result2 >= result1, "TIME should increase: {} >= {}", result2, result1);

        // Both should be positive
        assert!(result1 >= 0, "TIME should be positive");
        assert!(result2 >= 0, "TIME should be positive");
    }

    #[test]
    fn test_himem_function() {
        // RED: Test HIMEM returns top of memory
        let mut executor = Executor::new();

        let himem_var = Expression::Variable("HIMEM".to_string());

        let result = executor.eval_integer(&himem_var).unwrap();

        // HIMEM should return a positive memory address
        assert!(result > 0, "HIMEM should be positive");

        // Should be consistent across multiple reads
        let result2 = executor.eval_integer(&himem_var).unwrap();
        assert_eq!(result, result2, "HIMEM should be consistent");
    }

    #[test]
    fn test_lomem_function() {
        // RED: Test LOMEM returns bottom of memory (PAGE)
        let mut executor = Executor::new();

        let lomem_var = Expression::Variable("LOMEM".to_string());

        let result = executor.eval_integer(&lomem_var).unwrap();

        // LOMEM should return a positive memory address
        assert!(result > 0, "LOMEM should be positive");

        // Should be consistent across multiple reads
        let result2 = executor.eval_integer(&lomem_var).unwrap();
        assert_eq!(result, result2, "LOMEM should be consistent");

        // LOMEM should be less than HIMEM
        let himem_var = Expression::Variable("HIMEM".to_string());
        let himem = executor.eval_integer(&himem_var).unwrap();
        assert!(result < himem, "LOMEM should be < HIMEM");
    }

    #[test]
    fn test_err_erl_report_functions() {
        // RED: Test ERR, ERL, and REPORT$ return error information
        let mut executor = Executor::new();

        // Set a test error
        executor.set_last_error(18, 100, "Division by zero".to_string());

        // Test ERR (error number)
        let err_var = Expression::Variable("ERR".to_string());
        let err_result = executor.eval_integer(&err_var).unwrap();
        assert_eq!(err_result, 18, "ERR should return error number");

        // Test ERL (error line)
        let erl_var = Expression::Variable("ERL".to_string());
        let erl_result = executor.eval_integer(&erl_var).unwrap();
        assert_eq!(erl_result, 100, "ERL should return error line");

        // Test REPORT$ (error message)
        let report_call = Expression::FunctionCall {
            name: "REPORT$".to_string(),
            args: vec![],
        };
        let report_result = executor.eval_string(&report_call).unwrap();
        assert_eq!(report_result, "Division by zero", "REPORT$ should return error message");
    }

    #[test]
    fn test_err_erl_no_error() {
        // RED: Test ERR/ERL when no error has occurred
        let mut executor = Executor::new();

        // ERR should return 0 when no error
        let err_var = Expression::Variable("ERR".to_string());
        let err_result = executor.eval_integer(&err_var).unwrap();
        assert_eq!(err_result, 0, "ERR should return 0 when no error");

        // ERL should return 0 when no error
        let erl_var = Expression::Variable("ERL".to_string());
        let erl_result = executor.eval_integer(&erl_var).unwrap();
        assert_eq!(erl_result, 0, "ERL should return 0 when no error");

        // REPORT$ should return empty string when no error
        let report_call = Expression::FunctionCall {
            name: "REPORT$".to_string(),
            args: vec![],
        };
        let report_result = executor.eval_string(&report_call).unwrap();
        assert_eq!(report_result, "", "REPORT$ should return empty string when no error");
    }

    #[test]
    fn test_repeat_until_loop_helpers() {
        // RED: Test REPEAT...UNTIL helper methods
        use crate::parser::BinaryOperator;
        let mut executor = Executor::new();

        // Simulate:
        // 10 X% = 0
        // 20 REPEAT
        // 30 X% = X% + 1
        // 40 UNTIL X% = 5

        // Initialize X% = 0
        let init_stmt = Statement::Assignment {
            target: "X%".to_string(),
            expression: Expression::Integer(0),
        };
        executor.execute_statement(&init_stmt).unwrap();

        // REPEAT at line 20
        executor.push_repeat(20);

        // Loop several times
        for expected in 1..=5 {
            // X% = X% + 1
            let increment_stmt = Statement::Assignment {
                target: "X%".to_string(),
                expression: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(1)),
                },
            };
            executor.execute_statement(&increment_stmt).unwrap();

            // UNTIL X% = 5
            let condition = Expression::BinaryOp {
                left: Box::new(Expression::Variable("X%".to_string())),
                op: BinaryOperator::Equal,
                right: Box::new(Expression::Integer(5)),
            };

            let result = executor.check_until(&condition).unwrap();

            if expected < 5 {
                // Should loop back
                assert_eq!(result, Some(20), "Should loop back to REPEAT at line 20");
            } else {
                // Should exit loop
                assert_eq!(result, None, "Should exit loop when X% = 5");
            }
        }

        assert_eq!(executor.get_variable_int("X%").unwrap(), 5);
    }

    #[test]
    fn test_cls() {
        // RED: Test CLS outputs ANSI clear screen escape sequence
        let mut executor = Executor::new();

        let cls_stmt = Statement::Cls;
        executor.execute_statement(&cls_stmt).unwrap();

        // CLS should output the ANSI escape sequence: ESC[2J ESC[H
        // ESC[2J clears screen, ESC[H moves cursor to home
        assert!(
            executor.output.contains("\x1b[2J\x1b[H"),
            "CLS should output ANSI clear screen sequence"
        );
    }

    #[test]
    fn test_gosub_return_call_stack() {
        // RED: Test GOSUB/RETURN properly saves and restores execution position
        let mut executor = Executor::new();

        // Simulate:
        // 10 X% = 1
        // 20 GOSUB 100    (should save line 20 for return)
        // 30 X% = 3
        // 100 X% = 2
        // 110 RETURN      (should return to line AFTER 20, which is 30)

        // Push return address for line 20
        executor.push_gosub_return(20);

        // Verify return address was saved
        assert_eq!(executor.return_stack.len(), 1);

        // Pop return address
        let return_line = executor.pop_gosub_return().unwrap();

        // Should return to line 20 (caller will advance to next line)
        assert_eq!(
            return_line, 20,
            "RETURN should pop the line number that called GOSUB"
        );

        // Stack should be empty now
        assert_eq!(executor.return_stack.len(), 0);
    }

    #[test]
    fn test_gosub_return_nested() {
        // RED: Test nested GOSUB/RETURN
        let mut executor = Executor::new();

        // Simulate:
        // 10 GOSUB 100
        // 20 END
        // 100 GOSUB 200
        // 110 RETURN
        // 200 RETURN

        executor.push_gosub_return(10);
        executor.push_gosub_return(100);

        // First RETURN should go back to 100
        assert_eq!(executor.pop_gosub_return().unwrap(), 100);

        // Second RETURN should go back to 10
        assert_eq!(executor.pop_gosub_return().unwrap(), 10);
    }

    #[test]
    fn test_procedure_definition() {
        // RED: Test defining a procedure
        let mut executor = Executor::new();

        // Define a simple procedure
        executor.define_procedure("greet".to_string(), 100, vec![]);

        // Should be able to retrieve it
        let proc = executor.get_procedure("greet");
        assert!(proc.is_some());
        assert_eq!(proc.unwrap().line_number, 100);
        assert_eq!(proc.unwrap().params.len(), 0);
    }

    #[test]
    fn test_procedure_definition_with_params() {
        // RED: Test defining a procedure with parameters
        let mut executor = Executor::new();

        // Define procedure with parameters
        executor.define_procedure(
            "add".to_string(),
            200,
            vec!["X".to_string(), "Y".to_string()],
        );

        // Should be able to retrieve it
        let proc = executor.get_procedure("add");
        assert!(proc.is_some());
        assert_eq!(proc.unwrap().line_number, 200);
        assert_eq!(proc.unwrap().params, vec!["X", "Y"]);
    }

    #[test]
    fn test_procedure_not_found() {
        // RED: Test getting undefined procedure
        let mut executor = Executor::new();

        // Should return None for undefined procedure
        assert!(executor.get_procedure("undefined").is_none());
    }

    #[test]
    fn test_clear_procedures() {
        // RED: Test clearing all procedures
        let mut executor = Executor::new();

        executor.define_procedure("proc1".to_string(), 100, vec![]);
        executor.define_procedure("proc2".to_string(), 200, vec![]);

        // Both should exist
        assert!(executor.get_procedure("proc1").is_some());
        assert!(executor.get_procedure("proc2").is_some());

        // Clear all procedures
        executor.clear_procedures();

        // Both should be gone
        assert!(executor.get_procedure("proc1").is_none());
        assert!(executor.get_procedure("proc2").is_none());
    }

    #[test]
    fn test_local_variable_scoping() {
        // RED: Test that LOCAL prevents PROC from modifying global variable
        // This test implements the example from plan.md:
        // X = 10
        // PROC test
        // PRINT X  (should still be 10, not 99)
        //
        // DEF PROC test
        // LOCAL X
        // X = 99
        // ENDPROC

        let mut executor = Executor::new();

        // Set global X = 10
        executor.set_variable_int("X", 10);
        assert_eq!(executor.get_variable_int("X").unwrap(), 10);

        // Simulate PROC entry - enter local scope
        executor.enter_local_scope();

        // Simulate LOCAL X declaration inside PROC
        // This should save the old value and create a new local binding
        let local_stmt = Statement::Local {
            variables: vec!["X".to_string()],
        };
        executor.execute_statement(&local_stmt).unwrap();

        // Modify local X = 99
        executor.set_variable_int("X", 99);
        assert_eq!(executor.get_variable_int("X").unwrap(), 99);

        // Simulate ENDPROC - should restore old value
        executor.exit_local_scope().unwrap();

        // Global X should still be 10
        assert_eq!(executor.get_variable_int("X").unwrap(), 10);
    }

    #[test]
    fn test_def_fn_integer_function() {
        // RED: Test DEF FN with integer return
        // DEF FN add(X, Y) = X + Y
        // PRINT FN add(5, 3)  => should print 8

        let mut executor = Executor::new();

        // Define function: FN add(X, Y) = X + Y
        let def_fn_stmt = Statement::DefFn {
            name: "add".to_string(),
            params: vec!["X".to_string(), "Y".to_string()],
            expression: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X".to_string())),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Variable("Y".to_string())),
            },
        };
        executor.execute_statement(&def_fn_stmt).unwrap();

        // Call function: FN add(5, 3)
        let fn_call_expr = Expression::FunctionCall {
            name: "add".to_string(),
            args: vec![Expression::Integer(5), Expression::Integer(3)],
        };

        let result = executor.eval_integer(&fn_call_expr).unwrap();
        assert_eq!(result, 8);
    }

    #[test]
    fn test_def_fn_with_local_scope() {
        // RED: Test that FN parameters are local and don't affect globals
        // X = 100
        // DEF FN double(X) = X * 2
        // PRINT FN double(5)  => should print 10
        // PRINT X  => should still be 100

        let mut executor = Executor::new();

        // Set global X = 100
        executor.set_variable_int("X", 100);

        // Define function: FN double(X) = X * 2
        let def_fn_stmt = Statement::DefFn {
            name: "double".to_string(),
            params: vec!["X".to_string()],
            expression: Expression::BinaryOp {
                left: Box::new(Expression::Variable("X".to_string())),
                op: BinaryOperator::Multiply,
                right: Box::new(Expression::Integer(2)),
            },
        };
        executor.execute_statement(&def_fn_stmt).unwrap();

        // Call function: FN double(5)
        let fn_call_expr = Expression::FunctionCall {
            name: "double".to_string(),
            args: vec![Expression::Integer(5)],
        };

        let result = executor.eval_integer(&fn_call_expr).unwrap();
        assert_eq!(result, 10);

        // Global X should still be 100 (not modified by function)
        assert_eq!(executor.get_variable_int("X").unwrap(), 100);
    }

    #[test]
    fn test_power_operator() {
        // RED: Test 2 ^ 3 = 8
        let mut executor = Executor::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Integer(2)),
            op: BinaryOperator::Power,
            right: Box::new(Expression::Integer(3)),
        };
        let result = executor.eval_integer(&expr).unwrap();
        assert_eq!(result, 8);
    }

    #[test]
    fn test_modulo_operator() {
        // RED: Test 10 MOD 3 = 1
        let mut executor = Executor::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Integer(10)),
            op: BinaryOperator::Modulo,
            right: Box::new(Expression::Integer(3)),
        };
        let result = executor.eval_integer(&expr).unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_integer_divide_operator() {
        // RED: Test 10 DIV 3 = 3
        let mut executor = Executor::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Integer(10)),
            op: BinaryOperator::IntegerDivide,
            right: Box::new(Expression::Integer(3)),
        };
        let result = executor.eval_integer(&expr).unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_error_handler_set_and_clear() {
        // RED: Test ON ERROR GOTO and ON ERROR OFF
        let mut executor = Executor::new();

        // Initially no error handler
        assert_eq!(executor.get_error_handler(), None);

        // Set error handler
        executor.set_error_handler(1000);
        assert_eq!(executor.get_error_handler(), Some(1000));

        // Clear error handler
        executor.clear_error_handler();
        assert_eq!(executor.get_error_handler(), None);
    }

    #[test]
    fn test_erl_err_functions_no_error() {
        // RED: Test ERL and ERR when no error has occurred
        let mut executor = Executor::new();

        // ERL and ERR should return 0 when no error
        assert_eq!(executor.get_error_line(), 0);
        assert_eq!(executor.get_error_number(), 0);
    }

    #[test]
    fn test_erl_err_functions_after_error() {
        // RED: Test ERL and ERR after an error
        let mut executor = Executor::new();

        // Set error information
        executor.set_last_error(18, 100, "Division by zero".to_string());

        // Check ERL and ERR values
        assert_eq!(executor.get_error_line(), 100);
        assert_eq!(executor.get_error_number(), 18);
    }

    #[test]
    fn test_on_error_statement_execution() {
        // RED: Test executing ON ERROR GOTO statement
        let mut executor = Executor::new();

        let stmt = Statement::OnError { line_number: 1000 };
        executor.execute_statement(&stmt).unwrap();

        assert_eq!(executor.get_error_handler(), Some(1000));
    }

    #[test]
    fn test_on_error_off_statement_execution() {
        // RED: Test executing ON ERROR OFF statement
        let mut executor = Executor::new();

        // Set a handler first
        executor.set_error_handler(1000);

        // Execute ON ERROR OFF
        let stmt = Statement::OnErrorOff;
        executor.execute_statement(&stmt).unwrap();

        assert_eq!(executor.get_error_handler(), None);
    }

    #[test]
    fn test_erl_function_call() {
        // RED: Test calling ERL as a function
        let mut executor = Executor::new();

        // Set error info
        executor.set_last_error(6, 250, "Type mismatch".to_string());

        // Call ERL function
        let fn_call = Expression::FunctionCall {
            name: "ERL".to_string(),
            args: vec![],
        };

        let result = executor.eval_integer(&fn_call).unwrap();
        assert_eq!(result, 250);
    }

    #[test]
    fn test_err_function_call() {
        // RED: Test calling ERR as a function
        let mut executor = Executor::new();

        // Set error info
        executor.set_last_error(18, 150, "Division by zero".to_string());

        // Call ERR function
        let fn_call = Expression::FunctionCall {
            name: "ERR".to_string(),
            args: vec![],
        };

        let result = executor.eval_integer(&fn_call).unwrap();
        assert_eq!(result, 18);
    }

    #[test]
    fn test_openout_creates_file() {
        // RED: Test OPENOUT function creates file and returns handle
        use std::fs;
        let test_file = "test_openout.txt";
        
        // Clean up any existing file
        let _ = fs::remove_file(test_file);
        
        let mut executor = Executor::new();
        
        let result = executor.open_file_for_writing(test_file);
        assert!(result.is_ok());
        let handle = result.unwrap();
        assert_eq!(handle, 1); // First handle should be 1
        
        // File should exist
        assert!(fs::metadata(test_file).is_ok());
        
        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_openin_opens_existing_file() {
        // RED: Test OPENIN function opens existing file
        use std::fs;
        let test_file = "test_openin.txt";
        
        // Create a test file
        fs::write(test_file, "test content").unwrap();
        
        let mut executor = Executor::new();
        
        let result = executor.open_file_for_reading(test_file);
        assert!(result.is_ok());
        let handle = result.unwrap();
        assert_eq!(handle, 1);
        
        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_openin_fails_on_missing_file() {
        // RED: Test OPENIN returns FileNotFound error
        let mut executor = Executor::new();
        
        let result = executor.open_file_for_reading("nonexistent_file.txt");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BBCBasicError::FileNotFound(_)));
    }

    #[test]
    fn test_close_file() {
        // RED: Test CLOSE# closes a file
        use std::fs;
        let test_file = "test_close.txt";
        
        // Create a test file
        fs::write(test_file, "test").unwrap();
        
        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();
        
        // Close the file
        let handle_expr = Expression::Integer(handle);
        let result = executor.execute_close_file(&handle_expr);
        assert!(result.is_ok());
        
        // Trying to close again should fail
        let result = executor.execute_close_file(&handle_expr);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BBCBasicError::ChannelNotOpen(_)));
        
        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_print_file_writes_data() {
        // RED: Test PRINT# writes to file
        use std::fs;
        let test_file = "test_print.txt";
        
        let _ = fs::remove_file(test_file);
        
        let mut executor = Executor::new();
        let handle = executor.open_file_for_writing(test_file).unwrap();
        
        // Write some data
        let handle_expr = Expression::Integer(handle);
        let items = vec![
            crate::parser::PrintItem::Expression(Expression::String("Hello".to_string())),
            crate::parser::PrintItem::Comma,
            crate::parser::PrintItem::Expression(Expression::String("World".to_string())),
        ];
        
        let result = executor.execute_print_file(&handle_expr, &items);
        assert!(result.is_ok());
        
        // Close the file
        executor.execute_close_file(&handle_expr).unwrap();
        
        // Read back the content
        let content = fs::read_to_string(test_file).unwrap();
        assert_eq!(content, "Hello\tWorld\n");
        
        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_input_file_reads_data() {
        // RED: Test INPUT# reads from file
        use std::fs;
        let test_file = "test_input.txt";
        
        // Create test file with data
        fs::write(test_file, "42,Hello,3.14").unwrap();
        
        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();
        
        // Read data into variables
        let handle_expr = Expression::Integer(handle);
        let variables = vec!["A%".to_string(), "B$".to_string(), "C".to_string()];
        
        let result = executor.execute_input_file(&handle_expr, &variables);
        assert!(result.is_ok());
        
        // Check the variables were set
        assert_eq!(executor.variables.get_integer_var("A%").unwrap(), 42);
        assert_eq!(executor.variables.get_string_var("B$").unwrap(), "Hello");
        assert!((executor.variables.get_real_var("C").unwrap() - 3.14).abs() < 0.001);
        
        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_eof_function() {
        // RED: Test EOF# function
        use std::fs;
        let test_file = "test_eof.txt";
        
        // Create a small test file
        fs::write(test_file, "one line\n").unwrap();
        
        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();
        
        // Not at EOF initially
        let eof = executor.check_eof(handle).unwrap();
        assert_eq!(eof, 0); // FALSE
        
        // Read the line
        let handle_expr = Expression::Integer(handle);
        let variables = vec!["LINE$".to_string()];
        executor.execute_input_file(&handle_expr, &variables).unwrap();
        
        // Now at EOF
        let eof = executor.check_eof(handle).unwrap();
        assert_eq!(eof, -1); // TRUE
        
        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_multiple_file_handles() {
        // RED: Test opening multiple files simultaneously
        use std::fs;
        let file1 = "test_multi1.txt";
        let file2 = "test_multi2.txt";
        
        fs::write(file1, "File 1").unwrap();
        let _ = fs::remove_file(file2);
        
        let mut executor = Executor::new();
        
        // Open two files
        let handle1 = executor.open_file_for_reading(file1).unwrap();
        let handle2 = executor.open_file_for_writing(file2).unwrap();
        
        assert_eq!(handle1, 1);
        assert_eq!(handle2, 2);
        
        // Both should be open
        assert!(executor.open_files.contains_key(&handle1));
        assert!(executor.open_files.contains_key(&handle2));
        
        // Clean up
        drop(executor);
        let _ = fs::remove_file(file1);
        let _ = fs::remove_file(file2);
    }

    #[test]
    fn test_while_loop_helpers() {
        // RED: Test WHILE...ENDWHILE helper methods
        use crate::parser::BinaryOperator;
        let mut executor = Executor::new();

        // Simulate:
        // 10 X% = 0
        // 20 WHILE X% < 5
        // 30 X% = X% + 1
        // 40 ENDWHILE

        // Initialize X% = 0
        let init_stmt = Statement::Assignment {
            target: "X%".to_string(),
            expression: Expression::Integer(0),
        };
        executor.execute_statement(&init_stmt).unwrap();

        // WHILE X% < 5 at line 20
        let condition = Expression::BinaryOp {
            left: Box::new(Expression::Variable("X%".to_string())),
            op: BinaryOperator::LessThan,
            right: Box::new(Expression::Integer(5)),
        };

        // First check - X% = 0, should enter loop
        let result = executor.push_while(20, &condition).unwrap();
        assert_eq!(result, Some(20), "Should enter loop when X% = 0");

        // Loop several times
        for expected in 1..=5 {
            // X% = X% + 1
            let increment_stmt = Statement::Assignment {
                target: "X%".to_string(),
                expression: Expression::BinaryOp {
                    left: Box::new(Expression::Variable("X%".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Integer(1)),
                },
            };
            executor.execute_statement(&increment_stmt).unwrap();

            // Check ENDWHILE condition
            if expected < 5 {
                // Should loop back (X% < 5)
                let result = executor.check_endwhile(&condition).unwrap();
                assert_eq!(result, Some(20), "Should loop back when X% = {}", expected);
            } else {
                // Should exit loop (X% = 5)
                let result = executor.check_endwhile(&condition).unwrap();
                assert_eq!(result, None, "Should exit loop when X% = 5");
            }
        }

        // Verify X% final value
        let x_value = executor.variables.get_integer_var("X%").unwrap();
        assert_eq!(x_value, 5);
    }

    #[test]
    fn test_while_false_condition() {
        // Test WHILE with false condition (should not enter loop)
        let mut executor = Executor::new();

        // X% = 10
        let init_stmt = Statement::Assignment {
            target: "X%".to_string(),
            expression: Expression::Integer(10),
        };
        executor.execute_statement(&init_stmt).unwrap();

        // WHILE X% < 5 (false, since X% = 10)
        let condition = Expression::BinaryOp {
            left: Box::new(Expression::Variable("X%".to_string())),
            op: BinaryOperator::LessThan,
            right: Box::new(Expression::Integer(5)),
        };

        let result = executor.push_while(20, &condition).unwrap();
        assert_eq!(result, None, "Should not enter loop when condition is false");

        // while_stack should be empty (loop was never entered)
        assert!(executor.while_stack.is_empty());
    }

    #[test]
    fn test_nested_while_loops() {
        // Test nested WHILE loops
        let mut executor = Executor::new();

        // Outer: WHILE I% < 3
        executor.variables.set_integer_var("I%".to_string(), 0);
        let outer_condition = Expression::BinaryOp {
            left: Box::new(Expression::Variable("I%".to_string())),
            op: BinaryOperator::LessThan,
            right: Box::new(Expression::Integer(3)),
        };

        // Enter outer loop
        executor.push_while(10, &outer_condition).unwrap();
        assert_eq!(executor.while_stack.len(), 1);

        // Inner: WHILE J% < 2
        executor.variables.set_integer_var("J%".to_string(), 0);
        let inner_condition = Expression::BinaryOp {
            left: Box::new(Expression::Variable("J%".to_string())),
            op: BinaryOperator::LessThan,
            right: Box::new(Expression::Integer(2)),
        };

        // Enter inner loop
        executor.push_while(20, &inner_condition).unwrap();
        assert_eq!(executor.while_stack.len(), 2);

        // Exit inner loop
        executor.variables.set_integer_var("J%".to_string(), 2);
        executor.check_endwhile(&inner_condition).unwrap();
        assert_eq!(executor.while_stack.len(), 1, "Inner loop should be popped");

        // Exit outer loop
        executor.variables.set_integer_var("I%".to_string(), 3);
        executor.check_endwhile(&outer_condition).unwrap();
        assert_eq!(executor.while_stack.len(), 0, "Outer loop should be popped");
    }

    #[test]
    fn test_bget_reads_single_byte() {
        // RED: Test BGET# reads a single byte from file
        use std::fs;
        let test_file = "test_bget.dat";

        // Create a test file with some bytes
        fs::write(test_file, &[65, 66, 67, 255, 0]).unwrap();

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // Read first byte (65 = 'A')
        let byte1 = executor.bget(handle).unwrap();
        assert_eq!(byte1, 65);

        // Read second byte (66 = 'B')
        let byte2 = executor.bget(handle).unwrap();
        assert_eq!(byte2, 66);

        // Read third byte (67 = 'C')
        let byte3 = executor.bget(handle).unwrap();
        assert_eq!(byte3, 67);

        // Read fourth byte (255)
        let byte4 = executor.bget(handle).unwrap();
        assert_eq!(byte4, 255);

        // Read fifth byte (0)
        let byte5 = executor.bget(handle).unwrap();
        assert_eq!(byte5, 0);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_bget_at_eof() {
        // RED: Test BGET# at end of file returns -1
        use std::fs;
        let test_file = "test_bget_eof.dat";

        // Create a test file with one byte
        fs::write(test_file, &[42]).unwrap();

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // Read the one byte
        let byte = executor.bget(handle).unwrap();
        assert_eq!(byte, 42);

        // Try to read past EOF - should return -1
        let eof_byte = executor.bget(handle).unwrap();
        assert_eq!(eof_byte, -1);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_bput_writes_single_byte() {
        // RED: Test BPUT# writes a single byte to file
        use std::fs;
        let test_file = "test_bput.dat";

        let _ = fs::remove_file(test_file);

        let mut executor = Executor::new();
        let handle = executor.open_file_for_writing(test_file).unwrap();

        // Write some bytes
        executor.bput(handle, 65).unwrap();  // 'A'
        executor.bput(handle, 66).unwrap();  // 'B'
        executor.bput(handle, 67).unwrap();  // 'C'
        executor.bput(handle, 255).unwrap(); // Max byte value
        executor.bput(handle, 0).unwrap();   // Zero

        // Close the file
        drop(executor);

        // Read the file back to verify
        let bytes = fs::read(test_file).unwrap();
        assert_eq!(bytes, &[65, 66, 67, 255, 0]);

        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_bput_with_large_numbers() {
        // RED: Test BPUT# with numbers > 255 (should wrap using MOD 256)
        use std::fs;
        let test_file = "test_bput_wrap.dat";

        let _ = fs::remove_file(test_file);

        let mut executor = Executor::new();
        let handle = executor.open_file_for_writing(test_file).unwrap();

        // Write numbers > 255 (should MOD 256)
        executor.bput(handle, 256).unwrap();  // Should write 0
        executor.bput(handle, 257).unwrap();  // Should write 1
        executor.bput(handle, 300).unwrap();  // Should write 44

        // Close the file
        drop(executor);

        // Read the file back to verify
        let bytes = fs::read(test_file).unwrap();
        assert_eq!(bytes, &[0, 1, 44]);

        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_ptr_get_position() {
        // RED: Test PTR# function returns current file position
        use std::fs;
        let test_file = "test_ptr_get.dat";

        // Create test file with some bytes
        fs::write(test_file, &[1, 2, 3, 4, 5]).unwrap();

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // Initial position should be 0
        let pos0 = executor.get_ptr(handle).unwrap();
        assert_eq!(pos0, 0);

        // Read one byte - position should be 1
        executor.bget(handle).unwrap();
        let pos1 = executor.get_ptr(handle).unwrap();
        assert_eq!(pos1, 1);

        // Read two more bytes - position should be 3
        executor.bget(handle).unwrap();
        executor.bget(handle).unwrap();
        let pos3 = executor.get_ptr(handle).unwrap();
        assert_eq!(pos3, 3);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_ptr_set_position() {
        // RED: Test PTR# assignment sets file position
        use std::fs;
        let test_file = "test_ptr_set.dat";

        // Create test file with bytes
        fs::write(test_file, &[65, 66, 67, 68, 69]).unwrap(); // A, B, C, D, E

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // Set position to 2 (pointing at 'C')
        executor.set_ptr(handle, 2).unwrap();
        let byte = executor.bget(handle).unwrap();
        assert_eq!(byte, 67); // 'C'

        // Set position to 4 (pointing at 'E')
        executor.set_ptr(handle, 4).unwrap();
        let byte2 = executor.bget(handle).unwrap();
        assert_eq!(byte2, 69); // 'E'

        // Set position to 0 (back to start)
        executor.set_ptr(handle, 0).unwrap();
        let byte3 = executor.bget(handle).unwrap();
        assert_eq!(byte3, 65); // 'A'

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_ptr_with_output_file() {
        // RED: Test PTR# works with output files too
        use std::fs;
        let test_file = "test_ptr_output.dat";

        let _ = fs::remove_file(test_file);

        let mut executor = Executor::new();
        let handle = executor.open_file_for_writing(test_file).unwrap();

        // Write some bytes
        executor.bput(handle, 65).unwrap();
        executor.bput(handle, 66).unwrap();

        // Position should be 2
        let pos = executor.get_ptr(handle).unwrap();
        assert_eq!(pos, 2);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_ext_returns_file_size() {
        // RED: Test EXT# function returns file size
        use std::fs;
        let test_file = "test_ext.dat";

        // Create test file with 10 bytes
        fs::write(test_file, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap();

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // File size should be 10
        let size = executor.get_ext(handle).unwrap();
        assert_eq!(size, 10);

        // Size shouldn't change after reading
        executor.bget(handle).unwrap();
        executor.bget(handle).unwrap();
        let size2 = executor.get_ext(handle).unwrap();
        assert_eq!(size2, 10);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_ext_with_empty_file() {
        // RED: Test EXT# with empty file returns 0
        use std::fs;
        let test_file = "test_ext_empty.dat";

        // Create empty file
        fs::write(test_file, &[]).unwrap();

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // File size should be 0
        let size = executor.get_ext(handle).unwrap();
        assert_eq!(size, 0);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_ext_with_output_file() {
        // RED: Test EXT# works with output files
        use std::fs;
        let test_file = "test_ext_output.dat";

        let _ = fs::remove_file(test_file);

        let mut executor = Executor::new();
        let handle = executor.open_file_for_writing(test_file).unwrap();

        // Write 5 bytes
        for i in 1..=5 {
            executor.bput(handle, i).unwrap();
        }

        // Size should be 5
        let size = executor.get_ext(handle).unwrap();
        assert_eq!(size, 5);

        // Clean up
        drop(executor);
        let _ = fs::remove_file(test_file);
    }
}

