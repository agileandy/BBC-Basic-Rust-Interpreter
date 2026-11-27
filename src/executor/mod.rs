//! Execution engine for BBC BASIC statements
//!
//! Executes parsed BBC BASIC statements with proper control flow handling.

// Unit tests never read stdin, so the stdin reader is not used in them.
#[cfg_attr(test, allow(dead_code))]
mod key_input;
mod vdu;

use crate::error::{BBCBasicError, Result};
use crate::graphics::{GraphicsSystem, ScreenMode};
use crate::memory::MemoryManager;
use crate::parser::{DataValue, DimItem, Expression, Indirection, InputItem, Statement, TraceMode};
use crate::program::{Position, TYPED_LINE};
use crate::variables::{is_resident_integer, Variable, VariableStore};
use rand::Rng;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::rc::Rc;
use std::time::Duration;

/// Give the time limit of INKEY(n) or INKEY$(n) for n centiseconds.
///
/// A negative n is a key scan on a BBC Micro: INKEY(-99) is TRUE if the
/// space bar is pressed, and INKEY(-256) gives the machine type. This build
/// cannot scan the keyboard, so these give None, and the caller gives 0
/// (FALSE) or "".
fn inkey_limit(centiseconds: i32) -> Option<Duration> {
    u64::try_from(centiseconds)
        .ok()
        .map(|centiseconds| Duration::from_millis(centiseconds * 10))
}

/// Convert a real value to an integer. The fraction part is truncated toward zero.
/// A value outside the range of an i32, or a value that is not finite, gives "Too big".
fn real_to_integer(value: f64) -> Result<i32> {
    let truncated = value.trunc();
    if truncated.is_finite() && truncated >= i32::MIN as f64 && truncated <= i32::MAX as f64 {
        Ok(truncated as i32)
    } else {
        Err(BBCBasicError::TooBig)
    }
}

/// The error for a JSON function call with the wrong number of arguments.
fn json_argument_count_error(name: &str, count: usize) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: format!("{} requires {} arguments", name, count),
        line: None,
    }
}

/// Give a JSON text that a function made, or "String too long" if it is
/// longer than the string limit.
fn json_text_in_limit(text: String) -> Result<String> {
    if text.len() > crate::variables::MAX_STRING_LENGTH {
        Err(BBCBasicError::StringTooLong)
    } else {
        Ok(text)
    }
}

/// Remove the line end (`\n` or `\r\n`) of a reply, if there is one
fn without_line_end(reply: &str) -> &str {
    reply
        .strip_suffix('\n')
        .map(|r| r.strip_suffix('\r').unwrap_or(r))
        .unwrap_or(reply)
}

/// Split a reply of plain INPUT into its values at the commas.
/// Leading spaces of a value are removed and trailing spaces are kept.
/// A value that starts with a double quote is read to the closing quote,
/// with its commas and spaces kept, and the quotes are removed. The text
/// after the closing quote, to the next comma, is ignored.
fn split_input_reply(reply: &str) -> VecDeque<String> {
    let mut values = VecDeque::new();
    let mut rest = without_line_end(reply);
    loop {
        rest = rest.trim_start();
        let (value, after) = match rest.strip_prefix('"') {
            Some(quoted) => {
                let close = quoted.find('"').unwrap_or(quoted.len());
                let after = &quoted[close..];
                let comma = after.find(',').unwrap_or(after.len());
                (&quoted[..close], &after[comma..])
            }
            None => {
                let comma = rest.find(',').unwrap_or(rest.len());
                (&rest[..comma], &rest[comma..])
            }
        };
        values.push_back(value.to_string());
        match after.strip_prefix(',') {
            Some(next) => rest = next,
            None => return values,
        }
    }
}

/// Read the longest number at the start of `text`, after leading spaces, as `VAL` does.
/// VAL, INPUT and READ of a DATA text item use it.
/// The number has an optional sign, digits with one optional decimal point, and an
/// optional exponent. A `&` and hex digits is a 32-bit hex number, as in `&1F`.
/// If there is no number, the result is 0.
fn parse_leading_number(text: &str) -> f64 {
    let text = text.trim_start();
    if let Some(hex) = text.strip_prefix('&') {
        let digits = hex
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(hex.len());
        return u32::from_str_radix(&hex[..digits], 16).map_or(0.0, |v| v as i32 as f64);
    }
    let bytes = text.as_bytes();
    let digits_from = |mut i: usize| {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        i
    };

    let mut end = 0;
    if matches!(bytes.first(), Some(b'+') | Some(b'-')) {
        end = 1;
    }
    let int_end = digits_from(end);
    let mut mantissa_end = int_end;
    if bytes.get(int_end) == Some(&b'.') {
        mantissa_end = digits_from(int_end + 1);
    }
    // The mantissa must hold at least one digit
    if mantissa_end - end == 0 || (mantissa_end - end == 1 && bytes[end] == b'.') {
        return 0.0;
    }
    end = mantissa_end;

    if matches!(bytes.get(end), Some(b'E') | Some(b'e')) {
        let mut exp_start = end + 1;
        if matches!(bytes.get(exp_start), Some(b'+') | Some(b'-')) {
            exp_start += 1;
        }
        let exp_end = digits_from(exp_start);
        if exp_end > exp_start {
            end = exp_end;
        }
    }

    text[..end].parse::<f64>().unwrap_or(0.0)
}

/// Make the hex text of an integer, as `PRINT ~` does: upper case, with no `&`
/// and no leading zeros. A negative value shows its 32-bit form.
fn format_hex(value: i32) -> String {
    format!("{:X}", value as u32)
}

/// Make the PRINT text for a real value, as a BBC Micro does. The value is rounded
/// to 9 significant digits, without trailing zeros. If the absolute value is from
/// 1E-7 up to (not including) 1E9, the text is plain decimal. Otherwise the text
/// has a mantissa and an exponent, for example `1.5E10`.
fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if !value.is_finite() {
        return value.to_string();
    }

    // Round one time, then make each form from the same 9 digits
    let scientific = format!("{:.8e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("exponent format has an exponent");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");
    let digits = mantissa.replace('.', "");

    let text = if (-7..9).contains(&exponent) {
        let (whole, fraction) = if exponent >= 0 {
            let split = exponent as usize + 1;
            (digits[..split].to_string(), digits[split..].to_string())
        } else {
            let zeros = "0".repeat((-exponent - 1) as usize);
            ("0".to_string(), format!("{}{}", zeros, digits))
        };
        let fraction = fraction.trim_end_matches('0');
        if fraction.is_empty() {
            whole
        } else {
            format!("{}.{}", whole, fraction)
        }
    } else {
        let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
        format!("{}E{}", mantissa, exponent)
    };

    if value < 0.0 {
        format!("-{}", text)
    } else {
        text
    }
}

/// File handle for file I/O operations
#[derive(Debug)]
enum FileHandle {
    /// File opened for reading (OPENIN)
    Input(BufReader<File>),
    /// File opened for writing (OPENOUT)
    Output(BufWriter<File>),
    /// File opened for reading and writing (OPENUP)
    Update(BufReader<File>),
}

/// Local variable frame for procedure/function scoping
#[derive(Debug, Clone)]
struct LocalFrame {
    /// Saved variable values (variable name -> saved value)
    saved_variables: HashMap<String, Option<Variable>>,
    /// Saved arrays (array name -> saved entry, see `VariableStore::save_array`)
    saved_arrays: HashMap<String, Option<Option<Variable>>>,
}

impl LocalFrame {
    fn new() -> Self {
        Self {
            saved_variables: HashMap::new(),
            saved_arrays: HashMap::new(),
        }
    }
}

/// Procedure definition
#[derive(Debug, Clone)]
pub struct ProcedureDefinition {
    pub position: Position,
    pub params: Vec<String>,
}

/// Function definition (DEF FN)
#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub params: Vec<String>,
    pub body: FunctionBody,
}

/// The body of a function definition
#[derive(Debug, Clone)]
pub enum FunctionBody {
    /// Single-line function: `DEF FNname(params) = expr`
    Expression(Expression),
    /// Multi-line function: the body starts after the DEF statement at this position
    Lines(Position),
}

/// Evaluates the `=expr` result of a multi-line function in the type that
/// the caller asks for, and keeps the value.
pub type ResultEvaluator<'a> = &'a mut dyn FnMut(&mut Executor, &Expression) -> Result<()>;

/// Runs the body of a multi-line function from its DEF line.
///
/// At the `=expr` line that ends the body, it calls the evaluator with the
/// expression. The function scope is still active at that time, and an error
/// in the expression has the line number of the `=expr` line.
pub type FunctionBodyRunner = Rc<dyn Fn(&mut Executor, Position, ResultEvaluator) -> Result<()>>;

/// Holds the optional function body runner, so that `Executor` can derive `Debug`.
#[derive(Clone, Default)]
struct BodyRunnerSlot(Option<FunctionBodyRunner>);

impl std::fmt::Debug for BodyRunnerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Some(<function body runner>)"
        } else {
            "None"
        })
    }
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

/// Where the run continues when an error occurs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorHandler {
    /// ON ERROR GOTO: the first statement of this line
    Line(u16),
    /// ON ERROR <statements>: the first statement after ON ERROR
    Statements(Position),
}

/// The number of files that a program can open at one time.
const DEFAULT_OPEN_FILE_LIMIT: usize = 255;

/// The statement that put an entry on the return stack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnKind {
    /// GOSUB or ON GOSUB: RETURN goes back
    Gosub,
    /// A PROC call: ENDPROC goes back
    Proc,
}

/// BBC BASIC statement executor
#[derive(Debug)]
pub struct Executor {
    variables: VariableStore,
    memory: MemoryManager,
    graphics: GraphicsSystem,
    // Control flow stack for GOSUB/RETURN and PROC/ENDPROC
    return_stack: Vec<(ReturnKind, Position)>,
    // FOR loop state: (variable, end_value, step_value, FOR position).
    // An integer loop (`%` variable) holds whole values, which f64 holds with no loss.
    for_loops: Vec<(String, f64, f64, Position)>,
    // REPEAT loop stack: stores the positions of REPEAT statements
    repeat_stack: Vec<Position>,
    // WHILE loop stack: stores the positions of WHILE statements
    while_stack: Vec<Position>,
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
    // Runs the body lines of a multi-line function (set by the run loop)
    function_body_runner: BodyRunnerSlot,
    // Local variable stack for PROC/FN scoping
    local_stack: Vec<LocalFrame>,
    // Error handler: where the run continues on error (None = no handler)
    error_handler: Option<ErrorHandler>,
    // TRACE: the run prints each stored line number below this limit (None = off)
    trace_limit: Option<u32>,
    // Last error information (for ERL and ERR functions)
    last_error: Option<ErrorInfo>,
    // Open file handles: handle number -> FileHandle
    open_files: HashMap<i32, FileHandle>,
    // Maximum number of files open at one time. A test can lower it.
    open_file_limit: usize,
    // Next file handle number to allocate
    next_file_handle: i32,
    // FOR position to jump back to after the last NEXT (None = that loop is complete)
    next_loop_line: Option<Position>,
    // Start instant: TIME counts centiseconds from here
    start_time: std::time::Instant,
    // Size in bytes of the stored program: TOP is PAGE plus this size
    program_size: usize,
    // Bytes that `DIM name size` reserved above TOP. The next block starts
    // at TOP plus this count.
    dim_space_used: usize,
    // Replies for INPUT, used before stdin (for testing)
    input_queue: VecDeque<String>,
    // Keys for GET and GET$, used before stdin (for testing)
    key_queue: VecDeque<char>,
    // Output buffer (for testing)
    output: String,
    // Column of the print position: 0 after a newline, updated by print_output
    column: usize,
    // Current screen mode (MODE). A BBC Micro starts in mode 7, but the
    // executor starts in mode 0 so that graphics work with no MODE statement.
    screen_mode: ScreenMode,
    // Text colours (COLOUR and VDU 17). MODE sets the defaults: foreground is
    // the highest colour of the mode, and background is 0.
    text_foreground: u8,
    text_background: u8,
    // Bytes of a VDU command that waits for more parameter bytes. They stay
    // between VDU statements.
    vdu_queue: Vec<u8>,
}

impl Executor {
    /// Create a new executor
    pub fn new() -> Self {
        Self {
            variables: VariableStore::new(),
            memory: MemoryManager::new(),
            graphics: GraphicsSystem::new(),
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
            function_body_runner: BodyRunnerSlot::default(),
            local_stack: Vec::new(),
            error_handler: None,
            trace_limit: None,
            last_error: None,
            open_files: HashMap::new(),
            open_file_limit: DEFAULT_OPEN_FILE_LIMIT,
            next_file_handle: 1,
            next_loop_line: None,
            start_time: std::time::Instant::now(),
            program_size: 0,
            dim_space_used: 0,
            input_queue: VecDeque::new(),
            key_queue: VecDeque::new(),
            output: String::new(),
            column: 0,
            screen_mode: ScreenMode::from_number(0).expect("mode 0 is in the mode table"),
            // The defaults of mode 0, which has 2 colours
            text_foreground: 1,
            text_background: 0,
            vdu_queue: Vec::new(),
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
            Statement::ArrayAssignment {
                name,
                indices,
                expression,
            } => self.execute_array_assignment(name, indices, expression),
            Statement::MemoryAssignment {
                width,
                address,
                expression,
            } => self.execute_memory_assignment(*width, address, expression),
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
            Statement::Input { items } => self.execute_input(items),
            Statement::InputLine { items } => self.execute_input_line(items),
            Statement::Dim { items } => self.execute_dim(items),
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
            // REPORT prints the text of the last error, with no new line
            Statement::Report => {
                let text = self.last_error.as_ref().map(|e| e.message.clone());
                self.print_output(&text.unwrap_or_default());
                Ok(())
            }
            Statement::Clear => self.execute_clear(),
            // Graphics statements
            Statement::Plot { mode, x, y } => self.execute_plot(mode, x, y),
            Statement::Move { x, y } => self.execute_move(x, y),
            Statement::Draw { x, y } => self.execute_draw(x, y),
            Statement::Circle { x, y, radius } => self.execute_circle(x, y, radius),
            Statement::Gcol { mode, color } => self.execute_gcol(mode, color),
            Statement::Clg => self.execute_clg(),
            Statement::Mode { mode } => self.execute_mode(mode),
            Statement::Colour { colour } => self.execute_colour(colour),
            Statement::Vdu { items } => self.execute_vdu(items),
            Statement::Ellipse { x, y, major, minor } => self.execute_ellipse(x, y, major, minor),
            Statement::Rectangle {
                x1,
                y1,
                width,
                height,
                filled,
            } => self.execute_rectangle(x1, y1, width, height, *filled),
            Statement::Fill { x, y } => self.execute_fill(x, y),
            Statement::Origin { x, y } => self.execute_origin(x, y),
            // STEREO evaluates its arguments and has no other effect until
            // sound exists (issue #9).
            Statement::Stereo { channel, position } => {
                self.eval_integer(channel)?;
                self.eval_integer(position)?;
                Ok(())
            }
            Statement::MakeDirectory { path } => {
                crate::filesystem::make_directory(&self.eval_string(path)?)
            }
            Statement::RemoveDirectory { path } => {
                crate::filesystem::remove_directory(&self.eval_string(path)?)
            }
            Statement::DefProc { .. } => {
                // DEF PROC is handled during procedure collection in main.rs
                Ok(())
            }
            Statement::DefFn {
                name,
                params,
                expression: Some(expression),
            } => self.execute_def_fn(name, params, expression),
            Statement::DefFn {
                expression: None, ..
            } => {
                // A multi-line DEF FN is collected before the program runs
                Ok(())
            }
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
            Statement::OnErrorStatements { .. } => {
                // The run loop sets the handler, because it knows the position
                Ok(())
            }
            Statement::Trace { mode } => self.execute_trace(mode),
            Statement::PrintFile { handle, items } => self.execute_print_file(handle, items),
            Statement::InputFile { handle, variables } => {
                self.execute_input_file(handle, variables)
            }
            Statement::CloseFile { handle } => self.execute_close_file(handle),
            Statement::Bput { handle, value } => {
                let handle = self.eval_integer(handle)?;
                let value = self.eval_integer(value)?;
                self.bput(handle, value)
            }
            Statement::SetPtr { handle, position } => {
                let handle = self.eval_integer(handle)?;
                let position = self.eval_integer(position)?;
                self.set_ptr(handle, position)
            }
            Statement::SaveJson { file, json } => {
                let file = self.eval_string(file)?;
                let json = self.eval_string(json)?;
                Ok(crate::json::save_file(&file, &json)?)
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
        } else if target == "TIME$" {
            // Setting the clock with TIME$ is not supported.
            Err(BBCBasicError::SyntaxError {
                message: "TIME$ cannot be set".to_string(),
                line: None,
            })
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

    /// Execute an array element assignment statement (e.g., numbers%(I%) = 5)
    fn execute_array_assignment(
        &mut self,
        name: &str,
        indices: &[Expression],
        expression: &Expression,
    ) -> Result<()> {
        use crate::variables::Variable;

        // Evaluate all indices to integers
        let mut index_values = Vec::with_capacity(indices.len());
        for index_expr in indices {
            let idx = self.eval_integer(index_expr)?;
            if idx < 0 {
                return Err(BBCBasicError::SubscriptOutOfRange);
            }
            index_values.push(idx as usize);
        }

        // Determine the array type and evaluate the expression accordingly
        let value = if name.ends_with('%') {
            Variable::Integer(self.eval_integer(expression)?)
        } else if name.ends_with('$') {
            Variable::String(self.eval_string(expression)?)
        } else {
            Variable::Real(self.eval_real(expression)?)
        };

        // Set the array element
        self.variables.set_array_element(name, &index_values, value)
    }

    /// The addresses of the bytes that a memory operator uses, low byte
    /// first. An address has 16 bits, as on the BBC Micro, so the high bits
    /// of the value are ignored.
    fn memory_addresses(&mut self, width: Indirection, address: &Expression) -> Result<Vec<u16>> {
        let base = self.eval_integer(address)? as u16;
        let count = match width {
            Indirection::Byte => 1,
            Indirection::Word => 4,
        };
        Ok((0..count).map(|i| base.wrapping_add(i)).collect())
    }

    /// Read `?addr` (one byte, 0 to 255) or `!addr` (four bytes, low byte
    /// first, as a signed integer).
    fn read_memory(&mut self, width: Indirection, address: &Expression) -> Result<i32> {
        let mut bytes = [0u8; 4];
        for (byte, address) in bytes.iter_mut().zip(self.memory_addresses(width, address)?) {
            *byte = self.memory.peek(address)?;
        }
        Ok(i32::from_le_bytes(bytes))
    }

    /// Execute `?addr = value` (the low 8 bits) or `!addr = value` (4 bytes).
    /// If a byte is outside memory, no byte changes.
    fn execute_memory_assignment(
        &mut self,
        width: Indirection,
        address: &Expression,
        expression: &Expression,
    ) -> Result<()> {
        let addresses = self.memory_addresses(width, address)?;
        let value = self.eval_integer(expression)?.to_le_bytes();
        for &address in &addresses {
            self.memory.peek(address)?;
        }
        for (&address, byte) in addresses.iter().zip(value) {
            self.memory.poke(address, byte)?;
        }
        Ok(())
    }

    /// Execute a PRINT statement
    fn execute_print(&mut self, items: &[crate::parser::PrintItem]) -> Result<()> {
        use crate::parser::PrintItem;

        // After ~, numeric items print in hex until the next comma or semicolon
        let mut hex = false;
        for item in items {
            match item {
                PrintItem::Expression(expr) => {
                    // Evaluate expression and print it
                    let output = self.format_print_item(expr, hex)?;
                    self.print_output(&output);
                }
                PrintItem::Hex => hex = true,
                PrintItem::Semicolon => {
                    // Semicolon suppresses newline (do nothing)
                    hex = false;
                }
                PrintItem::Comma => {
                    hex = false;
                    // Comma moves to the next print zone of 10 columns
                    let next_zone = (self.column / 10 + 1) * 10;
                    self.print_output(&" ".repeat(next_zone - self.column));
                }
                PrintItem::Tab(expr) => {
                    // TAB evaluates its value one time and truncates a real
                    let pos = self.eval_number(expr)?.floor().max(0.0) as usize;
                    // A column before the cursor starts a new line (BBC rule)
                    if pos < self.column {
                        self.print_new_line();
                    }
                    self.print_output(&" ".repeat(pos - self.column));
                }
                PrintItem::TabXY(x, y) => {
                    // The same as VDU 31, x, y: each value is one byte
                    let x = self.eval_integer(x)? as u8;
                    let y = self.eval_integer(y)? as u8;
                    self.move_text_cursor(x, y);
                }
                PrintItem::Spc(expr) => {
                    // SPC evaluates its value one time and truncates a real
                    let count = self.eval_number(expr)?.floor().max(0.0) as usize;
                    self.print_output(&" ".repeat(count));
                }
            }
        }

        // Add newline unless last item was semicolon
        if items.is_empty() || !matches!(items.last(), Some(PrintItem::Semicolon)) {
            self.print_new_line();
        }

        Ok(())
    }

    /// Format a PRINT item. If `hex` is true, a numeric item shows in hex.
    /// A string item does not change.
    fn format_print_item(&mut self, expr: &Expression, hex: bool) -> Result<String> {
        if hex && !self.is_string_expr(expr) {
            Ok(format_hex(real_to_integer(self.eval_number(expr)?)?))
        } else {
            self.format_expression(expr)
        }
    }

    /// Format an expression for printing
    fn format_expression(&mut self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::Integer(_) => Ok(self.eval_integer(expr)?.to_string()),
            Expression::Real(_) => Ok(format_number(self.eval_real(expr)?)),
            Expression::String(_) => self.eval_string(expr),
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    Ok(self.eval_integer(expr)?.to_string())
                } else if name.ends_with('$') {
                    self.eval_string(expr)
                } else {
                    Ok(format_number(self.eval_real(expr)?))
                }
            }
            // VAL gives a real value, so do not try it as an integer first
            Expression::FunctionCall { name, .. } if name == "VAL" => {
                Ok(format_number(self.eval_real(expr)?))
            }
            // A user function with no $ in its name can give a string or a
            // number. Run its body one time and format the result by its type.
            Expression::FunctionCall { name, args }
                if !name.ends_with('$') && self.functions.contains_key(name) =>
            {
                self.call_function(name, args, Self::format_function_result)
            }
            // Select the evaluator by the type of the expression. Check for a
            // string first, because `is_real_expr` is true for each user
            // function, also for a string function such as FNx$.
            _ if self.is_string_expr(expr) => self.eval_string(expr),
            _ if self.is_real_expr(expr) => Ok(format_number(self.eval_real(expr)?)),
            _ => Ok(self.eval_integer(expr)?.to_string()),
        }
    }

    /// Format the `=` result of a user function for PRINT. A string prints as
    /// text, and a number prints as a real value.
    fn format_function_result(&mut self, expr: &Expression) -> Result<String> {
        match expr {
            _ if self.is_string_expr(expr) => self.eval_string(expr),
            // The result is the result of a second function: format it by its type
            Expression::FunctionCall { name, .. }
                if !name.ends_with('$') && self.functions.contains_key(name) =>
            {
                self.format_expression(expr)
            }
            _ => Ok(format_number(self.eval_real(expr)?)),
        }
    }

    /// Print output (to buffer in test mode, to stdout in production).
    /// A line feed (code 10) keeps the column. A carriage return (code 13)
    /// sets it to 0.
    fn print_output(&mut self, text: &str) {
        for ch in text.chars() {
            match ch {
                '\n' => {}
                '\r' => self.column = 0,
                _ => self.column += 1,
            }
        }
        self.output.push_str(text);
        #[cfg(not(test))]
        {
            print!("{}", text);
        }
    }

    /// Print a new line (BBC codes 13 and 10) and set the column to 0
    fn print_new_line(&mut self) {
        self.print_output("\n");
        self.column = 0;
    }

    /// Set the print column to 0. The runner calls this at the start of a
    /// run, because the command loop prints its messages with no count.
    pub fn reset_column(&mut self) {
        self.column = 0;
    }

    /// Get output buffer (for testing)
    pub fn get_output(&self) -> &str {
        &self.output
    }

    /// Add a reply for a later INPUT. Queued replies are used before stdin.
    pub fn queue_input(&mut self, line: &str) {
        self.input_queue.push_back(line.to_string());
    }

    /// Add keys for later GET or GET$, one key for each character.
    /// Queued keys are used before stdin.
    pub fn queue_keys(&mut self, keys: &str) {
        self.key_queue.extend(keys.chars());
    }

    /// Take the next key from the queue. If the queue is empty, read one key
    /// from stdin. Give None at the end of input.
    fn next_key(&mut self) -> Option<char> {
        if let Some(key) = self.key_queue.pop_front() {
            return Some(key);
        }
        #[cfg(test)]
        {
            None
        }
        #[cfg(not(test))]
        {
            key_input::read_key().map(char::from)
        }
    }

    /// Take the next key from the queue with no wait. If the queue is empty,
    /// wait for a maximum of `limit` for one key from stdin. Give None if no
    /// key comes in that time or at the end of input.
    fn next_key_within(&mut self, limit: Duration) -> Option<char> {
        if let Some(key) = self.key_queue.pop_front() {
            return Some(key);
        }
        #[cfg(test)]
        {
            let _ = limit;
            None
        }
        #[cfg(not(test))]
        {
            key_input::read_key_within(limit).map(char::from)
        }
    }

    /// Clear output buffer (for testing)
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

    /// Execute FOR statement
    fn execute_for(
        &mut self,
        variable: &str,
        start: &Expression,
        end: &Expression,
        step: Option<&Expression>,
    ) -> Result<()> {
        // A `%` loop variable is an integer loop: the values are truncated.
        // Any other loop variable is a real loop: the values keep their fraction parts.
        let (end_val, step_val) = if variable.ends_with('%') {
            let start_val = self.eval_integer(start)?;
            let end_val = self.eval_integer(end)?;
            let step_val = match step {
                Some(step_expr) => self.eval_integer(step_expr)?,
                None => 1,
            };
            self.variables
                .set_integer_var(variable.to_string(), start_val);
            (f64::from(end_val), f64::from(step_val))
        } else {
            let start_val = self.eval_number(start)?;
            let end_val = self.eval_number(end)?;
            let step_val = match step {
                Some(step_expr) => self.eval_number(step_expr)?,
                None => 1.0,
            };
            self.variables.set_real_var(variable.to_string(), start_val);
            (end_val, step_val)
        };

        // Store loop state: (variable, end_value, step_value, FOR position)
        // The run loop sets the FOR position (set_for_loop_line)
        self.for_loops.push((
            variable.to_string(),
            end_val,
            step_val,
            Position::line_start(0),
        ));

        Ok(())
    }

    /// Execute NEXT statement
    ///
    /// `NEXT a, b` handles each name in order. It stops at the first loop
    /// that continues. If each loop ends, the program continues after the NEXT.
    fn execute_next(&mut self, variables: &[String]) -> Result<()> {
        // If no variables specified, use the most recent FOR loop
        if variables.is_empty() {
            let Some((name, _, _, _)) = self.for_loops.last() else {
                return Err(BBCBasicError::NoFor);
            };
            return self.next_loop(name.clone());
        }

        for var_name in variables {
            self.next_loop(var_name.clone())?;
            if self.next_loop_line.is_some() {
                break;
            }
        }

        Ok(())
    }

    /// Step the FOR loop of one variable. Set the position to jump back to
    /// if the loop continues, or clear it if the loop ends.
    fn next_loop(&mut self, var_name: String) -> Result<()> {
        // Find the matching FOR loop
        let loop_index = self
            .for_loops
            .iter()
            .rposition(|(name, _, _, _)| name == &var_name)
            .ok_or(if self.for_loops.is_empty() {
                BBCBasicError::NoFor
            } else {
                BBCBasicError::CantMatchFor
            })?;

        let (_, end_val, step_val, _) = self.for_loops[loop_index];

        // Increment the loop variable
        let next_val = if var_name.ends_with('%') {
            let current_val = self
                .variables
                .get_integer_var(&var_name)
                .ok_or_else(|| BBCBasicError::NoSuchVariable(var_name.clone()))?;
            // The step of an integer loop is an i32 value (see execute_for).
            // If the next value overflows, it is past the end value. The loop
            // ends and the variable keeps its last value.
            match current_val.checked_add(step_val as i32) {
                Some(next_val) => {
                    self.variables.set_integer_var(var_name.clone(), next_val);
                    f64::from(next_val)
                }
                None => f64::from(current_val) + step_val,
            }
        } else {
            let current_val = self
                .variables
                .get_real_var(&var_name)
                .or_else(|| self.variables.get_integer_var(&var_name).map(f64::from))
                .ok_or_else(|| BBCBasicError::NoSuchVariable(var_name.clone()))?;
            let next_val = current_val + step_val;
            self.variables.set_real_var(var_name.clone(), next_val);
            next_val
        };

        // Check if loop is complete. A step of 0 uses the test for a positive step,
        // as in BBC BASIC.
        let loop_complete = if step_val >= 0.0 {
            next_val > end_val
        } else {
            next_val < end_val
        };

        // A NEXT for an outer loop closes the loops inside it
        self.for_loops.truncate(loop_index + 1);

        if loop_complete {
            // Remove the loop from the stack
            self.for_loops.remove(loop_index);
            self.next_loop_line = None;
        } else {
            // The run loop jumps back to the FOR line of this loop
            self.next_loop_line = Some(self.for_loops[loop_index].3);
        }

        Ok(())
    }

    /// Execute INPUT statement. A prompt text shows as it is. A variable that
    /// reads a new reply line shows `?`, but not directly after a prompt text
    /// with no comma or semicolon. One reply line can give values to more than
    /// one variable, with a comma between the values. Extra values are ignored.
    fn execute_input(&mut self, items: &[InputItem]) -> Result<()> {
        // The values that remain from the last reply line
        let mut values: VecDeque<String> = VecDeque::new();
        let mut question_mark = true;

        for item in items {
            let (var, indices) = match item {
                InputItem::Prompt {
                    text,
                    question_mark: shows,
                } => {
                    self.print_output(text);
                    question_mark = *shows;
                    continue;
                }
                InputItem::Variable(var) => (var, None),
                InputItem::ArrayElement { name, indices } => (name, Some(indices)),
            };
            if values.is_empty() {
                let reply = self.read_input_reply(question_mark);
                values = split_input_reply(&reply);
            }
            question_mark = true;
            let reply = values.pop_front().unwrap_or_default();

            // A reply that is not a number gives 0, as on a BBC Micro
            let value = if var.ends_with('%') {
                Variable::Integer(real_to_integer(parse_leading_number(&reply))?)
            } else if var.ends_with('$') {
                Variable::String(reply)
            } else {
                Variable::Real(parse_leading_number(&reply))
            };
            self.store_input_value(var, indices.map(Vec::as_slice), value)?;
        }
        Ok(())
    }

    /// Store an INPUT value in a variable, or in an array element if `indices`
    /// is given. The indices are evaluated now.
    fn store_input_value(
        &mut self,
        name: &str,
        indices: Option<&[Expression]>,
        value: Variable,
    ) -> Result<()> {
        if let Some(indices) = indices {
            let mut index_values = Vec::with_capacity(indices.len());
            for index_expr in indices {
                let idx = self.eval_integer(index_expr)?;
                if idx < 0 {
                    return Err(BBCBasicError::SubscriptOutOfRange);
                }
                index_values.push(idx as usize);
            }
            return self.variables.set_array_element(name, &index_values, value);
        }
        match value {
            Variable::Integer(val) => self.variables.set_integer_var(name.to_string(), val),
            Variable::String(val) => self.variables.set_string_var(name.to_string(), val)?,
            Variable::Real(val) => self.variables.set_real_var(name.to_string(), val),
            _ => return Err(BBCBasicError::TypeMismatch),
        }
        Ok(())
    }

    /// Execute INPUT LINE statement. Each string variable gets one full reply.
    /// Only the line end is removed. A numeric variable gives a type mismatch.
    /// The prompt texts and `?` follow the rules of INPUT.
    fn execute_input_line(&mut self, items: &[InputItem]) -> Result<()> {
        let mut question_mark = true;

        for item in items {
            let (var, indices) = match item {
                InputItem::Prompt {
                    text,
                    question_mark: shows,
                } => {
                    self.print_output(text);
                    question_mark = *shows;
                    continue;
                }
                InputItem::Variable(var) => (var, None),
                InputItem::ArrayElement { name, indices } => (name, Some(indices)),
            };
            if !var.ends_with('$') {
                return Err(BBCBasicError::TypeMismatch);
            }
            let reply = self.read_input_reply(question_mark);
            question_mark = true;
            let reply = without_line_end(&reply);
            self.store_input_value(
                var,
                indices.map(Vec::as_slice),
                Variable::String(reply.to_string()),
            )?;
        }
        Ok(())
    }

    /// Show `?` if `question_mark` is true, then read the next reply line.
    /// The Enter key of the reply moves the cursor to column 0.
    fn read_input_reply(&mut self, question_mark: bool) -> String {
        if question_mark {
            self.print_output("?");
        }
        let reply = self.next_input_reply();
        self.column = 0;
        reply
    }

    /// Take the next INPUT reply from the queue. If the queue is empty, read stdin.
    /// A read error or the end of input gives an empty reply. A queued reply
    /// adds a new line to the output, as the Enter key does on a screen.
    fn next_input_reply(&mut self) -> String {
        if let Some(reply) = self.input_queue.pop_front() {
            self.print_new_line();
            return reply;
        }
        #[cfg(test)]
        {
            String::new()
        }
        #[cfg(not(test))]
        {
            use std::io;

            let _ = io::stdout().flush();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                input.clear();
            }
            input
        }
    }

    /// Execute DIM statement
    fn execute_dim(&mut self, items: &[DimItem]) -> Result<()> {
        for item in items {
            let (name, dimensions) = match item {
                DimItem::Array { name, dimensions } => (name, dimensions),
                DimItem::Block { name, size } => {
                    self.execute_dim_block(name, size)?;
                    continue;
                }
            };
            // Evaluate dimension expressions
            let mut dim_sizes = Vec::new();
            for dim_expr in dimensions {
                let size = self.eval_integer(dim_expr)?;
                if size < 0 {
                    return Err(BBCBasicError::SubscriptOutOfRange);
                }
                // DIM A(N) gives the elements 0 to N, so N + 1 elements
                dim_sizes.push(size as usize + 1);
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

    /// Execute `DIM name size`: reserve `size + 1` bytes above TOP and put
    /// the address of the first byte in `name`. A size of -1 gives the
    /// address and reserves no bytes.
    fn execute_dim_block(&mut self, name: &str, size: &Expression) -> Result<()> {
        let size = self.eval_integer(size)?;
        if size < -1 {
            return Err(BBCBasicError::BadDim);
        }
        let bytes = (size as i64 + 1) as usize;
        let address = self.top() as usize + self.dim_space_used;
        if address + bytes > self.memory.get_himem() as usize {
            return Err(BBCBasicError::NoRoom);
        }
        self.execute_assignment(name, &Expression::Integer(address as i32))?;
        self.dim_space_used += bytes;
        Ok(())
    }

    /// Free the blocks that `DIM name size` reserved. The next block
    /// starts at TOP. `clear_variables` uses this.
    pub fn reset_dim_space(&mut self) {
        self.dim_space_used = 0;
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

    /// Execute DATA statement - does nothing when the run reaches it.
    /// The first pass of `run_program` collects all DATA values with `collect_data`.
    fn execute_data(&mut self, _values: &[DataValue]) -> Result<()> {
        Ok(())
    }

    /// Collect DATA statement values without executing (for program pre-processing)
    /// This is used to collect all DATA statements before program execution begins
    pub fn collect_data(&mut self, statement: &Statement, line_number: u16) -> Result<()> {
        if let Statement::Data { values } = statement {
            for value in values {
                self.data_values.push(value.clone());
                self.data_line_numbers.push(Some(line_number));
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

    /// Use the DATA values of the stored program (before a typed line). If
    /// they are the same as the current values, the DATA pointer stays, so
    /// READ continues. Otherwise the pointer goes to the first value.
    pub fn use_data_pool(&mut self, values: Vec<DataValue>, line_numbers: Vec<Option<u16>>) {
        if values == self.data_values && line_numbers == self.data_line_numbers {
            return;
        }
        self.data_values = values;
        self.data_line_numbers = line_numbers;
        self.data_pointer = 0;
    }

    /// Execute READ statement - reads data into variables
    fn execute_read(&mut self, variables: &[String]) -> Result<()> {
        for var_name in variables {
            // Check if we've run out of data
            if self.data_pointer >= self.data_values.len() {
                return Err(BBCBasicError::OutOfData);
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
                    DataValue::String(s) => parse_leading_number(s) as i32,
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
                    DataValue::String(s) => parse_leading_number(s),
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
            // No DATA at or after the target line: the next READ gives "Out of DATA"
            self.data_pointer = self.data_values.len();
        } else {
            // No line number: reset to beginning
            self.data_pointer = 0;
        }
        Ok(())
    }

    /// Execute CLEAR. See `clear_variables`.
    fn execute_clear(&mut self) -> Result<()> {
        self.clear_variables();
        Ok(())
    }

    /// Remove all variables, arrays and DIM blocks, close the loops, the calls and the
    /// LOCAL scopes, move the DATA pointer to the first value and remove a
    /// VDU command that is not complete. The
    /// resident integer variables A% to Z%, the procedures and the
    /// functions stay. CLEAR, RUN, NEW, LOAD and CHAIN use this.
    pub fn clear_variables(&mut self) {
        self.vdu_queue.clear();
        self.variables.clear();
        self.reset_dim_space();
        self.clear_control_stacks();
        self.local_stack.clear();
        self.data_pointer = 0;
    }

    /// The LVAR text: one line for each scalar variable, then one line for
    /// each array with its top index in each dimension, in name order.
    /// A resident integer variable (A% to Z%) is not listed when it is 0.
    pub fn variable_listing(&self) -> Vec<String> {
        let scalars = self
            .variables
            .scalars()
            .into_iter()
            .filter(|(name, variable)| {
                !(is_resident_integer(name) && **variable == Variable::Integer(0))
            })
            .map(|(name, variable)| match variable {
                Variable::Integer(value) => format!("{} = {}", name, value),
                Variable::Real(value) => format!("{} = {}", name, format_number(*value)),
                Variable::String(value) => format!("{} = \"{}\"", name, value),
                _ => name.to_string(),
            });
        let arrays = self.variables.arrays().into_iter().map(|(name, array)| {
            let top_indices: Vec<String> = array
                .dimensions()
                .unwrap_or_default()
                .iter()
                .map(|size| (size - 1).to_string())
                .collect();
            format!("{}({})", name, top_indices.join(","))
        });
        scalars.chain(arrays).collect()
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
        // The cursor is now at the start of the top line
        self.column = 0;
        Ok(())
    }

    /// Execute MODE statement - select a screen mode, clear the text screen
    /// and reset the graphics. A number from 8 to 255 selects mode `n AND 7`,
    /// as on a BBC Micro Model B. A number that is not 0 to 255 gives Bad MODE.
    fn execute_mode(&mut self, mode: &Expression) -> Result<()> {
        let number = self.eval_integer(mode)?;
        if !(0..=255).contains(&number) {
            return Err(BBCBasicError::BadMode);
        }
        let screen_mode = ScreenMode::from_number(number & 7).ok_or(BBCBasicError::BadMode)?;

        self.screen_mode = screen_mode;
        self.graphics.reset_for_mode(&screen_mode);
        self.text_foreground = screen_mode.colours - 1;
        self.text_background = 0;
        self.execute_cls()
    }

    /// Execute COLOUR statement - set the text foreground or background colour
    fn execute_colour(&mut self, colour: &Expression) -> Result<()> {
        let value = self.eval_integer(colour)?;
        self.set_text_colour(value as u8);
        Ok(())
    }

    /// Set a text colour from 1 byte, as COLOUR and VDU 17 do. A byte below
    /// 128 sets the foreground and a byte from 128 sets the background. The
    /// colour is reduced to the colour count of the current mode, as on a BBC
    /// Micro. No escape code goes to the output: colour output to a real
    /// terminal is out of scope (issue #101).
    fn set_text_colour(&mut self, byte: u8) {
        let colour = (byte & 0x7F) % self.screen_mode.colours;
        if byte < 128 {
            self.text_foreground = colour;
        } else {
            self.text_background = colour;
        }
    }

    /// The current screen mode
    pub fn screen_mode(&self) -> ScreenMode {
        self.screen_mode
    }

    /// The current text foreground colour
    pub fn text_foreground(&self) -> u8 {
        self.text_foreground
    }

    /// The current text background colour
    pub fn text_background(&self) -> u8 {
        self.text_background
    }

    /// Execute PLOT statement - plot with mode code
    fn execute_plot(&mut self, mode: &Expression, x: &Expression, y: &Expression) -> Result<()> {
        let mode_val = self.eval_integer(mode)?;
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics.plot(mode_val as u8, x_val, y_val);
        }
        Ok(())
    }

    /// Execute MOVE statement - move graphics cursor
    fn execute_move(&mut self, x: &Expression, y: &Expression) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics.move_to(x_val, y_val);
        }
        Ok(())
    }

    /// Execute DRAW statement - draw line to coordinates
    fn execute_draw(&mut self, x: &Expression, y: &Expression) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics.draw_line_to(x_val, y_val);
        }
        Ok(())
    }

    /// Execute CIRCLE statement - draw a circle
    fn execute_circle(
        &mut self,
        x: &Expression,
        y: &Expression,
        radius: &Expression,
    ) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;
        let radius_val = self.eval_integer(radius)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics.draw_circle(x_val, y_val, radius_val);
        }
        Ok(())
    }

    /// Execute GCOL statement - set graphics color
    fn execute_gcol(&mut self, mode: &Expression, color: &Expression) -> Result<()> {
        let mode_val = self.eval_integer(mode)?;
        let color_val = self.eval_integer(color)?;

        self.graphics.set_color(mode_val as u8, color_val as u8);
        Ok(())
    }

    /// Execute CLG statement - clear graphics screen
    fn execute_clg(&mut self) -> Result<()> {
        self.graphics.clear();
        Ok(())
    }

    /// Execute ELLIPSE statement - draw an ellipse
    fn execute_ellipse(
        &mut self,
        x: &Expression,
        y: &Expression,
        major: &Expression,
        minor: &Expression,
    ) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;
        let major_val = self.eval_integer(major)?;
        let minor_val = self.eval_integer(minor)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics
                .draw_ellipse(x_val, y_val, major_val, minor_val);
        }
        Ok(())
    }

    /// Execute RECTANGLE statement - draw a rectangle
    fn execute_rectangle(
        &mut self,
        x1: &Expression,
        y1: &Expression,
        width: &Expression,
        height: &Expression,
        filled: bool,
    ) -> Result<()> {
        let x1_val = self.eval_integer(x1)?;
        let y1_val = self.eval_integer(y1)?;
        let width_val = self.eval_integer(width)?;
        let height_val = self.eval_integer(height)?;

        let x2_val = x1_val + width_val;
        let y2_val = y1_val + height_val;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics
                .draw_rectangle(x1_val, y1_val, x2_val, y2_val, filled);
        }
        Ok(())
    }

    /// Execute FILL statement - flood fill from coordinates
    fn execute_fill(&mut self, x: &Expression, y: &Expression) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            self.graphics.flood_fill(x_val, y_val);
        }
        Ok(())
    }

    /// Execute ORIGIN statement - set graphics origin
    fn execute_origin(&mut self, x: &Expression, y: &Expression) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;

        self.graphics.set_origin(x_val, y_val);
        Ok(())
    }

    /// Get graphics output as string (for display or testing)
    pub fn get_graphics_output(&self) -> String {
        self.graphics.render()
    }

    /// Set the size in bytes of the stored program. The runner sets it
    /// before a run and before a typed line, so that TOP agrees with it.
    pub fn set_program_size(&mut self, size: usize) {
        self.program_size = size;
    }

    /// The first free address after the stored program
    fn top(&self) -> i32 {
        self.memory.get_page() as i32 + self.program_size as i32
    }

    /// Evaluate an expression to an integer value
    pub fn eval_integer(&mut self, expr: &Expression) -> Result<i32> {
        // An operation that can give a fraction part, such as `/`, uses real
        // arithmetic. Only the result is truncated, as in BBC BASIC.
        if matches!(
            expr,
            Expression::BinaryOp { .. } | Expression::UnaryOp { .. }
        ) && self.is_real_expr(expr)
        {
            return real_to_integer(self.eval_real(expr)?);
        }

        match expr {
            Expression::Integer(val) => Ok(*val),
            Expression::Real(val) => real_to_integer(*val),
            Expression::Variable(name) => {
                // Check for pseudo-variables first
                if name == "TIME" {
                    // TIME returns centiseconds since the executor started.
                    // A count from the Unix epoch does not fit in an i32.
                    let centiseconds = self.start_time.elapsed().as_millis() / 10;
                    return Ok(centiseconds as i32);
                } else if name == "HIMEM" {
                    // HIMEM returns top of available memory
                    return Ok(self.memory.get_himem() as i32);
                } else if name == "PAGE" {
                    // PAGE returns the start of program memory
                    return Ok(self.memory.get_page() as i32);
                } else if name == "TOP" || name == "LOMEM" {
                    // TOP returns the first free address after the program.
                    // LOMEM, the start of the variables, is equal to TOP.
                    return Ok(self.top());
                } else if name == "ERR" {
                    // ERR returns the last error number (0 if no error)
                    return Ok(self
                        .last_error
                        .as_ref()
                        .map(|e| e.error_number)
                        .unwrap_or(0));
                } else if name == "ERL" {
                    // ERL returns the line number where the last error occurred (0 if no error)
                    return Ok(self
                        .last_error
                        .as_ref()
                        .map(|e| e.error_line as i32)
                        .unwrap_or(0));
                }

                if name.ends_with('%') {
                    self.variables
                        .get_integer_var(name)
                        .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone()))
                } else {
                    // Try as real variable first, then as integer (for loop vars without % suffix)
                    if let Some(real_val) = self.variables.get_real_var(name) {
                        real_to_integer(real_val)
                    } else if let Some(int_val) = self.variables.get_integer_var(name) {
                        Ok(int_val)
                    } else {
                        Err(BBCBasicError::NoSuchVariable(name.clone()))
                    }
                }
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONVALID") => {
                let text = self.eval_json_argument(name, indices)?;
                Ok(if crate::json::is_valid(&text) { -1 } else { 0 })
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONLEN") => {
                let (json, path) = self.eval_json_path_arguments(name, indices)?;
                i32::try_from(crate::json::length(&json, &path)?).map_err(|_| BBCBasicError::TooBig)
            }
            Expression::ArrayAccess { name, .. } if self.is_json_call(name, "JSONGET") => {
                real_to_integer(self.eval_real(expr)?)
            }
            Expression::ArrayAccess { name, indices } => {
                use crate::variables::Variable;
                // Evaluate all indices to integers
                let mut index_values = Vec::with_capacity(indices.len());
                for index_expr in indices {
                    let idx = self.eval_integer(index_expr)?;
                    if idx < 0 {
                        return Err(BBCBasicError::SubscriptOutOfRange);
                    }
                    index_values.push(idx as usize);
                }
                // Get the array element
                let element = self.variables.get_array_element(name, &index_values)?;
                match element {
                    Variable::Integer(val) => Ok(val),
                    Variable::Real(val) => real_to_integer(val),
                    _ => Err(BBCBasicError::TypeMismatch),
                }
            }
            Expression::BinaryOp { op, left, right } => {
                use crate::parser::BinaryOperator;

                // A comparison uses the full values, so that the fraction
                // part of a real operand has an effect on the result.
                // An i32 converts to an f64 with no loss.
                if matches!(
                    op,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::LessThan
                        | BinaryOperator::LessThanOrEqual
                        | BinaryOperator::GreaterThan
                        | BinaryOperator::GreaterThanOrEqual
                ) {
                    // If one operand is a string, compare the two operands as
                    // strings. A numeric operand then gives a type mismatch.
                    // The String order is the ASCII order that BBC BASIC uses.
                    if self.is_string_expr(left) || self.is_string_expr(right) {
                        let left_val = self.eval_string(left)?;
                        let right_val = self.eval_string(right)?;
                        let is_true = match op {
                            BinaryOperator::Equal => left_val == right_val,
                            BinaryOperator::NotEqual => left_val != right_val,
                            BinaryOperator::LessThan => left_val < right_val,
                            BinaryOperator::LessThanOrEqual => left_val <= right_val,
                            BinaryOperator::GreaterThan => left_val > right_val,
                            _ => left_val >= right_val,
                        };
                        return Ok(if is_true { -1 } else { 0 });
                    }

                    let left_val = self.eval_number(left)?;
                    let right_val = self.eval_number(right)?;
                    let is_true = match op {
                        BinaryOperator::Equal => left_val == right_val,
                        BinaryOperator::NotEqual => left_val != right_val,
                        BinaryOperator::LessThan => left_val < right_val,
                        BinaryOperator::LessThanOrEqual => left_val <= right_val,
                        BinaryOperator::GreaterThan => left_val > right_val,
                        _ => left_val >= right_val,
                    };
                    // BBC BASIC convention: -1 for true, 0 for false
                    return Ok(if is_true { -1 } else { 0 });
                }

                let left_val = self.eval_integer(left)?;
                let right_val = self.eval_integer(right)?;

                match op {
                    // An integer overflow gives "Too big". BBC BASIC changes to
                    // real arithmetic here, which this interpreter does not do.
                    BinaryOperator::Add => {
                        left_val.checked_add(right_val).ok_or(BBCBasicError::TooBig)
                    }
                    BinaryOperator::Subtract => {
                        left_val.checked_sub(right_val).ok_or(BBCBasicError::TooBig)
                    }
                    BinaryOperator::Multiply => {
                        left_val.checked_mul(right_val).ok_or(BBCBasicError::TooBig)
                    }
                    BinaryOperator::IntegerDivide => {
                        if right_val == 0 {
                            Err(BBCBasicError::DivisionByZero)
                        } else {
                            left_val.checked_div(right_val).ok_or(BBCBasicError::TooBig)
                        }
                    }
                    BinaryOperator::Modulo => {
                        if right_val == 0 {
                            Err(BBCBasicError::DivisionByZero)
                        } else {
                            // The smallest integer MOD -1 is 0, with no overflow.
                            Ok(left_val.wrapping_rem(right_val))
                        }
                    }
                    // A negative exponent or an overflow gives a real result,
                    // which converts to an integer or gives "Too big".
                    BinaryOperator::Power => match u32::try_from(right_val)
                        .ok()
                        .and_then(|exponent| left_val.checked_pow(exponent))
                    {
                        Some(value) => Ok(value),
                        None => real_to_integer(f64::from(left_val).powf(f64::from(right_val))),
                    },
                    // Logical operators
                    BinaryOperator::And => Ok(left_val & right_val),
                    BinaryOperator::Or => Ok(left_val | right_val),
                    BinaryOperator::Eor => Ok(left_val ^ right_val),
                    // Bitwise shift operators
                    BinaryOperator::LeftShift => {
                        if right_val < 0 {
                            return Err(BBCBasicError::IllegalFunction);
                        }
                        // A shift of 32 places or more moves out all bits.
                        Ok(left_val.checked_shl(right_val as u32).unwrap_or(0))
                    }
                    BinaryOperator::RightShift => {
                        if right_val < 0 {
                            return Err(BBCBasicError::IllegalFunction);
                        }
                        // A shift of 32 places or more leaves only the sign fill.
                        Ok(left_val
                            .checked_shr(right_val as u32)
                            .unwrap_or(if left_val < 0 { -1 } else { 0 }))
                    }
                    _ => Err(BBCBasicError::IllegalFunction),
                }
            }
            Expression::UnaryOp { op, operand } => {
                use crate::parser::UnaryOperator;
                if let UnaryOperator::Indirect(width) = op {
                    return self.read_memory(*width, operand);
                }
                let val = self.eval_integer(operand)?;
                match op {
                    UnaryOperator::Minus => val.checked_neg().ok_or(BBCBasicError::TooBig),
                    UnaryOperator::Plus => Ok(val),
                    UnaryOperator::Not => Ok(if val == 0 { -1 } else { 0 }),
                    UnaryOperator::Indirect(_) => unreachable!("memory read returned above"),
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
                // Check for pseudo-variables first
                match name.as_str() {
                    "PI" => return Ok(std::f64::consts::PI),
                    "TIME" | "HIMEM" | "LOMEM" | "PAGE" | "TOP" | "ERR" | "ERL" => {
                        return self.eval_integer(expr).map(f64::from);
                    }
                    _ => {}
                }

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
            Expression::ArrayAccess { name, .. }
                if self.is_json_call(name, "JSONVALID") || self.is_json_call(name, "JSONLEN") =>
            {
                self.eval_integer(expr).map(f64::from)
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONGET") => {
                let (json, path) = self.eval_json_path_arguments(name, indices)?;
                Ok(crate::json::get_number(&json, &path)?)
            }
            Expression::ArrayAccess { name, indices } => {
                use crate::variables::Variable;
                // Evaluate all indices to integers
                let mut index_values = Vec::with_capacity(indices.len());
                for index_expr in indices {
                    let idx = self.eval_integer(index_expr)?;
                    if idx < 0 {
                        return Err(BBCBasicError::SubscriptOutOfRange);
                    }
                    index_values.push(idx as usize);
                }
                // Get the array element
                let element = self.variables.get_array_element(name, &index_values)?;
                match element {
                    Variable::Real(val) => Ok(val),
                    Variable::Integer(val) => Ok(val as f64),
                    _ => Err(BBCBasicError::TypeMismatch),
                }
            }
            Expression::BinaryOp { op, left, right } => {
                use crate::parser::BinaryOperator;

                // The other operators give an integer result
                if !matches!(
                    op,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Power
                ) {
                    return self.eval_integer(expr).map(f64::from);
                }

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
                if let UnaryOperator::Indirect(width) = op {
                    return self.read_memory(*width, operand).map(f64::from);
                }
                let val = self.eval_real(operand)?;
                match op {
                    UnaryOperator::Minus => Ok(-val),
                    UnaryOperator::Plus => Ok(val),
                    UnaryOperator::Not => Ok(if val == 0.0 { -1.0 } else { 0.0 }),
                    UnaryOperator::Indirect(_) => unreachable!("memory read returned above"),
                }
            }
            Expression::FunctionCall { name, args } => {
                if self.is_real_expr(expr) {
                    self.eval_function_real(name, args)
                } else {
                    self.eval_function_int(name, args).map(f64::from)
                }
            }
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Evaluate a numeric expression one time, as a real value when it can
    /// have a fraction part and as an integer value when it cannot.
    fn eval_number(&mut self, expr: &Expression) -> Result<f64> {
        if self.is_real_expr(expr) {
            self.eval_real(expr)
        } else {
            self.eval_integer(expr).map(f64::from)
        }
    }

    /// Tell if an expression can give a value with a fraction part.
    fn is_real_expr(&self, expr: &Expression) -> bool {
        use crate::parser::{BinaryOperator, UnaryOperator};
        match expr {
            Expression::Real(_) => true,
            Expression::Integer(_) | Expression::String(_) => false,
            Expression::Variable(name) => {
                name == "PI"
                    || (!name.ends_with('%')
                        && !name.ends_with('$')
                        && self.variables.get_real_var(name).is_some())
            }
            Expression::ArrayAccess { name, .. } => !name.ends_with('%') && !name.ends_with('$'),
            Expression::BinaryOp { op, left, right } => match op {
                // In BBC BASIC, `/` always gives a real value
                BinaryOperator::Divide => true,
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Power => self.is_real_expr(left) || self.is_real_expr(right),
                _ => false,
            },
            Expression::UnaryOp { op, operand } => match op {
                UnaryOperator::Plus | UnaryOperator::Minus => self.is_real_expr(operand),
                UnaryOperator::Not | UnaryOperator::Indirect(_) => false,
            },
            Expression::FunctionCall { name, args } => {
                if self.functions.contains_key(name) {
                    return true;
                }
                match name.as_str() {
                    "SIN" | "COS" | "TAN" | "ATN" | "SQR" | "SQRT" | "ACS" | "ASN" | "EXP"
                    | "LN" | "LOG" | "DEG" | "RAD" | "PI" | "VAL" => true,
                    // RND with no bracket gives an integer
                    "RND" => !args.is_empty(),
                    "ABS" => args.first().is_some_and(|arg| self.is_real_expr(arg)),
                    _ => false,
                }
            }
        }
    }

    /// Tell if an expression gives a string value.
    fn is_string_expr(&self, expr: &Expression) -> bool {
        self.is_string_expr_guarded(expr, &mut Vec::new())
    }

    /// Tell if an expression gives a string value. `open` holds the names of
    /// the user functions whose bodies are in the check now, so that a
    /// function that calls itself gives "not a string" and does not loop.
    fn is_string_expr_guarded<'a>(&'a self, expr: &Expression, open: &mut Vec<&'a str>) -> bool {
        use crate::parser::BinaryOperator;
        match expr {
            Expression::String(_) => true,
            Expression::FunctionCall { name, .. } if !name.ends_with('$') => {
                // The body of a single-line function has a type with no run.
                // The type of a multi-line function result is known only after
                // its body runs, so it is "not a string" here (a known limit).
                match self.functions.get_key_value(name) {
                    Some((key, func)) if !open.contains(&key.as_str()) => match &func.body {
                        FunctionBody::Expression(body) => {
                            open.push(key);
                            let is_string = self.is_string_expr_guarded(body, open);
                            open.pop();
                            is_string
                        }
                        FunctionBody::Lines(_) => false,
                    },
                    _ => name == "STR$~",
                }
            }
            Expression::Variable(name)
            | Expression::ArrayAccess { name, .. }
            | Expression::FunctionCall { name, .. } => name.ends_with('$') || name == "STR$~",
            Expression::BinaryOp {
                op: BinaryOperator::Add | BinaryOperator::StringConcat,
                left,
                right,
            } => {
                self.is_string_expr_guarded(left, open) || self.is_string_expr_guarded(right, open)
            }
            _ => false,
        }
    }

    /// Evaluate an expression to a string value
    fn eval_string(&mut self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::String(val) => Ok(val.clone()),
            // TIME$ is the date and time in UTC. TIME$ is from the BBC Master, not the Model B.
            Expression::Variable(name) if name == "TIME$" => {
                let seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs())
                    .unwrap_or(0);
                Ok(format_time_string(seconds))
            }
            // REPORT$ is an extension. A variable named REPORT$ keeps its old meaning.
            Expression::Variable(name)
                if name == "REPORT$" && self.variables.get_string_var(name).is_none() =>
            {
                self.eval_extension_string_function(name, &[])
            }
            Expression::Variable(name) => self
                .variables
                .get_string_var(name)
                .map(|s| s.to_string())
                .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone())),
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "LOADJSON$") => {
                let file = self.eval_json_argument(name, indices)?;
                let text = crate::json::load_file(&file)?;
                if text.len() > crate::variables::MAX_STRING_LENGTH {
                    return Err(BBCBasicError::StringTooLong);
                }
                Ok(text)
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONGET$") => {
                let (json, path) = self.eval_json_path_arguments(name, indices)?;
                let text = crate::json::get_text(&json, &path)?;
                if text.len() > crate::variables::MAX_STRING_LENGTH {
                    return Err(BBCBasicError::StringTooLong);
                }
                Ok(text)
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONTYPE$") => {
                let (json, path) = self.eval_json_path_arguments(name, indices)?;
                Ok(crate::json::type_name(&json, &path)?.to_string())
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONKEY$") => {
                let [json, path, n] = indices.as_slice() else {
                    return Err(json_argument_count_error(name, 3));
                };
                let (json, path) = (self.eval_string(json)?, self.eval_string(path)?);
                let n = self.eval_integer(n)?;
                Ok(crate::json::key_at(&json, &path, n)?)
            }
            Expression::ArrayAccess { name, indices }
                if self.is_json_call(name, "JSONSET$") || self.is_json_call(name, "JSONADD$") =>
            {
                let [json, path, value] = indices.as_slice() else {
                    return Err(json_argument_count_error(name, 3));
                };
                let (json, path) = (self.eval_string(json)?, self.eval_string(path)?);
                let value = if self.is_string_expr(value) {
                    serde_json::Value::String(self.eval_string(value)?)
                } else {
                    crate::json::number_value(self.eval_real(value)?)?
                };
                let text = if name == "JSONSET$" {
                    crate::json::set_value(&json, &path, value)?
                } else {
                    crate::json::add_value(&json, &path, value)?
                };
                json_text_in_limit(text)
            }
            Expression::ArrayAccess { name, indices }
                if self.is_json_call(name, "JSONSETJSON$") =>
            {
                let [json, path, fragment] = indices.as_slice() else {
                    return Err(json_argument_count_error(name, 3));
                };
                let (json, path) = (self.eval_string(json)?, self.eval_string(path)?);
                let value = crate::json::fragment_value(&self.eval_string(fragment)?)?;
                json_text_in_limit(crate::json::set_value(&json, &path, value)?)
            }
            Expression::ArrayAccess { name, indices } if self.is_json_call(name, "JSONDEL$") => {
                let (json, path) = self.eval_json_path_arguments(name, indices)?;
                json_text_in_limit(crate::json::delete_value(&json, &path)?)
            }
            // LISTDIR$(path$) is an extension. The parser cannot tell it from
            // an array, so an array named LISTDIR$ keeps its old meaning.
            Expression::ArrayAccess { name, indices }
                if name == "LISTDIR$" && !self.variables.has_array(name) =>
            {
                let [path] = indices.as_slice() else {
                    return Err(BBCBasicError::SyntaxError {
                        message: "LISTDIR$ requires 1 argument".to_string(),
                        line: None,
                    });
                };
                crate::filesystem::list_directory(&self.eval_string(path)?)
            }
            // UPPER$(s$) and LOWER$(s$) are extensions. The parser gives them as
            // an array access, so an array of the same name keeps its old meaning.
            Expression::ArrayAccess { name, indices }
                if (name == "UPPER$" || name == "LOWER$") && !self.variables.has_array(name) =>
            {
                self.eval_extension_string_function(name, indices)
            }
            Expression::ArrayAccess { name, indices } => {
                use crate::variables::Variable;
                // Evaluate all indices to integers
                let mut index_values = Vec::with_capacity(indices.len());
                for index_expr in indices {
                    let idx = self.eval_integer(index_expr)?;
                    if idx < 0 {
                        return Err(BBCBasicError::SubscriptOutOfRange);
                    }
                    index_values.push(idx as usize);
                }
                // Get the array element
                let element = self.variables.get_array_element(name, &index_values)?;
                match element {
                    Variable::String(val) => Ok(val),
                    _ => Err(BBCBasicError::TypeMismatch),
                }
            }
            Expression::FunctionCall { name, args } => self.eval_function_string(name, args),
            // String concatenation with +
            Expression::BinaryOp {
                op: crate::parser::BinaryOperator::Add | crate::parser::BinaryOperator::StringConcat,
                left,
                right,
            } => {
                let mut joined = self.eval_string(left)?;
                joined.push_str(&self.eval_string(right)?);
                Ok(joined)
            }
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
                val.checked_abs().ok_or(BBCBasicError::TooBig)
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
                // BBC BASIC returns 0 for non-numeric strings. A real value is truncated.
                real_to_integer(parse_leading_number(&s))
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
            "OPENUP" => {
                // Open file for reading and writing, returns file handle
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "OPENUP requires 1 argument (filename)".to_string(),
                        line: None,
                    });
                }
                let filename = self.eval_string(&args[0])?;
                self.open_file_for_update(&filename)
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
            "BGET" | "PTR" | "EXT" => {
                // BGET#, PTR# and EXT#: read a byte, the file position or the file size
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: format!("{} requires 1 argument (file handle)", name),
                        line: None,
                    });
                }
                let handle = self.eval_integer(&args[0])?;
                match name {
                    "BGET" => self.bget(handle),
                    "PTR" => self.get_ptr(handle),
                    _ => self.get_ext(handle),
                }
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
            "POINT" => {
                // POINT(x, y) - Read pixel state at coordinates
                // Returns -1 (TRUE) if pixel is set, 0 (FALSE) if not set
                if args.len() != 2 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "POINT requires 2 arguments (x, y)".to_string(),
                        line: None,
                    });
                }
                let x = self.eval_integer(&args[0])?;
                let y = self.eval_integer(&args[1])?;
                // Returns -1 if pixel is set, 0 if not (BBC BASIC boolean convention)
                Ok(if self.graphics.get_pixel(x, y) == Some(true) {
                    -1
                } else {
                    0
                })
            }
            // RND with no bracket: a random integer over the full 32-bit range
            "RND" if args.is_empty() => Ok(self.rng.borrow_mut().gen::<i32>()),
            // POS gives the print column. COUNT gives the characters printed
            // since the last new line. This build has one column counter, so
            // the two values are the same number.
            "POS" | "COUNT" if args.is_empty() => Ok(self.column as i32),
            // VPOS gives the print row. There is no screen model yet, so it gives 0.
            "VPOS" if args.is_empty() => Ok(0),
            // ADVAL(0) gives the fire buttons, ADVAL(n) with n > 0 gives an
            // analogue channel, and ADVAL(n) with n < 0 gives the state of a
            // buffer. This build has no analogue hardware, so each one gives 0.
            "ADVAL" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ADVAL requires 1 argument".to_string(),
                        line: None,
                    });
                }
                self.eval_integer(&args[0])?;
                Ok(0)
            }
            // GET waits for one key and gives its code. At the end of input it gives -1.
            "GET" if args.is_empty() => Ok(self.next_key().map_or(-1, |key| key as i32)),
            // INKEY(n) waits for a maximum of n centiseconds for a key and
            // gives its code. If no key comes, it gives -1.
            // A negative n (a key scan) gives 0: see inkey_limit.
            "INKEY" if args.len() == 1 => {
                let time = self.eval_integer(&args[0])?;
                Ok(match inkey_limit(time) {
                    Some(limit) => self.next_key_within(limit).map_or(-1, |key| key as i32),
                    None => 0,
                })
            }
            // Real-only functions should not be called as integers
            "SIN" | "COS" | "TAN" | "ATN" | "SQR" | "SQRT" | "ACS" | "ASN" | "EXP" | "LN"
            | "LOG" | "DEG" | "RAD" | "PI" | "RND" => Err(BBCBasicError::TypeMismatch),
            _ if name.starts_with("FN") => Err(BBCBasicError::NoSuchFnProc(name.to_string())),
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
                let radians = self.eval_real(&args[0])?;
                Ok(radians.sin())
            }
            "COS" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "COS requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let radians = self.eval_real(&args[0])?;
                Ok(radians.cos())
            }
            "TAN" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "TAN requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let radians = self.eval_real(&args[0])?;
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
                Ok(val.atan())
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
                // BBC BASIC reads the leading number and gives 0 for non-numeric strings
                Ok(parse_leading_number(&s))
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
                if !(-1.0..=1.0).contains(&val) {
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
                if !(-1.0..=1.0).contains(&val) {
                    return Err(BBCBasicError::IllegalFunction);
                }
                Ok(val.asin())
            }
            _ if name.starts_with("FN") => Err(BBCBasicError::NoSuchFnProc(name.to_string())),
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
            // GET$ waits for one key and gives it. At the end of input it gives "".
            "GET$" if args.is_empty() => Ok(self.next_key().map(String::from).unwrap_or_default()),
            // INKEY$(n) waits for a maximum of n centiseconds for a key and
            // gives it. If no key comes, or n is negative, it gives "".
            "INKEY$" if args.len() == 1 => {
                let time = self.eval_integer(&args[0])?;
                Ok(inkey_limit(time)
                    .and_then(|limit| self.next_key_within(limit))
                    .map(String::from)
                    .unwrap_or_default())
            }
            "CHR$" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "CHR$ requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let code = self.eval_integer(&args[0])?;
                if !(0..=255).contains(&code) {
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
            "STR$~" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "STR$~ requires 1 argument".to_string(),
                        line: None,
                    });
                }
                Ok(format_hex(real_to_integer(self.eval_number(&args[0])?)?))
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
                    Expression::Real(val) => Ok(format_number(*val)),
                    Expression::Integer(val) => Ok(val.to_string()),
                    _ => {
                        // Try to evaluate - prefer real if it works
                        if let Ok(real_val) = self.eval_real(&args[0]) {
                            // Check if it's actually an integer value
                            if real_val.fract() == 0.0 {
                                Ok((real_val as i32).to_string())
                            } else {
                                Ok(format_number(real_val))
                            }
                        } else if let Ok(int_val) = self.eval_integer(&args[0]) {
                            Ok(int_val.to_string())
                        } else {
                            Err(BBCBasicError::TypeMismatch)
                        }
                    }
                }
            }
            _ => {
                // Check for extension functions (UPPER$, LOWER$, STRING$, REPORT$)
                // These are NOT standard BBC BASIC functions
                self.eval_extension_string_function(name, args)
            }
        }
    }

    /// Tell if `name(...)` is a call of the JSON function `function`. The
    /// parser gives `name(...)` as an array access. An array of the same
    /// name in the program has priority, so that old programs do not change.
    fn is_json_call(&self, name: &str, function: &str) -> bool {
        name == function && !self.variables.has_array(name)
    }

    /// Evaluate the one text argument of a JSON function.
    fn eval_json_argument(&mut self, name: &str, args: &[Expression]) -> Result<String> {
        match args {
            [arg] => self.eval_string(arg),
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("{} requires 1 argument", name),
                line: None,
            }),
        }
    }

    /// Evaluate the text and path arguments of a JSON path function.
    fn eval_json_path_arguments(
        &mut self,
        name: &str,
        args: &[Expression],
    ) -> Result<(String, String)> {
        match args {
            [json, path] => Ok((self.eval_string(json)?, self.eval_string(path)?)),
            _ => Err(json_argument_count_error(name, 2)),
        }
    }

    /// Evaluate a non-standard extension string function
    ///
    /// These functions are NOT part of the original BBC BASIC specification
    /// but are provided as modern extensions.
    fn eval_extension_string_function(
        &mut self,
        name: &str,
        args: &[Expression],
    ) -> Result<String> {
        let error_msg = self
            .last_error
            .as_ref()
            .map(|e| e.message.clone())
            .unwrap_or_default();

        match name {
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
                let count = self.eval_integer(&args[0])?;
                let s = self.eval_string(&args[1])?;

                // STRING$(n, string) repeats the full string n times. A count
                // below 0 or an empty string gives an empty string. A result
                // longer than the string limit gives an error before the
                // string is made.
                let count = i64::from(count.max(0));
                if count * s.chars().count() as i64 > crate::variables::MAX_STRING_LENGTH as i64 {
                    return Err(BBCBasicError::StringTooLong);
                }
                Ok(s.repeat(count as usize))
            }
            "REPORT$" => {
                if !args.is_empty() {
                    return Err(BBCBasicError::SyntaxError {
                        message: "REPORT$ takes no arguments".to_string(),
                        line: None,
                    });
                }
                Ok(error_msg)
            }
            _ if name.starts_with("FN") => Err(BBCBasicError::NoSuchFnProc(name.to_string())),
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
    /// Returns Some(FOR position) if should loop back, None if loop completed
    pub fn should_loop_back(&self) -> Option<Position> {
        self.next_loop_line
    }

    /// Set the position of a FOR loop (called when FOR is executed)
    pub fn set_for_loop_line(&mut self, position: Position) {
        if let Some(loop_state) = self.for_loops.last_mut() {
            loop_state.3 = position;
        }
    }

    /// Push a REPEAT position onto the repeat stack
    pub fn push_repeat(&mut self, position: Position) {
        self.repeat_stack.push(position);
    }

    /// Evaluate UNTIL condition and return the REPEAT position if we should loop back
    pub fn check_until(&mut self, condition: &Expression) -> Result<Option<Position>> {
        // UNTIL must have an open REPEAT loop
        if self.repeat_stack.is_empty() {
            return Err(BBCBasicError::NoRepeat);
        }

        // Evaluate the condition
        let result = self.eval_integer(condition)?;

        if result == 0 {
            // Condition is false - loop back to REPEAT
            // Return the REPEAT position but keep it on stack (don't pop yet)
            Ok(self.repeat_stack.last().copied())
        } else {
            // Condition is true - exit loop
            self.repeat_stack.pop();
            Ok(None)
        }
    }

    /// Push a WHILE position onto the while stack and check condition
    /// Returns Some(position) if condition is TRUE (continue to loop body)
    /// Returns None if condition is FALSE (skip loop body)
    pub fn push_while(
        &mut self,
        position: Position,
        condition: &Expression,
    ) -> Result<Option<Position>> {
        // Evaluate the condition
        let result = self.eval_integer(condition)?;

        if result != 0 {
            // Condition is true - enter loop body
            self.while_stack.push(position);
            Ok(Some(position))
        } else {
            // Condition is false - skip loop body
            Ok(None)
        }
    }

    /// Handle ENDWHILE - return the WHILE position if we should loop back
    pub fn check_endwhile(&mut self, condition: &Expression) -> Result<Option<Position>> {
        // Evaluate the condition
        let result = self.eval_integer(condition)?;

        if result != 0 {
            // Condition is still true - loop back to WHILE
            // Return the WHILE position but keep it on stack (don't pop yet)
            Ok(self.while_stack.last().copied())
        } else {
            // Condition is false - exit loop
            self.while_stack.pop();
            Ok(None)
        }
    }

    /// Get the current WHILE position without popping (for ENDWHILE to retrieve condition)
    pub fn check_endwhile_get_while_line(&self) -> Option<Position> {
        self.while_stack.last().copied()
    }

    /// Push a return address onto the GOSUB stack
    pub fn push_gosub_return(&mut self, position: Position) {
        self.return_stack.push((ReturnKind::Gosub, position));
    }

    /// Pop a return address from the GOSUB stack. With a PROC call on top,
    /// give an error and change nothing.
    pub fn pop_gosub_return(&mut self) -> Result<Position> {
        match self.return_stack.last() {
            Some(&(ReturnKind::Gosub, position)) => {
                self.return_stack.pop();
                Ok(position)
            }
            _ => Err(BBCBasicError::BadCall),
        }
    }

    /// Push the position of a PROC call onto the return stack
    pub fn push_proc_return(&mut self, position: Position) {
        self.return_stack.push((ReturnKind::Proc, position));
    }

    /// Pop the newest PROC call, and the GOSUBs that this PROC left open.
    /// With no PROC call on the stack, give an error and change nothing.
    pub fn pop_proc_return(&mut self) -> Result<Position> {
        let index = self
            .return_stack
            .iter()
            .rposition(|&(kind, _)| kind == ReturnKind::Proc)
            .ok_or(BBCBasicError::BadCall)?;
        let (_, position) = self.return_stack[index];
        self.return_stack.truncate(index);
        Ok(position)
    }

    /// Define a procedure at the position of its DEF PROC statement
    pub fn define_procedure(&mut self, name: String, position: Position, params: Vec<String>) {
        self.procedures
            .insert(name, ProcedureDefinition { position, params });
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
        let frame = self.local_stack.last_mut().ok_or(BBCBasicError::NotLocal)?;

        // LOCAL NAME() saves the array only, and makes it undimensioned.
        // A second LOCAL for the same name in this frame keeps the first saved entry.
        if let Some(array_name) = name.strip_suffix("()") {
            let variables = &self.variables;
            frame
                .saved_arrays
                .entry(array_name.to_string())
                .or_insert_with(|| variables.save_array(array_name));
            self.variables.declare_local_array(array_name.to_string());
            return Ok(());
        }

        // Save current value (or None if doesn't exist).
        // A second LOCAL for the same name in this frame keeps the first saved value.
        let variables = &self.variables;
        frame
            .saved_variables
            .entry(name.to_string())
            .or_insert_with(|| variables.get_variable(name).cloned());

        // The local variable starts with the zero value of its type
        self.variables
            .restore_variable(name.to_string(), Self::zero_value(name));

        Ok(())
    }

    /// The value that LOCAL gives a scalar: 0, or an empty string
    fn zero_value(name: &str) -> Variable {
        if name.ends_with('%') {
            Variable::Integer(0)
        } else if name.ends_with('$') {
            Variable::String(String::new())
        } else {
            Variable::Real(0.0)
        }
    }

    /// The number of open local scopes
    pub fn local_depth(&self) -> usize {
        self.local_stack.len()
    }

    /// Exit local scopes until `depth` scopes are open, and restore their
    /// saved variables (after a run that stopped inside a PROC or FN)
    pub fn exit_local_scopes_to(&mut self, depth: usize) {
        while self.local_stack.len() > depth {
            // The stack is not empty, so exit_local_scope cannot fail
            let _ = self.exit_local_scope();
        }
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

        // Restore all saved scalars. As on a BBC Micro, a scalar that did not
        // exist before keeps the zero value that LOCAL gave it.
        for (name, saved_value) in frame.saved_variables {
            let var = saved_value.unwrap_or_else(|| Self::zero_value(&name));
            self.variables.restore_variable(name, var);
        }

        // Restore all saved arrays. An array that did not exist before is removed.
        for (name, saved_array) in frame.saved_arrays {
            self.variables.restore_array(name, saved_array);
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

    /// Clear all function definitions and the function body runner (used
    /// at the start of a run, so that a removed DEF FN is not callable)
    pub fn clear_functions(&mut self) {
        self.functions.clear();
        self.function_body_runner = BodyRunnerSlot::default();
    }

    /// Remove the multi-line functions. A typed line uses this before it
    /// defines the functions of the stored program again. A function that
    /// the user defined in immediate mode stays.
    pub fn clear_multi_line_functions(&mut self) {
        self.functions
            .retain(|_, function| !matches!(function.body, FunctionBody::Lines(_)));
    }

    /// Clear the GOSUB, FOR, REPEAT and WHILE stacks (used at the start of a
    /// run, so that a position from an earlier run is not used)
    pub fn clear_control_stacks(&mut self) {
        self.return_stack.clear();
        self.for_loops.clear();
        self.repeat_stack.clear();
        self.while_stack.clear();
        self.next_loop_line = None;
    }

    /// Set the number of files that a program can open at one time. A test
    /// lowers it, because a test cannot open 255 real files.
    pub fn set_open_file_limit(&mut self, limit: usize) {
        self.open_file_limit = limit;
    }

    /// Set error handler (ON ERROR GOTO line)
    pub fn set_error_handler(&mut self, line_number: u16) {
        self.error_handler = Some(ErrorHandler::Line(line_number));
    }

    /// Set the error handler to the statements at a position
    /// (ON ERROR <statements>)
    pub fn set_error_handler_statements(&mut self, position: Position) {
        self.error_handler = Some(ErrorHandler::Statements(position));
    }

    /// Clear error handler (ON ERROR OFF)
    pub fn clear_error_handler(&mut self) {
        self.error_handler = None;
    }

    /// Get the error handler (returns None if no handler set)
    pub fn get_error_handler(&self) -> Option<ErrorHandler> {
        self.error_handler
    }

    /// Execute a TRACE statement. The trace stays on between runs.
    fn execute_trace(&mut self, mode: &TraceMode) -> Result<()> {
        self.trace_limit = match mode {
            // Each stored line number is below the typed line key
            TraceMode::On => Some(TYPED_LINE),
            TraceMode::Off => None,
            TraceMode::Below(limit) => Some(self.eval_integer(limit)?.max(0) as u32),
        };
        Ok(())
    }

    /// Print `[line] ` if the trace is on for this stored line
    pub fn trace_line(&mut self, line: u16) {
        if self
            .trace_limit
            .is_some_and(|limit| u32::from(line) < limit)
        {
            self.print_output(&format!("[{}] ", line));
        }
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

    /// Get last error information (for extension functions)
    #[cfg(test)]
    pub fn get_last_error(&self) -> Option<&ErrorInfo> {
        self.last_error.as_ref()
    }

    /// Execute DEF FN statement - define a function
    fn execute_def_fn(
        &mut self,
        name: &str,
        params: &[String],
        expression: &Expression,
    ) -> Result<()> {
        self.define_function(
            name.to_string(),
            params.to_vec(),
            FunctionBody::Expression(expression.clone()),
        );
        Ok(())
    }

    /// Define a function (single-line or multi-line)
    pub fn define_function(&mut self, name: String, params: Vec<String>, body: FunctionBody) {
        self.functions
            .insert(name, FunctionDefinition { params, body });
    }

    /// Set the runner for the body lines of multi-line functions
    pub fn set_function_body_runner(&mut self, runner: FunctionBodyRunner) {
        self.function_body_runner = BodyRunnerSlot(Some(runner));
    }

    /// Call a function and return integer result
    fn call_function_int(&mut self, name: &str, args: &[Expression]) -> Result<i32> {
        if name.ends_with('$') {
            // A string function: do not run its body for a number
            return Err(BBCBasicError::TypeMismatch);
        }
        self.call_function(name, args, Self::eval_integer)
    }

    /// Call a function and return real result
    fn call_function_real(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        if name.ends_with('$') {
            // A string function: do not run its body for a number
            return Err(BBCBasicError::TypeMismatch);
        }
        self.call_function(name, args, Self::eval_real)
    }

    /// Call a function and return string result
    fn call_function_string(&mut self, name: &str, args: &[Expression]) -> Result<String> {
        self.call_function(name, args, Self::eval_string)
    }

    /// Evaluate the arguments of a PROC or FN call in the caller scope, before
    /// a parameter hides a caller variable of the same name (for example
    /// FNfact(N-1)). Each value has the type of its parameter.
    pub fn evaluate_arguments(
        &mut self,
        params: &[String],
        args: &[Expression],
    ) -> Result<Vec<Variable>> {
        let mut values = Vec::with_capacity(args.len());
        for (param_name, arg_expr) in params.iter().zip(args.iter()) {
            let value = if param_name.ends_with('%') {
                Variable::Integer(self.eval_integer(arg_expr)?)
            } else if param_name.ends_with('$') {
                Variable::String(self.eval_string(arg_expr)?)
            } else {
                Variable::Real(self.eval_real(arg_expr)?)
            };
            values.push(value);
        }
        Ok(values)
    }

    /// Declare each parameter as local in the current scope, then assign its
    /// argument value. ENDPROC or ENDFN restores the caller variable.
    pub fn bind_parameters(&mut self, params: &[String], values: Vec<Variable>) -> Result<()> {
        for (param_name, value) in params.iter().zip(values) {
            self.declare_local(param_name)?;
            match value {
                Variable::Integer(v) => self.variables.set_integer_var(param_name.clone(), v),
                Variable::String(v) => self.variables.set_string_var(param_name.clone(), v)?,
                Variable::Real(v) => self.variables.set_real_var(param_name.clone(), v),
                _ => return Err(BBCBasicError::TypeMismatch),
            }
        }
        Ok(())
    }

    /// Call a user-defined function and evaluate its result with `eval`.
    fn call_function<T>(
        &mut self,
        name: &str,
        args: &[Expression],
        eval: fn(&mut Self, &Expression) -> Result<T>,
    ) -> Result<T> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| BBCBasicError::NoSuchVariable(format!("Function {} not defined", name)))?
            .clone();

        // Check parameter count
        if args.len() != func.params.len() {
            return Err(BBCBasicError::Arguments);
        }

        let values = self.evaluate_arguments(&func.params, args)?;

        let local_depth = self.local_stack.len();
        let return_depth = self.return_stack.len();
        let for_depth = self.for_loops.len();
        let repeat_depth = self.repeat_stack.len();
        let while_depth = self.while_stack.len();

        // Enter local scope for function
        self.enter_local_scope();
        let result = self.run_function(&func, values, eval);

        // Exit local scope (restore variables), also after an error in the
        // function. Scopes, returns and loops that the body did not close go
        // too (for example a `=` line inside a FOR loop).
        while self.local_stack.len() > local_depth {
            self.exit_local_scope()?;
        }
        self.return_stack.truncate(return_depth);
        self.for_loops.truncate(for_depth);
        self.repeat_stack.truncate(repeat_depth);
        self.while_stack.truncate(while_depth);

        result
    }

    /// Bind the parameters, run the function body and evaluate the result.
    fn run_function<T>(
        &mut self,
        func: &FunctionDefinition,
        values: Vec<Variable>,
        eval: fn(&mut Self, &Expression) -> Result<T>,
    ) -> Result<T> {
        self.bind_parameters(&func.params, values)?;

        match &func.body {
            FunctionBody::Expression(expression) => eval(self, expression),
            FunctionBody::Lines(def_position) => {
                let runner = self.function_body_runner.0.clone().ok_or_else(|| {
                    BBCBasicError::SyntaxError {
                        message: "A multi-line function needs a running program".to_string(),
                        line: def_position.stored_line(),
                    }
                })?;
                let mut result = None;
                runner(self, *def_position, &mut |executor, expression| {
                    result = Some(eval(executor, expression)?);
                    Ok(())
                })?;
                result.ok_or(BBCBasicError::BadCall)
            }
        }
    }

    /// Open a file for reading (OPENIN).
    /// Gives 0, as BBC BASIC does, when the file cannot be opened.
    fn open_file_for_reading(&mut self, filename: &str) -> Result<i32> {
        self.open_existing_file(File::open(filename), FileHandle::Input)
    }

    /// Open an existing file for reading and writing (OPENUP), with the
    /// pointer at 0. Gives 0 when the file cannot be opened.
    fn open_file_for_update(&mut self, filename: &str) -> Result<i32> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(filename);
        self.open_existing_file(file, FileHandle::Update)
    }

    /// Store a file that OPENIN or OPENUP opened and give its handle,
    /// or give 0 if the open failed.
    fn open_existing_file(
        &mut self,
        file: std::io::Result<File>,
        kind: fn(BufReader<File>) -> FileHandle,
    ) -> Result<i32> {
        // Check if we've hit the maximum number of open files
        if self.open_files.len() >= self.open_file_limit {
            return Err(BBCBasicError::TooManyOpenFiles);
        }

        let Ok(file) = file else {
            return Ok(0);
        };

        // Allocate a handle
        let handle = self.next_file_handle;
        self.next_file_handle += 1;

        // Store the file handle
        self.open_files.insert(handle, kind(BufReader::new(file)));

        Ok(handle)
    }

    /// Write bytes at the pointer of an OPENUP file.
    fn write_update(reader: &mut BufReader<File>, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Seek;
        // A seek to the logical position drops the read buffer, so that the
        // write goes to the pointer and not to the end of the buffer.
        let pos = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(pos))?;
        reader.get_mut().write_all(bytes)
    }

    /// Open a file for writing (OPENOUT)
    fn open_file_for_writing(&mut self, filename: &str) -> Result<i32> {
        // Check if we've hit the maximum number of open files
        if self.open_files.len() >= self.open_file_limit {
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
            FileHandle::Input(reader) | FileHandle::Update(reader) => {
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
    fn execute_print_file(
        &mut self,
        handle_expr: &Expression,
        items: &[crate::parser::PrintItem],
    ) -> Result<()> {
        // Evaluate the handle
        let handle = self.eval_integer(handle_expr)?;

        // Process print items and build output string first (to avoid borrow issues)
        use crate::parser::PrintItem;
        let mut output = String::new();

        // After ~, numeric items are hex until the next comma or semicolon
        let mut hex = false;
        for item in items {
            match item {
                PrintItem::Expression(expr) => {
                    output.push_str(&self.format_print_item(expr, hex)?);
                }
                PrintItem::Tab(_) | PrintItem::TabXY(..) | PrintItem::Spc(_) => {
                    // The parser does not permit TAB and SPC in PRINT#
                }
                PrintItem::Hex => hex = true,
                PrintItem::Semicolon => {
                    // Semicolon suppresses newline - do nothing
                    hex = false;
                }
                PrintItem::Comma => {
                    hex = false;
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

        // Only output and update files can be written to
        let writer = match file_handle {
            FileHandle::Output(writer) => writer,
            FileHandle::Update(reader) => {
                return Self::write_update(reader, output.as_bytes())
                    .map_err(|e| BBCBasicError::DiskError(format!("Write error: {}", e)));
            }
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
            FileHandle::Input(reader) | FileHandle::Update(reader) => reader,
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
                self.variables
                    .set_string_var(var_name.clone(), value_str.to_string())?;
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

        // CLOSE#0 closes all open files, as in BBC BASIC
        if handle == 0 {
            self.open_files.clear();
            return Ok(());
        }

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
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // BGET# only works on input files
        match file_handle {
            FileHandle::Input(reader) | FileHandle::Update(reader) => {
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
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // BPUT# only works on output files
        match file_handle {
            FileHandle::Output(writer) => {
                // Convert value to byte (MOD 256)
                let byte = (value % 256) as u8;

                // Write the byte
                writer
                    .write_all(&[byte])
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Flush to ensure byte is written
                writer
                    .flush()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                Ok(())
            }
            FileHandle::Update(reader) => Self::write_update(reader, &[(value % 256) as u8])
                .map_err(|e| BBCBasicError::DiskError(e.to_string())),
            FileHandle::Input(_) => {
                Err(BBCBasicError::TypeMismatch) // Cannot write to input file
            }
        }
    }

    /// PTR# function - Get current file position
    pub fn get_ptr(&mut self, handle: i32) -> Result<i32> {
        use std::io::Seek;

        // Get the file handle
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Get current position from the underlying file
        match file_handle {
            FileHandle::Input(reader) | FileHandle::Update(reader) => {
                let pos = reader
                    .stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(pos as i32)
            }
            FileHandle::Output(writer) => {
                let pos = writer
                    .stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(pos as i32)
            }
        }
    }

    /// PTR# assignment - Set file position
    pub fn set_ptr(&mut self, handle: i32, position: i32) -> Result<()> {
        use std::io::Seek;

        // Get the file handle
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // A file has no position before its start
        if position < 0 {
            return Err(BBCBasicError::IllegalFunction);
        }

        // Seek to the specified position
        let pos = position as u64;
        match file_handle {
            FileHandle::Input(reader) | FileHandle::Update(reader) => {
                reader
                    .seek(std::io::SeekFrom::Start(pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(())
            }
            FileHandle::Output(writer) => {
                writer
                    .seek(std::io::SeekFrom::Start(pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;
                Ok(())
            }
        }
    }

    /// EXT# function - Get file size
    pub fn get_ext(&mut self, handle: i32) -> Result<i32> {
        use std::io::Seek;

        // Get the file handle
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Get file size by seeking to end
        match file_handle {
            FileHandle::Input(reader) | FileHandle::Update(reader) => {
                // Save current position
                let current_pos = reader
                    .stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Seek to end to get size
                let size = reader
                    .seek(std::io::SeekFrom::End(0))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Restore original position
                reader
                    .seek(std::io::SeekFrom::Start(current_pos))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                Ok(size as i32)
            }
            FileHandle::Output(writer) => {
                // Save current position
                let current_pos = writer
                    .stream_position()
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Seek to end to get size
                let size = writer
                    .seek(std::io::SeekFrom::End(0))
                    .map_err(|e| BBCBasicError::DiskError(e.to_string()))?;

                // Restore original position
                writer
                    .seek(std::io::SeekFrom::Start(current_pos))
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

/// Give the date and time for a count of seconds from 1970 (UTC) in the
/// form of TIME$, for example "Fri,27 Nov 2026.14:05:09".
///
/// TIME$ is from the BBC Master. The BBC Model B has no TIME$.
fn format_time_string(seconds_since_1970: u64) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = seconds_since_1970 / 86_400;
    let second_of_day = seconds_since_1970 % 86_400;
    let day_name = DAYS[(days % 7) as usize];

    // Calendar date from the day count (civil_from_days, Howard Hinnant).
    // The year starts on 1 March, so that 29 February is the last day of a year.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153; // 0 is March
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);

    format!(
        "{},{:02} {} {:04}.{:02}:{:02}:{:02}",
        day_name,
        day,
        MONTHS[(month - 1) as usize],
        year,
        second_of_day / 3600,
        second_of_day / 60 % 60,
        second_of_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_leading_number_edge_cases() {
        assert_eq!(parse_leading_number(".5"), 0.5);
        assert_eq!(parse_leading_number("+5"), 5.0);
        assert_eq!(parse_leading_number("7."), 7.0);
        assert_eq!(parse_leading_number("-"), 0.0);
        assert_eq!(parse_leading_number("."), 0.0);
        assert_eq!(parse_leading_number("-.x"), 0.0);
        assert_eq!(parse_leading_number("1E"), 1.0);
        assert_eq!(parse_leading_number("2e+2x"), 200.0);
        assert_eq!(parse_leading_number("1.5.6"), 1.5);
    }

    #[test]
    fn test_parse_leading_number_reads_hex() {
        assert_eq!(parse_leading_number("&1F"), 31.0);
        assert_eq!(parse_leading_number("  &FFx"), 255.0);
        assert_eq!(parse_leading_number("&FFFFFFFF"), -1.0);
        assert_eq!(parse_leading_number("&"), 0.0);
        assert_eq!(parse_leading_number("&G"), 0.0);
    }

    #[test]
    fn test_format_hex_uses_32_bit_form() {
        assert_eq!(format_hex(255), "FF");
        assert_eq!(format_hex(0), "0");
        assert_eq!(format_hex(-1), "FFFFFFFF");
    }

    #[test]
    fn test_format_number_rounds_to_nine_significant_digits() {
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(0.9999999999999999), "1");
        assert_eq!(format_number(4.999999999999999), "5");
        assert_eq!(format_number(1.0 / 3.0), "0.333333333");
        assert_eq!(format_number(3.5), "3.5");
        assert_eq!(format_number(-2.25), "-2.25");
        assert_eq!(format_number(123456789.0), "123456789");
    }

    #[test]
    fn test_format_number_uses_exponent_outside_plain_range() {
        assert_eq!(format_number(1234567890.0), "1.23456789E9");
        assert_eq!(format_number(1e9), "1E9");
        assert_eq!(format_number(1.5e10), "1.5E10");
        assert_eq!(format_number(1e-8), "1E-8");
        assert_eq!(format_number(-1e-8), "-1E-8");
        assert_eq!(format_number(1e-7), "0.0000001");
        assert_eq!(format_number(999999999.6), "1E9");
    }

    #[test]
    fn test_format_number_zero_has_no_sign() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
    }
    use crate::parser::BinaryOperator;

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
        assert_eq!(executor.for_loops[0].1, 10.0); // end value
        assert_eq!(executor.for_loops[0].2, 1.0); // step value
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
        assert_eq!(executor.for_loops[0].2, -1.0); // step value
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
        assert!(matches!(result.unwrap_err(), BBCBasicError::NoFor));
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
            items: ["A%", "B$", "C"]
                .iter()
                .map(|name| InputItem::Variable(name.to_string()))
                .collect(),
        };

        // In test mode, INPUT initializes variables to default values
        executor.execute_statement(&stmt).unwrap();

        // Variables should be initialized
        assert_eq!(executor.get_variable_int("A%").unwrap(), 0);
        assert_eq!(executor.get_variable_string("B$").unwrap(), "");
        assert_eq!(executor.get_variable_real("C").unwrap(), 0.0);
    }

    fn dim_array(name: &str, dimensions: Vec<Expression>) -> DimItem {
        DimItem::Array {
            name: name.to_string(),
            dimensions,
        }
    }

    #[test]
    fn test_dim_integer_array() {
        // RED: Test DIM A%(10)
        let mut executor = Executor::new();
        let stmt = Statement::Dim {
            items: vec![dim_array("A%", vec![Expression::Integer(10)])],
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
            items: vec![dim_array(
                "B%",
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
            items: vec![
                dim_array("A%", vec![Expression::Integer(10)]),
                dim_array("B$", vec![Expression::Integer(5)]),
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

        // Y% keeps its start value 0 because condition is false
        assert_eq!(executor.get_variable_int("Y%").unwrap(), 0);
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
        // RED: Test SIN(PI / 2) = 1 (BBC BASIC uses radians)
        let mut executor = Executor::new();

        let sin_half_pi = Expression::FunctionCall {
            name: "SIN".to_string(),
            args: vec![Expression::Real(std::f64::consts::FRAC_PI_2)],
        };

        let result = executor.eval_real(&sin_half_pi).unwrap();
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
        // RED: Test TAN(PI / 4) ≈ 1.0 (BBC BASIC uses radians)
        let mut executor = Executor::new();

        let tan_quarter_pi = Expression::FunctionCall {
            name: "TAN".to_string(),
            args: vec![Expression::Real(std::f64::consts::FRAC_PI_4)],
        };

        let result = executor.eval_real(&tan_quarter_pi).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_atn_function() {
        // RED: Test ATN(1) = PI / 4 radians
        let mut executor = Executor::new();

        let atn_expr = Expression::FunctionCall {
            name: "ATN".to_string(),
            args: vec![Expression::Real(1.0)],
        };

        let result = executor.eval_real(&atn_expr).unwrap();
        assert!((result - std::f64::consts::FRAC_PI_4).abs() < 0.0001);
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
            args: vec![Expression::Integer(5), Expression::String("*".to_string())],
        };
        assert_eq!(executor.eval_string(&string_expr).unwrap(), "*****");

        // Test STRING$(3, "ABC") - repeats the full string
        let string_expr2 = Expression::FunctionCall {
            name: "STRING$".to_string(),
            args: vec![
                Expression::Integer(3),
                Expression::String("ABC".to_string()),
            ],
        };
        assert_eq!(executor.eval_string(&string_expr2).unwrap(), "ABCABCABC");
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
        executor.collect_data(&data_stmt, 10).unwrap();

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
        executor.collect_data(&data_stmt, 10).unwrap();

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
        executor.collect_data(&data_stmt, 10).unwrap();

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
        executor.collect_data(&data_stmt, 10).unwrap();

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
        let data_stmt1 = Statement::Data {
            values: vec![DataValue::Integer(100), DataValue::Integer(200)],
        };
        executor.collect_data(&data_stmt1, 10).unwrap();

        // Line 20: DATA 300, 400
        let data_stmt2 = Statement::Data {
            values: vec![DataValue::Integer(300), DataValue::Integer(400)],
        };
        executor.collect_data(&data_stmt2, 20).unwrap();

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
        executor.collect_data(&data_stmt1, 10).unwrap();

        // DATA 30, 40
        let data_stmt2 = Statement::Data {
            values: vec![DataValue::Integer(30), DataValue::Integer(40)],
        };
        executor.collect_data(&data_stmt2, 20).unwrap();

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
        executor.collect_data(&data_stmt, 10).unwrap();

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
        executor.collect_data(&data_stmt, 20).unwrap();

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
                (0.0..1.0).contains(&result),
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
                (1..=10).contains(&as_int),
                "RND(10) should return values 1-10, got {}",
                result
            );
        }
    }

    #[test]
    fn test_time_string_at_second_zero_is_the_start_of_1970() {
        assert_eq!(format_time_string(0), "Thu,01 Jan 1970.00:00:00");
    }

    #[test]
    fn test_time_string_on_a_leap_day() {
        assert_eq!(format_time_string(951_782_400), "Tue,29 Feb 2000.00:00:00");
    }

    #[test]
    fn test_time_string_with_a_time_of_day() {
        assert_eq!(
            format_time_string(1_795_788_309),
            "Fri,27 Nov 2026.14:05:09"
        );
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
        assert!(
            result2 >= result1,
            "TIME should increase: {} >= {}",
            result2,
            result1
        );

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
        assert_eq!(
            report_result, "Division by zero",
            "REPORT$ should return error message"
        );
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
        assert_eq!(
            report_result, "",
            "REPORT$ should return empty string when no error"
        );
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
        executor.push_repeat(Position::line_start(20));

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
                assert_eq!(
                    result,
                    Some(Position::line_start(20)),
                    "Should loop back to REPEAT at line 20"
                );
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
        executor.push_gosub_return(Position::line_start(20));

        // Verify return address was saved
        assert_eq!(executor.return_stack.len(), 1);

        // Pop return address
        let return_line = executor.pop_gosub_return().unwrap();

        // Should return to line 20 (caller will advance to next line)
        assert_eq!(
            return_line,
            Position::line_start(20),
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

        executor.push_gosub_return(Position::line_start(10));
        executor.push_gosub_return(Position::line_start(100));

        // First RETURN should go back to 100
        assert_eq!(
            executor.pop_gosub_return().unwrap(),
            Position::line_start(100)
        );

        // Second RETURN should go back to 10
        assert_eq!(
            executor.pop_gosub_return().unwrap(),
            Position::line_start(10)
        );
    }

    #[test]
    fn test_procedure_definition() {
        // RED: Test defining a procedure
        let mut executor = Executor::new();

        // Define a simple procedure
        executor.define_procedure("greet".to_string(), Position::line_start(100), vec![]);

        // Should be able to retrieve it
        let proc = executor.get_procedure("greet");
        assert!(proc.is_some());
        assert_eq!(proc.unwrap().position, Position::line_start(100));
        assert_eq!(proc.unwrap().params.len(), 0);
    }

    #[test]
    fn test_procedure_definition_with_params() {
        // RED: Test defining a procedure with parameters
        let mut executor = Executor::new();

        // Define procedure with parameters
        executor.define_procedure(
            "add".to_string(),
            Position::line_start(200),
            vec!["X".to_string(), "Y".to_string()],
        );

        // Should be able to retrieve it
        let proc = executor.get_procedure("add");
        assert!(proc.is_some());
        assert_eq!(proc.unwrap().position, Position::line_start(200));
        assert_eq!(proc.unwrap().params, vec!["X", "Y"]);
    }

    #[test]
    fn test_procedure_not_found() {
        // RED: Test getting undefined procedure
        let executor = Executor::new();

        // Should return None for undefined procedure
        assert!(executor.get_procedure("undefined").is_none());
    }

    #[test]
    fn test_clear_procedures() {
        // RED: Test clearing all procedures
        let mut executor = Executor::new();

        executor.define_procedure("proc1".to_string(), Position::line_start(100), vec![]);
        executor.define_procedure("proc2".to_string(), Position::line_start(200), vec![]);

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
            expression: Some(Expression::BinaryOp {
                left: Box::new(Expression::Variable("X".to_string())),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Variable("Y".to_string())),
            }),
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
            expression: Some(Expression::BinaryOp {
                left: Box::new(Expression::Variable("X".to_string())),
                op: BinaryOperator::Multiply,
                right: Box::new(Expression::Integer(2)),
            }),
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
        assert_eq!(executor.get_error_handler(), Some(ErrorHandler::Line(1000)));

        // Clear error handler
        executor.clear_error_handler();
        assert_eq!(executor.get_error_handler(), None);
    }

    #[test]
    fn test_erl_err_functions_no_error() {
        // RED: Test ERL and ERR when no error has occurred
        let executor = Executor::new();

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

        assert_eq!(executor.get_error_handler(), Some(ErrorHandler::Line(1000)));
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
    fn test_openin_gives_zero_on_missing_file() {
        // Issue #105: OPENIN gives 0 for a file that is not there, as BBC BASIC does
        let mut executor = Executor::new();

        let result = executor.open_file_for_reading("nonexistent_file.txt");
        assert_eq!(result, Ok(0));
        assert!(executor.open_files.is_empty());
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
        assert!(matches!(
            result.unwrap_err(),
            BBCBasicError::ChannelNotOpen(_)
        ));

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
        executor
            .execute_input_file(&handle_expr, &variables)
            .unwrap();

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
        let result = executor
            .push_while(Position::line_start(20), &condition)
            .unwrap();
        assert_eq!(
            result,
            Some(Position::line_start(20)),
            "Should enter loop when X% = 0"
        );

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
                assert_eq!(
                    result,
                    Some(Position::line_start(20)),
                    "Should loop back when X% = {}",
                    expected
                );
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

        let result = executor
            .push_while(Position::line_start(20), &condition)
            .unwrap();
        assert_eq!(
            result, None,
            "Should not enter loop when condition is false"
        );

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
        executor
            .push_while(Position::line_start(10), &outer_condition)
            .unwrap();
        assert_eq!(executor.while_stack.len(), 1);

        // Inner: WHILE J% < 2
        executor.variables.set_integer_var("J%".to_string(), 0);
        let inner_condition = Expression::BinaryOp {
            left: Box::new(Expression::Variable("J%".to_string())),
            op: BinaryOperator::LessThan,
            right: Box::new(Expression::Integer(2)),
        };

        // Enter inner loop
        executor
            .push_while(Position::line_start(20), &inner_condition)
            .unwrap();
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
        fs::write(test_file, [65, 66, 67, 255, 0]).unwrap();

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
        fs::write(test_file, [42]).unwrap();

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
        executor.bput(handle, 65).unwrap(); // 'A'
        executor.bput(handle, 66).unwrap(); // 'B'
        executor.bput(handle, 67).unwrap(); // 'C'
        executor.bput(handle, 255).unwrap(); // Max byte value
        executor.bput(handle, 0).unwrap(); // Zero

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
        executor.bput(handle, 256).unwrap(); // Should write 0
        executor.bput(handle, 257).unwrap(); // Should write 1
        executor.bput(handle, 300).unwrap(); // Should write 44

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
        fs::write(test_file, [1, 2, 3, 4, 5]).unwrap();

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
        fs::write(test_file, [65, 66, 67, 68, 69]).unwrap(); // A, B, C, D, E

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
        fs::write(test_file, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap();

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
        fs::write(test_file, []).unwrap();

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

    #[test]
    fn test_inkey_with_empty_queue_gives_no_key_with_no_wait() {
        // A test build does not read stdin, so the time limit is not used.
        let inkey = |name: &str| Expression::FunctionCall {
            name: name.to_string(),
            args: vec![Expression::Integer(100)],
        };
        let mut executor = Executor::new();
        let start = std::time::Instant::now();

        assert_eq!(executor.eval_integer(&inkey("INKEY")).unwrap(), -1);
        assert_eq!(executor.eval_string(&inkey("INKEY$")).unwrap(), "");
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
    }
}
