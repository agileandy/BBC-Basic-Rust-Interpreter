//! Execution engine for BBC BASIC statements
//!
//! Executes parsed BBC BASIC statements with proper control flow handling.

mod array_functions;
mod console;
// Unit tests never read stdin, so the stdin reader is not used in them.
#[cfg_attr(test, allow(dead_code))]
mod key_input;
mod matrix;
mod sound;
mod structures;
mod vdu;
mod whole_array;

pub use console::{Console, CursorMove, Interrupt};
pub use structures::Argument;
use structures::MemberIndices;

use crate::collections::{CollectionStore, Element};
use crate::error::{BBCBasicError, Result};
use crate::graphics::{GraphicsSystem, ScreenMode};
use crate::library::LibraryStore;
use crate::memory::MemoryManager;
use crate::parser::{
    BinaryOperator, DataValue, DimItem, Expression, Indirection, InputItem, MemberPath, Statement,
    TraceMode,
};
use crate::program::{Position, TYPED_LINE};
use crate::variables::{
    is_resident_integer, resident_default, StructureId, Variable, VariableStore,
    PRINT_FORMAT_DEFAULT, PRINT_FORMAT_NAME,
};
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

/// Where the value of a SWAP target is: a variable, the index values of an
/// array element, or the path and the index values of a member (issues
/// #191, #192 and #193)
enum SwapPlace<'a> {
    Variable,
    Element(Vec<usize>),
    Member(&'a MemberPath, MemberIndices),
}

/// The name of a SWAP target, which gives its type, and its place
type SwapLocation<'a> = (&'a str, SwapPlace<'a>);

/// The control variable of a FOR loop, with the index values that the FOR
/// evaluated one time (issue #198). 2 loops with equal targets are the same
/// loop, so `FOR A(1)` and `FOR A(2)` are 2 loops.
#[derive(Debug, Clone, PartialEq)]
enum ForTarget {
    Variable(String),
    Element(String, Vec<usize>),
    Member {
        structure: String,
        members: Vec<String>,
        indices: MemberIndices,
    },
}

/// Convert a value to the type of the variable `name`. A number and a
/// string give "Type mismatch". A real that does not fit in an integer gives
/// "Too big".
fn convert_to_type_of(name: &str, value: Variable) -> Result<Variable> {
    match (name.ends_with('$'), name.ends_with('%'), value) {
        (true, _, Variable::String(text)) => Ok(Variable::String(text)),
        (false, true, Variable::Integer(number)) => Ok(Variable::Integer(number)),
        (false, true, Variable::Real(number)) => real_to_integer(number).map(Variable::Integer),
        (false, false, Variable::Integer(number)) => Ok(Variable::Real(f64::from(number))),
        (false, false, Variable::Real(number)) => Ok(Variable::Real(number)),
        _ => Err(BBCBasicError::TypeMismatch),
    }
}

/// The error of a list function with the wrong count of arguments (issue #139).
fn list_arguments_error(name: &str, count: usize) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: format!("{} requires {} arguments", name, count),
        line: None,
    }
}

/// The error for a JSON function call with the wrong number of arguments.
fn json_argument_count_error(name: &str, count: usize) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: format!("{} requires {} arguments", name, count),
        line: None,
    }
}

/// The error for a name that the string evaluator does not know.
fn unknown_string_function(name: &str) -> BBCBasicError {
    BBCBasicError::SyntaxError {
        message: format!("Unknown string function: {}", name),
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

/// The 3 number formats of byte 3 of `@%`
#[derive(Debug, Clone, Copy, PartialEq)]
enum NumberStyle {
    /// G: up to the count of digits as significant digits, no trailing zeros
    General,
    /// E: one digit, a point, the other digits and the exponent
    Exponent,
    /// F: the count of digits as decimal places
    Fixed,
}

/// The number format of `@%` (issue 166). The rules are the rules of the
/// number output routine of the BBC BASIC 2 ROM (&9EDF to &A03F).
#[derive(Debug, Clone, Copy, PartialEq)]
struct NumberFormat {
    /// Byte 1: the width of a PRINT field and of a comma zone
    width: usize,
    /// Byte 2: the count of digits, from 0 to 10
    digits: usize,
    /// Byte 3: the format. A value above 2 is the general format.
    style: NumberStyle,
    /// Byte 4 is not 0: STR$ uses this format
    for_str: bool,
}

/// The format of STR$ when byte 4 of `@%` is 0: the general format with 10
/// digits. The ROM sets format 0 at &B0B1 and 10 digits at &9EF9.
const STR_FORMAT: NumberFormat = NumberFormat {
    width: 0,
    digits: 10,
    style: NumberStyle::General,
    for_str: false,
};

impl NumberFormat {
    /// The format of an `@%` value. A count of digits that is above 10 is 10.
    /// A count of 0 is 10 in the general and exponent formats (ROM &9EEA).
    fn from_value(value: i32) -> Self {
        let [width, digits, style, for_str] = value.to_le_bytes();
        let style = match style {
            1 => NumberStyle::Exponent,
            2 => NumberStyle::Fixed,
            _ => NumberStyle::General,
        };
        let digits = match digits {
            0 if style == NumberStyle::Fixed => 0,
            0 => 10,
            count => count.min(10),
        };
        Self {
            width: width as usize,
            digits: digits as usize,
            style,
            for_str: for_str != 0,
        }
    }

    /// The text of a real value in this format, with no field
    fn format_real(&self, value: f64) -> String {
        if !value.is_finite() {
            return value.to_string();
        }
        let text = match self.style {
            NumberStyle::General => general_text(value.abs(), self.digits, ROM_LOWEST_PLAIN),
            NumberStyle::Exponent => exponent_text(value.abs(), self.digits),
            NumberStyle::Fixed => fixed_text(value.abs(), self.digits),
        };
        with_sign(value, text)
    }

    /// The text of an integer value in this format, with no field. The ROM
    /// changes an integer to a real value before it makes the digits (&9F09
    /// to &9F0C), so an integer shows as a real value does.
    fn format_integer(&self, value: i32) -> String {
        self.format_real(value as f64)
    }
}

/// The lowest decimal exponent that the general format of the ROM shows in
/// plain decimal: -1, so 0.1 is plain and 0.05 is 5E-2. The ROM gives the
/// exponent form for a negative exponent other than -1 (&9FC3 to &9FCD).
const ROM_LOWEST_PLAIN: i32 = -1;

/// The lowest decimal exponent that the PRINT# and LVAR text shows in plain
/// decimal. This text is not a ROM format, and does not change (issue 197).
const FILE_LOWEST_PLAIN: i32 = -7;

/// Put a minus sign before the text of a value that is below 0
fn with_sign(value: f64, text: String) -> String {
    if value < 0.0 {
        format!("-{}", text)
    } else {
        text
    }
}

/// Round a value that is more than 0 to `count` significant digits, with a 5
/// rounded up, as the ROM adds 5 at the digit after the last. Give the digits
/// and the decimal exponent of the first digit. A count of 0 gives the digit
/// 1 at the next exponent if the first digit is 5 or more. Give `None` if the
/// value rounds to 0.
fn round_digits(value: f64, count: i32) -> Option<(String, i32)> {
    let scientific = format!("{:.20e}", value);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("exponent format has an exponent");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");
    let all_digits = mantissa.replace('.', "").into_bytes();

    if count < 0 {
        return None;
    }
    let count = count as usize;
    if count == 0 {
        return (all_digits[0] >= b'5').then(|| ("1".to_string(), exponent + 1));
    }

    let mut digits = all_digits[..count].to_vec();
    if all_digits[count] >= b'5' {
        // Add 1 to the last digit, and carry to the left
        let mut index = count;
        loop {
            if index == 0 {
                // All digits were 9: the value is the next power of 10
                let mut text = "1".to_string();
                text.push_str(&"0".repeat(count - 1));
                return Some((text, exponent + 1));
            }
            index -= 1;
            if digits[index] == b'9' {
                digits[index] = b'0';
            } else {
                digits[index] += 1;
                break;
            }
        }
    }
    Some((
        String::from_utf8(digits).expect("digits are ASCII"),
        exponent,
    ))
}

/// The general format of a value that is 0 or more. If the exponent of the
/// rounded value is from `lowest` up to (not including) the count of digits,
/// the text is plain decimal (ROM &9FB3 to &9FB9). Otherwise the text has a
/// mantissa and an exponent, for example `1.5E10`.
fn general_text(value: f64, count: usize, lowest: i32) -> String {
    let Some((digits, exponent)) = (value > 0.0)
        .then(|| round_digits(value, count as i32))
        .flatten()
    else {
        return "0".to_string();
    };

    if (lowest..count as i32).contains(&exponent) {
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
        let fraction = digits[1..].trim_end_matches('0');
        if fraction.is_empty() {
            format!("{}E{}", &digits[..1], exponent)
        } else {
            format!("{}.{}E{}", &digits[..1], fraction, exponent)
        }
    }
}

/// The exponent format of a value that is 0 or more: one digit, a point, the
/// other digits and the exponent. The exponent part always has 4 characters:
/// a space is in the place of the minus sign, and a space follows an exponent
/// of one digit (ROM &A02B).
fn exponent_text(value: f64, count: usize) -> String {
    let (digits, exponent) = (value > 0.0)
        .then(|| round_digits(value, count as i32))
        .flatten()
        .unwrap_or_else(|| ("0".repeat(count), 0));
    let mut text = format!("{}.{}E{}", &digits[..1], &digits[1..], exponent);
    if exponent >= 0 {
        text.push(' ');
    }
    if exponent.abs() < 10 {
        text.push(' ');
    }
    text
}

/// The fixed format of a value that is 0 or more, with `places` decimal
/// places. The text always has a point, also with 0 places. If the value
/// needs more than 10 significant digits, the text is the general format
/// with 10 digits (ROM &9F50).
fn fixed_text(value: f64, places: usize) -> String {
    let rounded = if value > 0.0 {
        // The decimal exponent of the value before it is rounded
        let exponent: i32 = format!("{:e}", value)
            .split_once('e')
            .and_then(|(_, exponent)| exponent.parse().ok())
            .expect("exponent format has an exponent");
        let count = places as i32 + exponent + 1;
        if count > 10 {
            return general_text(value, 10, ROM_LOWEST_PLAIN);
        }
        round_digits(value, count)
    } else {
        None
    };
    let (digits, exponent) = rounded.unwrap_or_else(|| ("0".to_string(), 0));
    if places as i32 + exponent + 1 > 10 {
        return general_text(value, 10, ROM_LOWEST_PLAIN);
    }

    // Each digit after the rounded digits is 0
    if exponent >= 0 {
        let whole = exponent as usize + 1;
        let digits = format!("{:0<width$}", digits, width = whole + places);
        format!("{}.{}", &digits[..whole], &digits[whole..])
    } else {
        let zeros = "0".repeat((-exponent - 1) as usize);
        let fraction = format!("{}{}", zeros, digits);
        format!("0.{:0<width$}", fraction, width = places)
    }
}

/// Put the text of a number at the right of a field of `width` characters, as
/// PRINT does. A text that is longer than the field does not change.
fn pad_to_field(text: &str, width: usize) -> String {
    format!("{:>width$}", text, width = width)
}

/// Make the hex text of an integer, as `PRINT ~` does: upper case, with no `&`
/// and no leading zeros. A negative value shows its 32-bit form.
fn format_hex(value: i32) -> String {
    format!("{:X}", value as u32)
}

/// Make the text for a real value. If `exact` is false, the text is the PRINT text.
/// If `exact` is true (PRINT#, issue #157), the text reads back as the same value:
/// it is the 9 digit text with plain decimal from 1E-7 if that text gives the same
/// value, otherwise the shortest text that gives the same value, for example
/// `0.3333333333333333`.
fn format_real(value: f64, exact: bool) -> String {
    if !exact || !value.is_finite() {
        return format_number(value);
    }
    let text = with_sign(value, general_text(value.abs(), 9, FILE_LOWEST_PLAIN));
    if text.parse::<f64>() == Ok(value) {
        text
    } else {
        format!("{:?}", value)
    }
}

/// The format of the start value of `@%`: the general format with 9 digits
const DEFAULT_FORMAT: NumberFormat = NumberFormat {
    width: 10,
    digits: 9,
    style: NumberStyle::General,
    for_str: false,
};

/// Make the PRINT text for a real value with the start value of `@%`, as a BBC
/// Micro does. The value is rounded to 9 significant digits, without trailing
/// zeros. If the rounded absolute value is from 0.1 up to (not including) 1E9,
/// the text is plain decimal. Otherwise the text has a mantissa and an
/// exponent, for example `1.5E10` or `5E-2`.
fn format_number(value: f64) -> String {
    DEFAULT_FORMAT.format_real(value)
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
    /// Saved structure names, for LOCAL and for a structure parameter (see
    /// `VariableStore::save_structure`, issue #194)
    saved_structures: HashMap<String, Option<Option<StructureId>>>,
    /// The LOCAL structures that a DIM made for this frame. The exit removes them.
    local_structures: Vec<StructureId>,
    /// The DATA pointers that LOCAL DATA saved in this routine. The exit
    /// of the routine restores the first (issue #186).
    saved_data: Vec<DataPointer>,
}

/// A DATA pointer: the line set of the DATA (0 for the program, n for
/// library n) and the index of the next item in the DATA of that set
type DataPointer = (usize, usize);

impl LocalFrame {
    fn new() -> Self {
        Self {
            saved_variables: HashMap::new(),
            saved_arrays: HashMap::new(),
            saved_structures: HashMap::new(),
            local_structures: Vec::new(),
            saved_data: Vec::new(),
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
    /// Single-line function: `DEF FNname(params) = expr`, and the line of
    /// the DEF (none for a typed line)
    Expression(Expression, Option<u32>),
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
    /// ON ERROR GOTO: the first statement of the line with this line key.
    /// The key holds the line set, so that the handler of a library goes to
    /// a line of the library (issue #185).
    Line(u32),
    /// ON ERROR <statements>: the first statement after ON ERROR
    Statements(Position),
}

/// The number of files that a program can open at one time.
const DEFAULT_OPEN_FILE_LIMIT: usize = 255;

/// The maximum number of open evaluation levels: evaluators, function
/// evaluators, FN calls and EVAL calls. Each level uses the native stack, so
/// the limit stops a stack overflow (issue 151).
///
/// On the 64 MiB interpreter thread of `main`, a debug build overflows at
/// 2880 levels on the worst path that was measured (a PRINT of a FN call in
/// a multi-line FN body). This limit gives a margin of 2.4. A simple
/// recursive FN uses about 4 levels for each call.
///
/// A library caller on a thread with a smaller stack (2 MiB for a spawned
/// thread) can overflow before this limit. Tests of deep recursion run the
/// binary for this reason.
const EVALUATION_DEPTH_LIMIT: usize = 1200;

/// The maximum number of entries on the return stack, on each loop stack
/// and on the LOCAL scope stack (issue 151)
const STACK_ENTRY_LIMIT: usize = 10_000;

/// Give No room if a stack with `len` entries cannot take one more
fn check_room(len: usize) -> Result<()> {
    if len >= STACK_ENTRY_LIMIT {
        return Err(BBCBasicError::NoRoom);
    }
    Ok(())
}

/// The statement that put an entry on the return stack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnKind {
    /// GOSUB or ON GOSUB: RETURN goes back
    Gosub,
    /// A PROC call: ENDPROC goes back and closes the loops above these depths
    Proc(LoopDepths),
}

/// The depths of the FOR, REPEAT and WHILE stacks at a call
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct LoopDepths {
    for_depth: usize,
    repeat_depth: usize,
    while_depth: usize,
}

/// What one entry of the FOR stack counts
#[derive(Debug, Clone, Copy, PartialEq)]
enum ForKind {
    /// `FOR v = start TO end STEP step`. An integer loop (`%` variable)
    /// holds whole values, which f64 holds with no loss.
    Count { end: f64, step: f64 },
    /// `FOR EACH v IN collection` (issue #141): the handle of the list or
    /// map, and the index of the element that `v` has
    Each { handle: i32, index: usize },
}

/// One entry of the FOR stack
#[derive(Debug, Clone, PartialEq)]
struct ForLoop {
    target: ForTarget,
    kind: ForKind,
    /// The FOR statement. The run loop sets it (set_for_loop_line).
    position: Position,
}

/// BBC BASIC statement executor
#[derive(Debug)]
pub struct Executor {
    variables: VariableStore,
    memory: MemoryManager,
    graphics: GraphicsSystem,
    // Control flow stack for GOSUB/RETURN and PROC/ENDPROC
    return_stack: Vec<(ReturnKind, Position)>,
    // FOR loop state: FOR and FOR EACH loops, the newest last
    for_loops: Vec<ForLoop>,
    // REPEAT loop stack: stores the positions of REPEAT statements
    repeat_stack: Vec<Position>,
    // WHILE loop stack: stores the positions of WHILE statements
    while_stack: Vec<Position>,
    // DATA storage: stores all DATA values in program order
    data_values: Vec<DataValue>,
    // DATA line numbers: tracks which line each DATA value came from (parallel to data_values)
    data_line_numbers: Vec<Option<u16>>,
    // DATA pointer: current index in the DATA of the set `data_set`
    data_pointer: usize,
    // The line set of the DATA pointer: 0 for the program (data_values),
    // n for library n (issue #186)
    data_set: usize,
    // The DATA of each library of INSTALL, by line set: the line key and the
    // value of each item, in file order (issue #186)
    library_data: HashMap<usize, Vec<(u32, DataValue)>>,
    // The DATA pointers that LOCAL DATA saved outside a routine
    saved_data: Vec<DataPointer>,
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
    // DEF line of the innermost single-line function whose body failed
    function_error_line: Option<u32>,
    // Local variable stack for PROC/FN scoping
    local_stack: Vec<LocalFrame>,
    // The number of evaluation levels (evaluators, FN and EVAL calls) open now
    evaluation_depth: usize,
    // The loop depths at the start of the newest FN call
    function_loop_floor: LoopDepths,
    // Error handler: where the run continues on error (None = no handler)
    error_handler: Option<ErrorHandler>,
    // The libraries of INSTALL (issue #185)
    libraries: LibraryStore,
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
    // True while PRINT# formats its items: a real value gets all its digits,
    // so that INPUT# reads the same value (issue #157)
    exact_reals: bool,
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
    // Lists of the program, found by handle (extension, issue #139)
    collections: CollectionStore,
    // The screen and keyboard of a front end (issue #171). None: stdout and stdin
    console: console::ConsoleSlot,
    // The sound engine and its output, from the first SOUND (issue #208)
    sound: sound::SoundSlot,
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
            data_set: 0,
            library_data: HashMap::new(),
            saved_data: Vec::new(),
            current_line: None,
            rng: RefCell::new(rand::thread_rng()),
            procedures: HashMap::new(),
            functions: HashMap::new(),
            function_body_runner: BodyRunnerSlot::default(),
            function_error_line: None,
            local_stack: Vec::new(),
            evaluation_depth: 0,
            function_loop_floor: LoopDepths::default(),
            error_handler: None,
            libraries: LibraryStore::default(),
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
            exact_reals: false,
            screen_mode: ScreenMode::from_number(0).expect("mode 0 is in the mode table"),
            // The defaults of mode 0, which has 2 colours
            text_foreground: 1,
            text_background: 0,
            vdu_queue: Vec::new(),
            collections: CollectionStore::new(),
            console: console::ConsoleSlot::default(),
            sound: sound::SoundSlot::default(),
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
            Statement::CompoundAssignment {
                target,
                operator,
                expression,
            } => self.execute_compound_assignment(target, operator, expression),
            Statement::ArrayAssignment {
                name,
                indices,
                expression,
            } => self.execute_array_assignment(name, indices, expression),
            Statement::ArrayAssign {
                target,
                operator,
                value,
            } => self.execute_array_assign(target, operator.as_ref(), value),
            Statement::Matrix {
                statement,
                target,
                source,
            } => self.execute_matrix_statement(*statement, target, source),
            Statement::MemberAssignment { target, expression } => {
                self.execute_member_assignment(target, expression)
            }
            Statement::StructureCopy { target, source } => {
                self.execute_structure_copy(target, source)
            }
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
            Statement::ForEach {
                variable,
                collection,
            } => self.execute_for_each(variable, collection),
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
            Statement::Swap { first, second } => self.execute_swap(first, second),
            Statement::Restore { line_number } => self.execute_restore(*line_number),
            Statement::LocalData => self.execute_local_data(),
            Statement::RestoreData => self.execute_restore_data(),
            // The run loop executes RESTORE +n, because it knows the line
            Statement::RestoreRelative { .. } => Err(BBCBasicError::SyntaxError {
                message: "RESTORE +n needs the run loop".to_string(),
                line: None,
            }),
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
            // ERROR makes an error with the number and the text. The error
            // number is one byte, as ERR is 0 to 255 (issue #201).
            Statement::Error { number, text } => {
                let number = (self.eval_integer(number)? & 0xFF) as u8;
                let message = self.eval_string(text)?;
                Err(BBCBasicError::UserError { number, message })
            }
            Statement::Clear => self.execute_clear(),
            // Graphics statements
            Statement::Plot { mode, x, y } => self.execute_plot(mode, x, y),
            Statement::Move { x, y } => self.execute_move(x, y),
            Statement::Draw { x, y } => self.execute_draw(x, y),
            Statement::Circle {
                x,
                y,
                radius,
                filled,
            } => self.execute_circle(x, y, radius, *filled),
            Statement::Gcol { mode, color } => self.execute_gcol(mode, color),
            Statement::Clg => self.execute_clg(),
            Statement::Mode { mode } => self.execute_mode(mode),
            Statement::Colour { colour } => self.execute_colour(colour),
            Statement::Vdu { items } => self.execute_vdu(items),
            Statement::Ellipse {
                x,
                y,
                major,
                minor,
                filled,
            } => self.execute_ellipse(x, y, major, minor, *filled),
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
            Statement::Sound {
                channel,
                amplitude,
                pitch,
                duration,
            } => self.execute_sound(channel, amplitude, pitch, duration),
            Statement::Envelope { values } => self.execute_envelope(values),
            Statement::MakeDirectory { path } => {
                crate::filesystem::make_directory(&self.eval_string(path)?)
            }
            Statement::RemoveDirectory { path } => {
                crate::filesystem::remove_directory(&self.eval_string(path)?)
            }
            Statement::ListAdd { list, value } => {
                let list = self.eval_integer(list)?;
                let value = self.eval_list_element(value)?;
                self.collections.add(list, value)
            }
            Statement::ListSet { list, index, value } => {
                let list = self.eval_integer(list)?;
                let index = self.eval_integer(index)?;
                let value = self.eval_list_element(value)?;
                self.collections.set(list, index, value)
            }
            Statement::ListInsert { list, index, value } => {
                let list = self.eval_integer(list)?;
                let index = self.eval_integer(index)?;
                let value = self.eval_list_element(value)?;
                self.collections.insert(list, index, value)
            }
            Statement::ListRemove { list, index } => {
                let list = self.eval_integer(list)?;
                let index = self.eval_integer(index)?;
                self.collections.remove(list, index)
            }
            Statement::ListClear { list } => {
                let list = self.eval_integer(list)?;
                self.collections.clear_list(list)
            }
            Statement::MapPut { map, key, value } => {
                let map = self.eval_integer(map)?;
                let key = self.eval_string(key)?;
                let value = self.eval_list_element(value)?;
                self.collections.put(map, &key, value)
            }
            Statement::MapRemove { map, key } => {
                let map = self.eval_integer(map)?;
                let key = self.eval_string(key)?;
                self.collections.remove_key(map, &key)
            }
            Statement::MapClear { map } => {
                let map = self.eval_integer(map)?;
                self.collections.clear_map(map)
            }
            Statement::ListAddCollection { list, handle } => {
                let list = self.eval_integer(list)?;
                let handle = self.eval_integer(handle)?;
                let element = self.collections.collection_element(handle)?;
                self.collections.add(list, element)
            }
            Statement::MapPutCollection { map, key, handle } => {
                let map = self.eval_integer(map)?;
                let key = self.eval_string(key)?;
                let handle = self.eval_integer(handle)?;
                let element = self.collections.collection_element(handle)?;
                self.collections.put(map, &key, element)
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
            // The run loop executes INSTALL, because it adds a line set
            // (issue #185). With no run loop, INSTALL cannot work.
            Statement::Install { .. } => Err(BBCBasicError::SyntaxError {
                message: "INSTALL needs the run loop".to_string(),
                line: None,
            }),
            Statement::Trace { mode } => self.execute_trace(mode),
            Statement::PrintFile { handle, items } => self.execute_print_file(handle, items),
            Statement::InputFile { handle, targets } => self.execute_input_file(handle, targets),
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

    /// Execute a PRINT statement.
    ///
    /// The field rules are the rules of BBC BASIC 2 (the PRINT routine at
    /// &8D83 to &8E1F of the ROM, and the PRINT page of the BBC Micro User
    /// Guide). At the start of the statement, a number prints at the right of a
    /// field of the width in byte 1 of `@%` (issue 166). A `;` turns the field off, so
    /// that the numbers after it have no padding. A `,` first prints spaces up
    /// to the next field boundary (none if the column is on a boundary), then
    /// turns the field on again. A string is never padded. `TAB`, `SPC` and `'`
    /// do not change the field.
    fn execute_print(&mut self, items: &[crate::parser::PrintItem]) -> Result<()> {
        use crate::parser::PrintItem;

        // After ~, numeric items print in hex until the next comma or semicolon
        let mut hex = false;
        // The width of the field of the next number: 0 after a semicolon
        let mut field = self.number_format().width;
        for item in items {
            match item {
                PrintItem::Expression(expr) => {
                    // Evaluate expression and print it
                    let (output, number) = self.format_print_item(expr, hex)?;
                    if number {
                        self.print_output(&pad_to_field(&output, field));
                    } else {
                        self.print_output(&output);
                    }
                }
                PrintItem::Hex => hex = true,
                PrintItem::Semicolon => {
                    // Semicolon suppresses newline and turns the field off
                    hex = false;
                    field = 0;
                }
                PrintItem::Comma => {
                    hex = false;
                    field = self.number_format().width;
                    // Comma moves to the next field boundary. A width of 0
                    // has no boundaries (ROM &8DA6).
                    if field != 0 {
                        let past_boundary = self.column % field;
                        if past_boundary != 0 {
                            self.print_output(&" ".repeat(field - past_boundary));
                        }
                    }
                }
                PrintItem::NewLine => self.print_new_line(),
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
    /// A string item does not change. The flag is true for a number.
    fn format_print_item(&mut self, expr: &Expression, hex: bool) -> Result<(String, bool)> {
        if hex && !self.is_string_expr(expr) {
            Ok((format_hex(real_to_integer(self.eval_number(expr)?)?), true))
        } else {
            self.format_expression(expr)
        }
    }

    /// Format an expression for printing. The flag is true for a number.
    fn format_expression(&mut self, expr: &Expression) -> Result<(String, bool)> {
        match expr {
            Expression::Integer(_) => self.format_integer_item(expr),
            Expression::Real(_) => self.format_real_item(expr),
            Expression::String(_) => Ok((self.eval_string(expr)?, false)),
            Expression::Member(path) => self.format_member(path),
            Expression::Variable(name) => {
                if name.ends_with('%') {
                    self.format_integer_item(expr)
                } else if name.ends_with('$') {
                    Ok((self.eval_string(expr)?, false))
                } else {
                    self.format_real_item(expr)
                }
            }
            // VAL gives a real value, so do not try it as an integer first
            Expression::FunctionCall { name, .. } if name == "VAL" => self.format_real_item(expr),
            // A user function with no $ in its name can give a string or a
            // number. Run its body one time and format the result by its type.
            Expression::FunctionCall { name, args }
                if !name.ends_with('$') && self.functions.contains_key(name) =>
            {
                self.call_function(name, args, Self::format_function_result)
            }
            // The type of an EVAL result is known only after the parse of its
            // text. Parse the text one time and format the result by its type.
            Expression::FunctionCall { name, args } if name == "EVAL" => {
                self.nested_call(|executor| {
                    let parsed = executor.parse_eval_text(args)?;
                    executor.format_expression(&parsed)
                })
            }
            // Select the evaluator by the type of the expression. Check for a
            // string first, because `is_real_expr` is true for each user
            // function, also for a string function such as FNx$.
            _ if self.is_string_expr(expr) => Ok((self.eval_string(expr)?, false)),
            _ if self.is_real_expr(expr) => self.format_real_item(expr),
            _ => self.format_integer_item(expr),
        }
    }

    /// Format a real expression for printing, with the number flag. PRINT
    /// uses the format of `@%`. PRINT# uses the text that reads back.
    fn format_real_item(&mut self, expr: &Expression) -> Result<(String, bool)> {
        let value = self.eval_real(expr)?;
        let text = if self.exact_reals {
            format_real(value, true)
        } else {
            self.number_format().format_real(value)
        };
        Ok((text, true))
    }

    /// Format an integer expression for printing, with the number flag. PRINT
    /// uses the format of `@%`. PRINT# shows all the digits.
    fn format_integer_item(&mut self, expr: &Expression) -> Result<(String, bool)> {
        let value = self.eval_integer(expr)?;
        let text = if self.exact_reals {
            value.to_string()
        } else {
            self.number_format().format_integer(value)
        };
        Ok((text, true))
    }

    /// The number format of the value of `@%` (issue 166)
    fn number_format(&self) -> NumberFormat {
        let value = self
            .variables
            .get_integer_var(PRINT_FORMAT_NAME)
            .unwrap_or(PRINT_FORMAT_DEFAULT);
        NumberFormat::from_value(value)
    }

    /// The number format of STR$: the format of `@%` if its byte 4 is not 0,
    /// otherwise the general format with 9 digits (ROM &B0AC)
    fn str_format(&self) -> NumberFormat {
        let format = self.number_format();
        if format.for_str {
            format
        } else {
            STR_FORMAT
        }
    }

    /// Format the `=` result of a user function for PRINT. A string prints as
    /// text, and a number prints as a real value. The flag is true for a number.
    fn format_function_result(&mut self, expr: &Expression) -> Result<(String, bool)> {
        match expr {
            _ if self.is_string_expr(expr) => Ok((self.eval_string(expr)?, false)),
            // The result is the result of a second function: format it by its type
            Expression::FunctionCall { name, .. }
                if !name.ends_with('$') && self.functions.contains_key(name) =>
            {
                self.format_expression(expr)
            }
            _ => self.format_real_item(expr),
        }
    }

    /// Print output (to buffer in test mode, to stdout in production).
    /// A line feed (code 10) keeps the column. A carriage return (code 13)
    /// sets it to 0. A console with a text cursor gets the text as VDU codes.
    fn print_output(&mut self, text: &str) {
        if self.print_to_screen(text) {
            return;
        }
        for ch in text.chars() {
            match ch {
                '\n' => {}
                '\r' => self.column = 0,
                _ => self.column += 1,
            }
        }
        self.write_text(text);
    }

    /// Give the screen and the keyboard of a front end to the executor. The
    /// program text then goes to the console and not to the output buffer.
    pub fn set_console(&mut self, console: Box<dyn Console>) {
        self.console.0 = Some(console);
    }

    /// The stop check: give the pending interrupt of the console, if there
    /// is one. With no console, there is never an interrupt. First the
    /// console sees the graphics canvas (issue #173).
    pub fn take_interrupt(&mut self) -> Option<Interrupt> {
        let console = self.console.0.as_mut()?;
        console.see_graphics(&self.graphics, self.screen_mode);
        console.interrupt()
    }

    /// Show text with no change to the print column: to the console, or to
    /// the output buffer in test mode and to stdout in production. The
    /// commands of the prompt (LIST, LVAR and the others) use it.
    pub fn write_text(&mut self, text: &str) {
        if let Some(console) = self.console.0.as_mut() {
            console.write(text);
            return;
        }
        self.output.push_str(text);
        #[cfg(not(test))]
        {
            print!("{}", text);
        }
    }

    /// Read one typed line for a command, such as a line of AUTO: from the
    /// console, or from stdin. Give None at the end of input, or if an
    /// interrupt stopped the read.
    pub fn read_text_line(&mut self) -> Option<String> {
        if let Some(console) = self.console.0.as_mut() {
            let line = console.read_line();
            return line.filter(|_| console.interrupt().is_none());
        }
        #[cfg(test)]
        {
            None
        }
        #[cfg(not(test))]
        {
            use std::io;

            io::stdout().flush().unwrap();
            let mut line = String::new();
            match io::stdin().read_line(&mut line) {
                Ok(0) | Err(_) => None,
                Ok(_) => Some(line),
            }
        }
    }

    /// Read one key from the console, and give the error Escape if an
    /// interrupt stopped the read.
    fn console_key(&mut self, limit: Option<Duration>) -> Option<Result<Option<char>>> {
        let console = self.console.0.as_mut()?;
        let key = console.read_key(limit);
        Some(match console.interrupt() {
            Some(_) => Err(BBCBasicError::Escape),
            None => Ok(key),
        })
    }

    /// Print a new line (BBC codes 13 and 10) and set the column to 0
    fn print_new_line(&mut self) {
        // Not the VDU path: in a console, code 10 does not go to column 0
        self.write_text("\n");
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
    /// from the console or stdin. Give None at the end of input. An
    /// interrupt of the console read gives the error Escape.
    fn next_key(&mut self) -> Result<Option<char>> {
        if let Some(key) = self.key_queue.pop_front() {
            return Ok(Some(key));
        }
        if let Some(key) = self.console_key(None) {
            return key;
        }
        #[cfg(test)]
        {
            Ok(None)
        }
        #[cfg(not(test))]
        {
            Ok(key_input::read_key().map(char::from))
        }
    }

    /// Take the next key from the queue with no wait. If the queue is empty,
    /// wait for a maximum of `limit` for one key from the console or stdin.
    /// Give None if no key comes in that time or at the end of input. An
    /// interrupt of the console read gives the error Escape.
    fn next_key_within(&mut self, limit: Duration) -> Result<Option<char>> {
        if let Some(key) = self.key_queue.pop_front() {
            return Ok(Some(key));
        }
        if let Some(key) = self.console_key(Some(limit)) {
            return key;
        }
        #[cfg(test)]
        {
            let _ = limit;
            Ok(None)
        }
        #[cfg(not(test))]
        {
            Ok(key_input::read_key_within(limit).map(char::from))
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
        variable: &InputItem,
        start: &Expression,
        end: &Expression,
        step: Option<&Expression>,
    ) -> Result<()> {
        // The indices of an element or a member are evaluated one time, here
        let target = self.for_target(variable)?;
        // An integer loop variable is an integer loop: the values are truncated.
        // Any other loop variable is a real loop: the values keep their fraction parts.
        let integer_loop = self.for_target_is_integer(&target)?;
        let (start_val, end_val, step_val) = if integer_loop {
            let start_val = self.eval_integer(start)?;
            let end_val = self.eval_integer(end)?;
            let step_val = match step {
                Some(step_expr) => self.eval_integer(step_expr)?,
                None => 1,
            };
            (
                f64::from(start_val),
                f64::from(end_val),
                f64::from(step_val),
            )
        } else {
            let start_val = self.eval_number(start)?;
            let end_val = self.eval_number(end)?;
            let step_val = match step {
                Some(step_expr) => self.eval_number(step_expr)?,
                None => 1.0,
            };
            (start_val, end_val, step_val)
        };

        // Make room first, so that a FOR that fails changes nothing
        self.open_for_loop(&target)?;
        let start_value = if integer_loop {
            // The value came from an i32, so the conversion has no loss
            Variable::Integer(start_val as i32)
        } else {
            Variable::Real(start_val)
        };
        self.store_for_value(&target, start_value)?;

        // The run loop sets the FOR position (set_for_loop_line)
        self.for_loops.push(ForLoop {
            target,
            kind: ForKind::Count {
                end: end_val,
                step: step_val,
            },
            position: Position::line_start(0),
        });

        Ok(())
    }

    /// Execute `FOR EACH variable IN collection` (issue #141). The
    /// collection is evaluated one time. If it has a first element, the
    /// variable gets it. The loop goes on the FOR stack also when the
    /// collection is empty: the run loop then skips to the matching NEXT.
    fn execute_for_each(&mut self, variable: &str, collection: &Expression) -> Result<()> {
        let handle = self.eval_integer(collection)?;
        // Make room first, as FOR does (issue 151)
        let target = ForTarget::Variable(variable.to_string());
        self.open_for_loop(&target)?;
        self.set_each_variable(variable, handle, 0)?;
        // The run loop sets the FOR position (set_for_loop_line)
        self.for_loops.push(ForLoop {
            target,
            kind: ForKind::Each { handle, index: 0 },
            position: Position::line_start(0),
        });
        Ok(())
    }

    /// The count of elements of a list or of keys of a map at this time.
    fn collection_count(&self, handle: i32) -> Result<usize> {
        self.collections
            .length(handle)
            .or_else(|_| self.collections.map_length(handle))
    }

    /// Give the FOR EACH variable the element at `index` of a list, or the
    /// key at `index` of a map. If `index` is not below the count at this
    /// time, change nothing and return false. A numeric variable reads an
    /// element as LISTGET does, and a string variable as LISTGET$ does: the
    /// wrong type gives Type mismatch. A map needs a string variable.
    fn set_each_variable(&mut self, variable: &str, handle: i32, index: usize) -> Result<bool> {
        let is_map = self.collections.length(handle).is_err();
        let count = self.collection_count(handle)?;
        let is_string = variable.ends_with('$');
        if is_map && !is_string {
            return Err(BBCBasicError::TypeMismatch);
        }
        if index >= count {
            return Ok(false);
        }
        let index = i32::try_from(index).map_err(|_| BBCBasicError::TooBig)?;
        if is_string {
            let text = if is_map {
                self.collections.map_key(handle, index)?
            } else {
                self.collections.get_text(handle, index)?
            };
            self.variables.set_string_var(variable.to_string(), text)?;
        } else {
            let value = self.collections.get_number(handle, index)?;
            if variable.ends_with('%') {
                self.variables
                    .set_integer_var(variable.to_string(), real_to_integer(value)?);
            } else {
                self.variables.set_real_var(variable.to_string(), value);
            }
        }
        Ok(true)
    }

    /// Step a FOR EACH loop to the element at `index`. NEXT reads the count
    /// at this time, so the loop sees the elements that its body adds.
    fn next_each(&mut self, loop_index: usize, handle: i32, index: usize) -> Result<()> {
        // Store the index first: after a trapped error, the next NEXT goes
        // to the element after this one. As for a FOR loop, an error does
        // not close the inner loops.
        self.for_loops[loop_index].kind = ForKind::Each { handle, index };
        // A FOR EACH loop has a plain variable (execute_for_each)
        let ForTarget::Variable(variable) = self.for_loops[loop_index].target.clone() else {
            return Err(BBCBasicError::TypeMismatch);
        };
        let has_element = self.set_each_variable(&variable, handle, index)?;
        // A NEXT for an outer loop closes the loops inside it
        self.for_loops.truncate(loop_index + 1);
        if has_element {
            self.next_loop_line = Some(self.for_loops[loop_index].position);
        } else {
            self.for_loops.remove(loop_index);
            self.next_loop_line = None;
        }
        Ok(())
    }

    /// Tell if the newest FOR loop is a FOR EACH loop with no element at
    /// its index: a FOR EACH of an empty collection (issue #141).
    pub fn for_each_is_empty(&self) -> bool {
        match self.for_loops.last() {
            Some(ForLoop {
                kind: ForKind::Each { handle, index },
                ..
            }) => self
                .collection_count(*handle)
                .map_or(true, |count| *index >= count),
            _ => false,
        }
    }

    /// Remove the newest FOR loop: a FOR EACH loop with no matching NEXT.
    pub fn drop_newest_for_loop(&mut self) {
        self.for_loops.pop();
    }

    /// Execute NEXT statement
    ///
    /// `NEXT a, b` handles each name in order. It stops at the first loop
    /// that continues. If each loop ends, the program continues after the NEXT.
    fn execute_next(&mut self, variables: &[InputItem]) -> Result<()> {
        // If no variables specified, use the most recent FOR loop
        if variables.is_empty() {
            let Some(newest) = self.for_loops.last() else {
                return Err(BBCBasicError::NoFor);
            };
            return self.next_loop(newest.target.clone());
        }

        for variable in variables {
            // The indices of a named element are evaluated now (issue #198)
            let target = self.for_target(variable)?;
            self.next_loop(target)?;
            if self.next_loop_line.is_some() {
                break;
            }
        }

        Ok(())
    }

    /// Step the FOR loop of one target. Set the position to jump back to
    /// if the loop continues, or clear it if the loop ends.
    fn next_loop(&mut self, target: ForTarget) -> Result<()> {
        // Find the matching FOR loop. A loop of the variable EACH keeps the
        // old meaning of `NEXT EACH`. With no such loop, `NEXT EACH` matches
        // the newest FOR EACH loop (issue #141).
        let loop_index = self
            .for_loops
            .iter()
            .rposition(|entry| entry.target == target)
            .or_else(|| {
                matches!(&target, ForTarget::Variable(name) if name == "EACH")
                    .then(|| {
                        self.for_loops
                            .iter()
                            .rposition(|entry| matches!(entry.kind, ForKind::Each { .. }))
                    })
                    .flatten()
            })
            .ok_or(if self.for_loops.is_empty() {
                BBCBasicError::NoFor
            } else {
                BBCBasicError::CantMatchFor
            })?;

        let (end_val, step_val) = match self.for_loops[loop_index].kind {
            ForKind::Count { end, step } => (end, step),
            ForKind::Each { handle, index } => {
                return self.next_each(loop_index, handle, index + 1)
            }
        };

        // Increment the loop variable
        let target = self.for_loops[loop_index].target.clone();
        let current = self.for_value(&target)?;
        let next_val = match current {
            Variable::Integer(current_val) if self.for_target_is_integer(&target)? => {
                // The step of an integer loop is an i32 value (see execute_for).
                // If the next value overflows, it is past the end value. The loop
                // ends and the variable keeps its last value.
                match current_val.checked_add(step_val as i32) {
                    Some(next_val) => {
                        self.store_for_value(&target, Variable::Integer(next_val))?;
                        f64::from(next_val)
                    }
                    None => f64::from(current_val) + step_val,
                }
            }
            Variable::Integer(current_val) => {
                let next_val = f64::from(current_val) + step_val;
                self.store_for_value(&target, Variable::Real(next_val))?;
                next_val
            }
            Variable::Real(current_val) => {
                let next_val = current_val + step_val;
                self.store_for_value(&target, Variable::Real(next_val))?;
                next_val
            }
            _ => return Err(BBCBasicError::TypeMismatch),
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
            self.next_loop_line = Some(self.for_loops[loop_index].position);
        }

        Ok(())
    }

    /// The FOR target of a FOR or NEXT name, with its index values evaluated
    /// now. The manual of BBC BASIC for Windows (bbcwin5.html, FOR): "Any
    /// numeric assignable item may be used as the control variable".
    fn for_target(&mut self, item: &InputItem) -> Result<ForTarget> {
        let (name, place) = self.swap_location(item)?;
        Ok(match place {
            SwapPlace::Variable => ForTarget::Variable(name.to_string()),
            SwapPlace::Element(indices) => ForTarget::Element(name.to_string(), indices),
            SwapPlace::Member(path, indices) => ForTarget::Member {
                structure: path.structure.clone(),
                members: path.members.clone(),
                indices,
            },
        })
    }

    /// Tell if a FOR target holds an integer. A variable has the type of its
    /// name. An element or a member has the type of its value, so it must
    /// exist. A string target gives Type mismatch.
    fn for_target_is_integer(&self, target: &ForTarget) -> Result<bool> {
        let value = match target {
            ForTarget::Variable(name) if name.ends_with('$') => {
                return Err(BBCBasicError::TypeMismatch)
            }
            ForTarget::Variable(name) => return Ok(name.ends_with('%')),
            _ => self.for_value(target)?,
        };
        match value {
            Variable::Integer(_) => Ok(true),
            Variable::Real(_) => Ok(false),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The value of a FOR target. A variable that does not exist gives
    /// "No such variable".
    fn for_value(&self, target: &ForTarget) -> Result<Variable> {
        match target {
            ForTarget::Variable(name) => self.swap_value(&(name, SwapPlace::Variable)),
            ForTarget::Element(name, indices) => self.variables.get_array_element(name, indices),
            ForTarget::Member {
                structure,
                members,
                indices,
            } => self
                .variables
                .get_member(structure, &indices.element, members, &indices.member),
        }
    }

    /// Store a value of the type of the target in a FOR target
    fn store_for_value(&mut self, target: &ForTarget, value: Variable) -> Result<()> {
        match target {
            ForTarget::Variable(name) => self.store_input_value(name, None, value),
            ForTarget::Element(name, indices) => {
                self.variables.set_array_element(name, indices, value)
            }
            ForTarget::Member {
                structure,
                members,
                indices,
            } => self.variables.set_member(
                structure,
                &indices.element,
                members,
                &indices.member,
                value,
            ),
        }
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
            let var = match item {
                InputItem::Prompt {
                    text,
                    question_mark: shows,
                } => {
                    self.print_output(text);
                    question_mark = *shows;
                    continue;
                }
                InputItem::NewLine => {
                    self.print_new_line();
                    continue;
                }
                InputItem::Variable(var) => var,
                InputItem::ArrayElement { name, .. } => name,
                InputItem::Member(path) => path.member(),
            };
            let member_indices = self.check_member_target(item)?;
            if values.is_empty() {
                let reply = self.read_input_reply(question_mark)?;
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
            self.store_target(item, &member_indices, value)?;
        }
        Ok(())
    }

    /// Store a value in the target of INPUT, READ or INPUT#: a variable, an
    /// array element or a structure member (issue #191). A member uses the
    /// indices that `check_member_target` gave, so that each index is
    /// evaluated one time (issue 202).
    fn store_target(
        &mut self,
        target: &InputItem,
        member_indices: &MemberIndices,
        value: Variable,
    ) -> Result<()> {
        match target {
            InputItem::Variable(name) => self.store_input_value(name, None, value),
            InputItem::ArrayElement { name, indices } => {
                self.store_input_value(name, Some(indices), value)
            }
            InputItem::Member(path) => self.store_member(path, member_indices, value),
            // A prompt text and a new line store no value
            InputItem::Prompt { .. } | InputItem::NewLine => Ok(()),
        }
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
            let var = match item {
                InputItem::Prompt {
                    text,
                    question_mark: shows,
                } => {
                    self.print_output(text);
                    question_mark = *shows;
                    continue;
                }
                InputItem::NewLine => {
                    self.print_new_line();
                    continue;
                }
                InputItem::Variable(var) => var,
                InputItem::ArrayElement { name, .. } => name,
                InputItem::Member(path) => path.member(),
            };
            let member_indices = self.check_member_target(item)?;
            if !var.ends_with('$') {
                return Err(BBCBasicError::TypeMismatch);
            }
            let reply = self.read_input_reply(question_mark)?;
            question_mark = true;
            let reply = without_line_end(&reply);
            self.store_target(item, &member_indices, Variable::String(reply.to_string()))?;
        }
        Ok(())
    }

    /// Show `?` if `question_mark` is true, then read the next reply line.
    /// The Enter key of the reply moves the cursor to column 0.
    fn read_input_reply(&mut self, question_mark: bool) -> Result<String> {
        if question_mark {
            self.print_output("?");
        }
        let reply = self.next_input_reply()?;
        self.column = 0;
        Ok(reply)
    }

    /// Take the next INPUT reply from the queue. If the queue is empty, read stdin.
    /// A read error or the end of input gives an empty reply. A queued reply
    /// adds a new line to the output, as the Enter key does on a screen.
    /// A console reads the reply, if there is one. An interrupt of the
    /// console read gives the error Escape.
    fn next_input_reply(&mut self) -> Result<String> {
        if let Some(reply) = self.input_queue.pop_front() {
            self.print_new_line();
            return Ok(reply);
        }
        if let Some(console) = self.console.0.as_mut() {
            let reply = console.read_line();
            return match (reply, console.interrupt()) {
                (Some(reply), None) => Ok(reply),
                (None, None) => Ok(String::new()),
                (_, Some(_)) => Err(BBCBasicError::Escape),
            };
        }
        #[cfg(test)]
        {
            Ok(String::new())
        }
        #[cfg(not(test))]
        {
            use std::io;

            let _ = io::stdout().flush();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                input.clear();
            }
            Ok(input)
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
                DimItem::Structure { name, members } => {
                    self.execute_dim_structure(name, members)?;
                    continue;
                }
                DimItem::StructureArray {
                    name,
                    dimensions,
                    members,
                } => {
                    self.execute_dim_structure_array(name, dimensions, members)?;
                    continue;
                }
                DimItem::StructurePrototype {
                    name,
                    dimensions,
                    prototype,
                } => {
                    self.execute_dim_prototype(name, dimensions, prototype)?;
                    continue;
                }
            };
            let dim_sizes = self.eval_dimensions(dimensions)?;

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

    /// The element counts of the dimensions of a DIM, an array or an array
    /// member. DIM A(N) gives the elements 0 to N, so N + 1 elements. A
    /// negative size gives Bad DIM, as BBC BASIC does (issue 202).
    fn eval_dimensions(&mut self, dimensions: &[Expression]) -> Result<Vec<usize>> {
        let mut dim_sizes = Vec::with_capacity(dimensions.len());
        for dim_expr in dimensions {
            let size = self.eval_integer(dim_expr)?;
            if size < 0 {
                return Err(BBCBasicError::BadDim);
            }
            dim_sizes.push(size as usize + 1);
        }
        Ok(dim_sizes)
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
        self.data_set = 0;
        self.library_data.clear();
        self.saved_data.clear();
    }

    /// Use the DATA of the library with line set `set`: the line key and
    /// the value of each item (issue #186). If the DATA is the same as
    /// before, the DATA pointer stays.
    pub fn use_library_data(&mut self, set: usize, items: Vec<(u32, DataValue)>) {
        if self.library_data.get(&set) == Some(&items) {
            return;
        }
        if self.data_set == set {
            self.data_pointer = 0;
        }
        self.library_data.insert(set, items);
    }

    /// The next DATA item of the set of the DATA pointer. The end of the
    /// DATA of the set gives Out of DATA: a READ in the program does not
    /// read the DATA of a library.
    fn next_data_value(&mut self) -> Result<DataValue> {
        let value = if self.data_set == 0 {
            self.data_values.get(self.data_pointer).cloned()
        } else {
            self.library_data
                .get(&self.data_set)
                .and_then(|items| items.get(self.data_pointer))
                .map(|(_, value)| value.clone())
        };
        let value = value.ok_or(BBCBasicError::OutOfData)?;
        self.data_pointer += 1;
        Ok(value)
    }

    /// RESTORE +n (issue #186): move the DATA pointer to the first item at
    /// or after the line key `target` of line set `set`. With no target, or
    /// no DATA at or after it in the set, the next READ gives Out of DATA.
    pub fn restore_data_to(&mut self, set: usize, target: Option<u32>) {
        let keys: Vec<u32> = if set == 0 {
            let numbers = self.data_line_numbers.iter();
            numbers.map(|line| line.map_or(0, u32::from)).collect()
        } else {
            let items = self.library_data.get(&set).map_or(&[][..], Vec::as_slice);
            items.iter().map(|(key, _)| *key).collect()
        };
        self.data_set = set;
        self.data_pointer = target
            .and_then(|target| keys.iter().position(|&key| key >= target))
            .unwrap_or(keys.len());
    }

    /// LOCAL DATA: save the DATA pointer. In a routine, the exit of the
    /// routine restores it. Outside a routine, RESTORE DATA restores it.
    /// Loops do not restore it, as the manual says they do (issue #186).
    fn execute_local_data(&mut self) -> Result<()> {
        let pointer = (self.data_set, self.data_pointer);
        match self.local_stack.last_mut() {
            Some(frame) => frame.saved_data.push(pointer),
            None => self.saved_data.push(pointer),
        }
        Ok(())
    }

    /// RESTORE DATA: restore the last DATA pointer that LOCAL DATA saved.
    /// With no saved pointer, the error is Not LOCAL.
    fn execute_restore_data(&mut self) -> Result<()> {
        let saved = match self.local_stack.last_mut() {
            Some(frame) if !frame.saved_data.is_empty() => frame.saved_data.pop(),
            _ => self.saved_data.pop(),
        };
        (self.data_set, self.data_pointer) = saved.ok_or(BBCBasicError::NotLocal)?;
        Ok(())
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
        self.data_set = 0;
    }

    /// Execute READ statement - reads data into variables and array elements.
    /// The indices of an element are evaluated when it gets its value, so
    /// `READ I, A(I)` uses the new I (issue #153).
    fn execute_read(&mut self, variables: &[InputItem]) -> Result<()> {
        for target in variables {
            let var_name = match target {
                InputItem::Variable(name) => name,
                InputItem::ArrayElement { name, .. } => name,
                InputItem::Member(path) => path.member(),
                InputItem::Prompt { .. } | InputItem::NewLine => {
                    return Err(BBCBasicError::SyntaxError {
                        message: "Expected variable name in READ".to_string(),
                        line: None,
                    })
                }
            };
            let member_indices = self.check_member_target(target)?;
            let data_value = &self.next_data_value()?;

            // Convert the value to the type of the target
            let value = if var_name.ends_with('%') {
                Variable::Integer(match data_value {
                    DataValue::Integer(v) => *v,
                    DataValue::Real(v) => *v as i32,
                    DataValue::String(s) => parse_leading_number(s) as i32,
                })
            } else if var_name.ends_with('$') {
                Variable::String(match data_value {
                    DataValue::String(s) => s.clone(),
                    DataValue::Integer(v) => v.to_string(),
                    DataValue::Real(v) => v.to_string(),
                })
            } else {
                Variable::Real(match data_value {
                    DataValue::Real(v) => *v,
                    DataValue::Integer(v) => *v as f64,
                    DataValue::String(s) => parse_leading_number(s),
                })
            };
            self.store_target(target, &member_indices, value)?;
        }
        Ok(())
    }

    /// Execute SWAP (issue #162). The indices of the 2 targets are evaluated
    /// one time, before the exchange. The 2 values are read and converted
    /// before a target changes, so an error changes no target.
    fn execute_swap(&mut self, first: &InputItem, second: &InputItem) -> Result<()> {
        let first = self.swap_location(first)?;
        let second = self.swap_location(second)?;
        let first_value = self.swap_value(&first)?;
        let second_value = self.swap_value(&second)?;
        let new_first = convert_to_type_of(first.0, second_value)?;
        let new_second = convert_to_type_of(second.0, first_value)?;
        self.store_swap_value(&first, new_first)?;
        self.store_swap_value(&second, new_second)
    }

    /// The name of a SWAP target and, for an array element, its index values
    fn swap_location<'a>(&mut self, target: &'a InputItem) -> Result<SwapLocation<'a>> {
        match target {
            InputItem::Variable(name) => Ok((name, SwapPlace::Variable)),
            InputItem::Member(path) => {
                let indices = self.member_indices(path)?;
                Ok((path.member(), SwapPlace::Member(path, indices)))
            }
            InputItem::ArrayElement { name, indices } => {
                let mut index_values = Vec::with_capacity(indices.len());
                for index_expr in indices {
                    let idx = self.eval_integer(index_expr)?;
                    if idx < 0 {
                        return Err(BBCBasicError::SubscriptOutOfRange);
                    }
                    index_values.push(idx as usize);
                }
                Ok((name, SwapPlace::Element(index_values)))
            }
            // The parser gives no prompt text and no ' in SWAP
            InputItem::Prompt { .. } | InputItem::NewLine => Err(BBCBasicError::SyntaxError {
                message: "Expected variable name in SWAP".to_string(),
                line: None,
            }),
        }
    }

    /// The value of a SWAP target. A variable that does not exist gives
    /// "No such variable".
    fn swap_value(&self, (name, place): &SwapLocation) -> Result<Variable> {
        match place {
            SwapPlace::Element(indices) => return self.variables.get_array_element(name, indices),
            SwapPlace::Member(path, indices) => return indices.get(&self.variables, path),
            SwapPlace::Variable => {}
        }
        let value = if name.ends_with('$') {
            self.variables
                .get_string_var(name)
                .map(|value| Variable::String(value.to_string()))
        } else if name.ends_with('%') {
            self.variables.get_integer_var(name).map(Variable::Integer)
        } else {
            self.variables
                .get_real_var(name)
                .map(Variable::Real)
                .or_else(|| self.variables.get_integer_var(name).map(Variable::Integer))
        };
        value.ok_or_else(|| BBCBasicError::NoSuchVariable(name.to_string()))
    }

    /// Store a converted value in a SWAP target
    fn store_swap_value(&mut self, (name, place): &SwapLocation, value: Variable) -> Result<()> {
        match place {
            SwapPlace::Element(indices) => self.variables.set_array_element(name, indices, value),
            SwapPlace::Member(path, indices) => indices.set(&mut self.variables, path, value),
            SwapPlace::Variable => self.store_input_value(name, None, value),
        }
    }

    /// Execute a compound assignment such as `S% += 2` (issue #204). The
    /// target has the form of a SWAP target, so its indices are evaluated
    /// one time. `A += B` is `A = A + (B)`, with the conversion of a normal
    /// assignment. A variable that does not exist starts at 0 or "", as the
    /// manual, page bbcwin2.html, "Creation of variables", shows with
    /// `Value += 1`.
    fn execute_compound_assignment(
        &mut self,
        target: &InputItem,
        operator: &BinaryOperator,
        expression: &Expression,
    ) -> Result<()> {
        let location = self.swap_location(target)?;
        let old = match self.swap_value(&location) {
            Err(BBCBasicError::NoSuchVariable(_)) if matches!(location.1, SwapPlace::Variable) => {
                Self::zero_value(location.0)
            }
            value => value?,
        };
        let old = match old {
            Variable::Integer(value) => Expression::Integer(value),
            Variable::Real(value) => Expression::Real(value),
            Variable::String(value) => Expression::String(value),
            _ => return Err(BBCBasicError::TypeMismatch),
        };
        let combined = Expression::BinaryOp {
            op: operator.clone(),
            left: Box::new(old),
            right: Box::new(expression.clone()),
        };
        let name = location.0;
        let value = if name.ends_with('%') {
            Variable::Integer(self.eval_integer(&combined)?)
        } else if name.ends_with('$') {
            Variable::String(self.eval_string(&combined)?)
        } else {
            Variable::Real(self.eval_real(&combined)?)
        };
        self.store_swap_value(&location, value)
    }

    /// Execute RESTORE statement - resets data pointer
    fn execute_restore(&mut self, line_number: Option<u16>) -> Result<()> {
        // RESTORE and RESTORE n go to the DATA of the program (issue #186)
        self.data_set = 0;
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
    /// LOCAL scopes, remove all lists, move the DATA pointer to the first value and remove a
    /// VDU command that is not complete. The
    /// resident integer variables A% to Z%, the procedures and the
    /// functions stay. CLEAR, RUN, NEW, LOAD and CHAIN use this.
    pub fn clear_variables(&mut self) {
        self.vdu_queue.clear();
        self.collections.clear();
        self.variables.clear();
        self.reset_dim_space();
        self.clear_control_stacks();
        self.local_stack.clear();
        self.data_pointer = 0;
        self.data_set = 0;
        self.saved_data.clear();
    }

    /// The LVAR text: one line for each scalar variable, then one line for
    /// each array with its top index in each dimension, in name order.
    /// A resident integer variable (@% and A% to Z%) is not listed when it has
    /// its start value.
    pub fn variable_listing(&self) -> Vec<String> {
        let scalars = self
            .variables
            .scalars()
            .into_iter()
            .filter(|(name, variable)| {
                !(is_resident_integer(name)
                    && **variable == Variable::Integer(resident_default(name)))
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
        // The members of each structure follow the arrays (issue #191)
        scalars
            .chain(arrays)
            .chain(self.structure_listing())
            .collect()
    }

    /// Execute CLS statement - clear screen
    fn execute_cls(&mut self) -> Result<()> {
        if let Some(console) = self.console.0.as_mut() {
            console.clear();
            self.column = 0;
            return Ok(());
        }
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
        if let Some(console) = self.console.0.as_mut() {
            console.screen_mode(screen_mode);
        }
        self.send_text_colours();
        self.execute_cls()
    }

    /// Give the text colours to the console, if there is one (issue #174).
    fn send_text_colours(&mut self) {
        if let Some(console) = self.console.0.as_mut() {
            console.text_colours(self.text_foreground, self.text_background);
        }
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
    /// terminal is out of scope (issue #101). A console gets the colours: the
    /// IDE shows them (issue #174). Mode 7 is teletext: the MOS ignores
    /// VDU 17 there (issue #199).
    fn set_text_colour(&mut self, byte: u8) {
        if ScreenMode::from_number(7) == Some(self.screen_mode) {
            return;
        }
        let colour = (byte & 0x7F) % self.screen_mode.colours;
        if byte < 128 {
            self.text_foreground = colour;
        } else {
            self.text_background = colour;
        }
        self.send_text_colours();
    }

    /// The current screen mode
    pub fn screen_mode(&self) -> ScreenMode {
        self.screen_mode
    }

    /// A read-only view of the graphics canvas. The IDE shows it (issue #173).
    pub fn graphics(&self) -> &GraphicsSystem {
        &self.graphics
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

    /// Execute CIRCLE statement - draw a circle, or a filled circle
    fn execute_circle(
        &mut self,
        x: &Expression,
        y: &Expression,
        radius: &Expression,
        filled: bool,
    ) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;
        let radius_val = self.eval_integer(radius)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            if filled {
                self.graphics.draw_filled_circle(x_val, y_val, radius_val);
            } else {
                self.graphics.draw_circle(x_val, y_val, radius_val);
            }
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

    /// Execute ELLIPSE statement - draw an ellipse, or a filled ellipse
    fn execute_ellipse(
        &mut self,
        x: &Expression,
        y: &Expression,
        major: &Expression,
        minor: &Expression,
        filled: bool,
    ) -> Result<()> {
        let x_val = self.eval_integer(x)?;
        let y_val = self.eval_integer(y)?;
        let major_val = self.eval_integer(major)?;
        let minor_val = self.eval_integer(minor)?;

        // A text-only screen mode has no graphics
        if self.screen_mode.has_graphics {
            if filled {
                self.graphics
                    .draw_filled_ellipse(x_val, y_val, major_val, minor_val);
            } else {
                self.graphics
                    .draw_ellipse(x_val, y_val, major_val, minor_val);
            }
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

        // The far corner becomes the last point, so it saturates at the i32
        // limits (the rule of issue 137 in src/graphics/mod.rs)
        let x2_val = x1_val.saturating_add(width_val);
        let y2_val = y1_val.saturating_add(height_val);

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

        // "FILL x,y is equivalent to PLOT 133,x,y" (BBC BASIC for Windows
        // manual), so FILL also moves the last point. A text-only screen
        // mode has no graphics, as for PLOT
        if self.screen_mode.has_graphics {
            self.graphics.plot(133, x_val, y_val);
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

    /// The body of `eval_integer`, inside one evaluation level
    fn eval_integer_level(&mut self, expr: &Expression) -> Result<i32> {
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
            Expression::Member(path) => self.member_integer(path),
            Expression::Variable(name) if self.is_new_list_call(name) => self.new_collection(name),
            Expression::ArrayAccess { name, indices } if self.is_list_call(name) => {
                real_to_integer(self.eval_list_function(name, indices)?)
            }
            Expression::ArrayAccess { name, indices } if self.is_matrix_call(name) => {
                real_to_integer(self.eval_matrix_function(name, indices)?)
            }
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

                // A power always gives a real value, which converts to an
                // integer or gives "Too big" (issue #175).
                if let BinaryOperator::Power = op {
                    return real_to_integer(self.eval_real(expr)?);
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
                    // NOT gives the bit complement of the 32-bit value (issue #158).
                    UnaryOperator::Not => Ok(!val),
                    UnaryOperator::Indirect(_) => unreachable!("memory read returned above"),
                }
            }
            // A string function in a numeric context, such as A% = CHR$(65)
            Expression::FunctionCall { .. } if self.is_string_expr(expr) => {
                Err(BBCBasicError::TypeMismatch)
            }
            Expression::FunctionCall { name, args } => self.eval_function_int(name, args),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The body of `eval_real`, inside one evaluation level
    fn eval_real_level(&mut self, expr: &Expression) -> Result<f64> {
        match expr {
            Expression::Integer(val) => Ok(*val as f64),
            Expression::Real(val) => Ok(*val),
            Expression::Member(path) => self.member_real(path),
            Expression::Variable(name) if self.is_new_list_call(name) => {
                self.new_collection(name).map(f64::from)
            }
            Expression::ArrayAccess { name, indices } if self.is_list_call(name) => {
                self.eval_list_function(name, indices)
            }
            Expression::ArrayAccess { name, indices } if self.is_matrix_call(name) => {
                self.eval_matrix_function(name, indices)
            }
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

                let value = match op {
                    BinaryOperator::Add => left_val + right_val,
                    BinaryOperator::Subtract => left_val - right_val,
                    BinaryOperator::Multiply => left_val * right_val,
                    BinaryOperator::Divide => {
                        if right_val == 0.0 {
                            return Err(BBCBasicError::DivisionByZero);
                        }
                        left_val / right_val
                    }
                    // A negative base to a power with a fraction has no real result.
                    BinaryOperator::Power if left_val < 0.0 && right_val.fract() != 0.0 => {
                        return Err(BBCBasicError::LogRange);
                    }
                    // Zero to a negative whole power is 1 / 0.
                    BinaryOperator::Power
                        if left_val == 0.0 && right_val < 0.0 && right_val.fract() == 0.0 =>
                    {
                        return Err(BBCBasicError::DivisionByZero);
                    }
                    BinaryOperator::Power => left_val.powf(right_val),
                    _ => return Err(BBCBasicError::IllegalFunction),
                };
                // A result that is not finite is out of range.
                if value.is_finite() {
                    Ok(value)
                } else {
                    Err(BBCBasicError::TooBig)
                }
            }
            Expression::UnaryOp { op, operand } => {
                use crate::parser::UnaryOperator;
                if let UnaryOperator::Indirect(width) = op {
                    return self.read_memory(*width, operand).map(f64::from);
                }
                // NOT converts its operand to an integer, as AND does (issue #158).
                if let UnaryOperator::Not = op {
                    return self.eval_integer(expr).map(f64::from);
                }
                let val = self.eval_real(operand)?;
                match op {
                    UnaryOperator::Minus => Ok(-val),
                    UnaryOperator::Plus => Ok(val),
                    UnaryOperator::Not | UnaryOperator::Indirect(_) => {
                        unreachable!("NOT and memory read returned above")
                    }
                }
            }
            // A string function in a numeric context, such as A = LEFT$("a", 1)
            Expression::FunctionCall { .. } if self.is_string_expr(expr) => {
                Err(BBCBasicError::TypeMismatch)
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

    /// Evaluate an expression one time and give its value as a literal
    /// expression. The CASE statement compares this value with each WHEN
    /// value, and does not evaluate its expression again.
    pub fn eval_to_literal(&mut self, expr: &Expression) -> Result<Expression> {
        if self.is_string_expr(expr) {
            Ok(Expression::String(self.eval_string(expr)?))
        } else if self.is_real_expr(expr) {
            Ok(Expression::Real(self.eval_real(expr)?))
        } else {
            Ok(Expression::Integer(self.eval_integer(expr)?))
        }
    }

    /// Tell if an expression can give a value with a fraction part.
    fn is_real_expr(&self, expr: &Expression) -> bool {
        use crate::parser::{BinaryOperator, UnaryOperator};
        match expr {
            Expression::Real(_) => true,
            Expression::Integer(_) | Expression::String(_) | Expression::Structure(_) => false,
            Expression::Variable(name) => {
                name == "PI"
                    || (!name.ends_with('%')
                        && !name.ends_with('$')
                        && self.variables.get_real_var(name).is_some())
            }
            Expression::ArrayAccess { name, .. } => !name.ends_with('%') && !name.ends_with('$'),
            Expression::Member(path) => !path.member().ends_with(['%', '$']),
            Expression::BinaryOp { op, left, right } => match op {
                // In BBC BASIC, `/` always gives a real value
                BinaryOperator::Divide => true,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply => {
                    self.is_real_expr(left) || self.is_real_expr(right)
                }
                // In BBC BASIC, `^` always gives a real value (issue #175)
                BinaryOperator::Power => true,
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
                    | "LN" | "LOG" | "DEG" | "RAD" | "PI" | "VAL" | "EVAL" => true,
                    // RND with no bracket gives an integer
                    "RND" => !args.is_empty(),
                    "ABS" => args.first().is_some_and(|arg| self.is_real_expr(arg)),
                    "SUM" | "MOD" => array_functions::gives_real(name, args),
                    _ => false,
                }
            }
        }
    }

    /// Tell if an expression gives a string value.
    ///
    /// EVAL is "not a string" here, because its text is a run-time value. A
    /// target or PRINT gives the type of an EVAL call. In an operation, the
    /// other operand gives the type: `EVAL(A$) + "x"` is a string, but
    /// `EVAL(A$) + EVAL(B$)` is a number (a known limit, issue 147).
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
            Expression::FunctionCall { name, args } if array_functions::is_array_function(name) => {
                array_functions::gives_string(name, args)
            }
            Expression::FunctionCall { name, .. } if !name.ends_with('$') => {
                // The body of a single-line function has a type with no run.
                // The type of a multi-line function result is known only after
                // its body runs, so it is "not a string" here (a known limit).
                match self.functions.get_key_value(name) {
                    Some((key, func)) if !open.contains(&key.as_str()) => match &func.body {
                        FunctionBody::Expression(body, _) => {
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
            Expression::Member(path) => path.member().ends_with('$'),
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

    /// The body of `eval_string`, inside one evaluation level
    fn eval_string_level(&mut self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::String(val) => Ok(val.clone()),
            Expression::Member(path) => self.member_string(path),
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
            // A numeric variable or constant in a string context, such as
            // A$ = PI (issue 146). A variable with no value keeps its error.
            Expression::Variable(name) if !name.ends_with('$') => {
                self.eval_number(expr)?;
                Err(BBCBasicError::TypeMismatch)
            }
            Expression::Variable(name) => self
                .variables
                .get_string_var(name)
                .map(|s| s.to_string())
                .ok_or_else(|| BBCBasicError::NoSuchVariable(name.clone())),
            // LISTGET$(l, i) is an extension (issue #139). An array named
            // LISTGET$ keeps its old meaning.
            Expression::ArrayAccess { name, indices }
                if name == "LISTGET$" && !self.variables.has_array(name) =>
            {
                let [list, index] = indices.as_slice() else {
                    return Err(list_arguments_error(name, 2));
                };
                let list = self.eval_integer(list)?;
                let index = self.eval_integer(index)?;
                self.collections.get_text(list, index)
            }
            // TOJSON$(handle) is an extension (issue #142). An array named
            // TOJSON$ keeps its old meaning.
            Expression::ArrayAccess { name, indices }
                if name == "TOJSON$" && !self.variables.has_array(name) =>
            {
                let [handle] = indices.as_slice() else {
                    return Err(list_arguments_error(name, 1));
                };
                let handle = self.eval_integer(handle)?;
                let value = self
                    .collections
                    .to_json(handle, crate::variables::MAX_STRING_LENGTH)?;
                json_text_in_limit(crate::json::to_compact(&value))
            }
            // LISTKIND$(l, i) and MAPKIND$(m, key$) are extensions (issue
            // #142). An array with that name keeps its old meaning.
            Expression::ArrayAccess { name, indices }
                if matches!(name.as_str(), "LISTKIND$" | "MAPKIND$")
                    && !self.variables.has_array(name) =>
            {
                let [handle, at] = indices.as_slice() else {
                    return Err(list_arguments_error(name, 2));
                };
                let handle = self.eval_integer(handle)?;
                let kind = if name == "LISTKIND$" {
                    let index = self.eval_integer(at)?;
                    self.collections.list_kind(handle, index)?
                } else {
                    let key = self.eval_string(at)?;
                    self.collections.map_kind(handle, &key)?
                };
                Ok(kind.to_string())
            }
            // MAPGET$(m, key$) and MAPKEY$(m, n) are extensions (issue #140).
            // An array with that name keeps its old meaning.
            Expression::ArrayAccess { name, indices }
                if matches!(name.as_str(), "MAPGET$" | "MAPKEY$")
                    && !self.variables.has_array(name) =>
            {
                let [map, key] = indices.as_slice() else {
                    return Err(list_arguments_error(name, 2));
                };
                let map = self.eval_integer(map)?;
                if name == "MAPKEY$" {
                    let index = self.eval_integer(key)?;
                    self.collections.map_key(map, index)
                } else {
                    let key = self.eval_string(key)?;
                    self.collections.map_get_text(map, &key)
                }
            }
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
    /// Tokenize and parse the text argument of EVAL as one expression. The
    /// text is parsed each time, with no cache (issue 147).
    fn parse_eval_text(&mut self, args: &[Expression]) -> Result<Expression> {
        let [text] = args else {
            return Err(BBCBasicError::SyntaxError {
                message: "EVAL requires 1 argument".to_string(),
                line: None,
            });
        };
        let text = self.eval_string(text)?;
        let tokens = crate::tokenizer::tokenize_expression(&text)?;
        crate::parser::parse_expression(&tokens)
    }

    /// The body of `eval_function_int`, inside one evaluation level
    fn eval_function_int_level(&mut self, name: &str, args: &[Expression]) -> Result<i32> {
        // Check if this is a user-defined function first
        if self.functions.contains_key(name) {
            return self.call_function_int(name, args);
        }

        // Otherwise, it's a built-in function
        match name {
            "EVAL" => self.nested_call(|executor| {
                let parsed = executor.parse_eval_text(args)?;
                if executor.is_string_expr(&parsed) {
                    return Err(BBCBasicError::TypeMismatch);
                }
                executor.eval_integer(&parsed)
            }),
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
                // POINT(x, y) gives the colour number of the pixel, and -1
                // for a point off the screen (issues 161 and 176)
                if args.len() != 2 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "POINT requires 2 arguments (x, y)".to_string(),
                        line: None,
                    });
                }
                let x = self.eval_integer(&args[0])?;
                let y = self.eval_integer(&args[1])?;
                Ok(self.graphics.point(x, y).map_or(-1, i32::from))
            }
            // RND with no bracket: a random integer over the full 32-bit range
            "RND" if args.is_empty() => Ok(self.rng.borrow_mut().gen::<i32>()),
            // POS and VPOS give the column and the row of the text cursor of
            // the console, after a wrap at the last column also (issue #200).
            // With no console there is no screen model: POS gives the print
            // column, and VPOS gives 0. COUNT gives the characters printed
            // since the last new line: the print column.
            "POS" if args.is_empty() => Ok(self
                .console
                .0
                .as_mut()
                .and_then(|console| console.cursor())
                .map_or(self.column, |(column, _)| column)
                as i32),
            "COUNT" if args.is_empty() => Ok(self.column as i32),
            "VPOS" if args.is_empty() => Ok(self
                .console
                .0
                .as_mut()
                .and_then(|console| console.cursor())
                .map_or(0, |(_, row)| row as i32)),
            // ADVAL(0) gives the fire buttons, ADVAL(n) with n > 0 gives an
            // analogue channel, and ADVAL(n) with n < 0 gives the state of a
            // buffer. This build has no analogue hardware, so each one gives 0.
            // ADVAL(-5) to ADVAL(-8) give the free bytes in the queues of sound
            // channels 0 to 3 (issue #208). BASIC 2 gives the low byte to
            // OSBYTE 128, which reads buffers 4 to 7 for X = &FB to &F8.
            "ADVAL" => {
                if args.len() != 1 {
                    return Err(BBCBasicError::SyntaxError {
                        message: "ADVAL requires 1 argument".to_string(),
                        line: None,
                    });
                }
                let buffer = self.eval_integer(&args[0])? as u8;
                Ok(match buffer {
                    0xF8..=0xFB => self.sound_free_space(0xFB - buffer),
                    _ => 0,
                })
            }
            // GET waits for one key and gives its code. At the end of input it gives -1.
            "GET" if args.is_empty() => Ok(self.next_key()?.map_or(-1, |key| key as i32)),
            // INKEY(n) waits for a maximum of n centiseconds for a key and
            // gives its code. If no key comes, it gives -1.
            // A negative n (a key scan) gives 0: see inkey_limit.
            "INKEY" if args.len() == 1 => {
                let time = self.eval_integer(&args[0])?;
                Ok(match inkey_limit(time) {
                    Some(limit) => self.next_key_within(limit)?.map_or(-1, |key| key as i32),
                    None => 0,
                })
            }
            // Real-only functions should not be called as integers
            "SIN" | "COS" | "TAN" | "ATN" | "SQR" | "SQRT" | "ACS" | "ASN" | "EXP" | "LN"
            | "LOG" | "DEG" | "RAD" | "PI" | "RND" => Err(BBCBasicError::TypeMismatch),
            "SUM" | "SUMLEN" | "MOD" | "DIM" => {
                real_to_integer(self.array_function_number(name, args)?)
            }
            _ if name.starts_with("FN") => Err(BBCBasicError::NoSuchFnProc(name.to_string())),
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown function: {}", name),
                line: None,
            }),
        }
    }

    /// The body of `eval_function_real`, inside one evaluation level
    fn eval_function_real_level(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        // Check if this is a user-defined function first
        if self.functions.contains_key(name) {
            return self.call_function_real(name, args);
        }

        // Otherwise, it's a built-in function
        match name {
            "EVAL" => self.nested_call(|executor| {
                let parsed = executor.parse_eval_text(args)?;
                if executor.is_string_expr(&parsed) {
                    return Err(BBCBasicError::TypeMismatch);
                }
                executor.eval_real(&parsed)
            }),
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
                    return Err(BBCBasicError::NegativeRoot);
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
                let result = val.exp();
                // A result that is too large for a real number is Exp range.
                if result.is_infinite() {
                    return Err(BBCBasicError::ExpRange);
                }
                Ok(result)
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
                    return Err(BBCBasicError::LogRange);
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
                    return Err(BBCBasicError::LogRange);
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
                    return Err(BBCBasicError::NegativeRoot);
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
                    return Err(BBCBasicError::NegativeRoot);
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
                    return Err(BBCBasicError::NegativeRoot);
                }
                Ok(val.asin())
            }
            "SUM" | "SUMLEN" | "MOD" | "DIM" => self.array_function_number(name, args),
            _ if name.starts_with("FN") => Err(BBCBasicError::NoSuchFnProc(name.to_string())),
            _ => Err(BBCBasicError::SyntaxError {
                message: format!("Unknown function: {}", name),
                line: None,
            }),
        }
    }

    /// The body of `eval_function_string`, inside one evaluation level
    fn eval_function_string_level(&mut self, name: &str, args: &[Expression]) -> Result<String> {
        // Check if this is a user-defined function first
        if self.functions.contains_key(name) {
            return self.call_function_string(name, args);
        }

        // Otherwise, it's a built-in function
        match name {
            "EVAL" => self.nested_call(|executor| {
                let parsed = executor.parse_eval_text(args)?;
                if !executor.is_string_expr(&parsed) {
                    return Err(BBCBasicError::TypeMismatch);
                }
                executor.eval_string(&parsed)
            }),
            // GET$ waits for one key and gives it. At the end of input it gives "".
            "GET$" if args.is_empty() => Ok(self.next_key()?.map(String::from).unwrap_or_default()),
            // INKEY$(n) waits for a maximum of n centiseconds for a key and
            // gives it. If no key comes, or n is negative, it gives "".
            "INKEY$" if args.len() == 1 => {
                let time = self.eval_integer(&args[0])?;
                let key = match inkey_limit(time) {
                    Some(limit) => self.next_key_within(limit)?,
                    None => None,
                };
                Ok(key.map(String::from).unwrap_or_default())
            }
            "SUM" | "SUMLEN" | "MOD" | "DIM" => self.array_function_text(name, args),
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
                // Byte 4 of @% selects the format (issue 166). Read @% after
                // the argument, as the ROM does.
                // Check if the expression is explicitly a Real or contains decimal point
                match &args[0] {
                    Expression::Real(val) => Ok(self.str_format().format_real(*val)),
                    Expression::Integer(val) => Ok(self.str_format().format_integer(*val)),
                    // Select the evaluator one time, as PRINT does. A second
                    // try after an error runs the argument again, and a
                    // recursive function then doubles its calls at each
                    // level (issue 151).
                    // A real value gives the PRINT text (issue 181).
                    argument if self.is_real_expr(argument) => {
                        let value = self.eval_real(argument)?;
                        Ok(self.str_format().format_real(value))
                    }
                    argument => {
                        let value = self.eval_integer(argument)?;
                        Ok(self.str_format().format_integer(value))
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

    /// Tell if `name` with no bracket is a call of NEWLIST (issue #139) or
    /// NEWMAP (issue #140). A variable with that name has priority, so that
    /// old programs do not change.
    fn is_new_list_call(&self, name: &str) -> bool {
        matches!(name, "NEWLIST" | "NEWMAP") && !self.variables.has_variable(name)
    }

    /// Make a new list or a new map, for NEWLIST or NEWMAP with no bracket.
    fn new_collection(&mut self, name: &str) -> Result<i32> {
        if name == "NEWMAP" {
            self.collections.new_map()
        } else {
            self.collections.new_list()
        }
    }

    /// Tell if `name(...)` is a call of a numeric list function (issue #139),
    /// a numeric map function (issue #140) or a JSON conversion function
    /// (issue #142). An array of the same name has priority.
    fn is_list_call(&self, name: &str) -> bool {
        matches!(
            name,
            "NEWLIST"
                | "LISTLEN"
                | "LISTGET"
                | "LISTISSTR"
                | "NEWMAP"
                | "MAPLEN"
                | "MAPGET"
                | "MAPHAS"
                | "MAPISSTR"
                | "JSONTOLIST"
                | "JSONTOMAP"
        ) && !self.variables.has_array(name)
    }

    /// Evaluate a numeric list function: NEWLIST(), LISTLEN(l), LISTGET(l, i)
    /// or LISTISSTR(l, i). Or evaluate a numeric map function: NEWMAP(),
    /// MAPLEN(m), MAPGET(m, key$), MAPHAS(m, key$) or MAPISSTR(m, key$).
    /// LISTISSTR, MAPHAS and MAPISSTR give TRUE (-1) or FALSE (0).
    /// JSONTOLIST(json$) and JSONTOMAP(json$) give the handle of a new list
    /// or map.
    fn eval_list_function(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        let truth = |value: bool| if value { -1.0 } else { 0.0 };
        match (name, args) {
            ("NEWLIST", []) => self.collections.new_list().map(f64::from),
            ("NEWMAP", []) => self.collections.new_map().map(f64::from),
            ("JSONTOLIST" | "JSONTOMAP", [json]) => {
                let value = crate::json::parse(&self.eval_string(json)?)?;
                let handle = if name == "JSONTOLIST" {
                    self.collections.list_from_json(value)?
                } else {
                    self.collections.map_from_json(value)?
                };
                Ok(f64::from(handle))
            }
            ("JSONTOLIST" | "JSONTOMAP", _) => Err(list_arguments_error(name, 1)),
            ("MAPLEN", [map]) => {
                let map = self.eval_integer(map)?;
                Ok(self.collections.map_length(map)? as f64)
            }
            ("MAPGET" | "MAPHAS" | "MAPISSTR", [map, key]) => {
                let map = self.eval_integer(map)?;
                let key = self.eval_string(key)?;
                match name {
                    "MAPGET" => self.collections.map_get_number(map, &key),
                    "MAPHAS" => self.collections.has(map, &key).map(truth),
                    _ => self.collections.map_is_text(map, &key).map(truth),
                }
            }
            ("NEWMAP", _) => Err(list_arguments_error(name, 0)),
            ("MAPLEN", _) => Err(list_arguments_error(name, 1)),
            ("LISTLEN", [list]) => {
                let list = self.eval_integer(list)?;
                Ok(self.collections.length(list)? as f64)
            }
            ("LISTGET" | "LISTISSTR", [list, index]) => {
                let list = self.eval_integer(list)?;
                let index = self.eval_integer(index)?;
                if name == "LISTGET" {
                    self.collections.get_number(list, index)
                } else if self.collections.is_text(list, index)? {
                    Ok(-1.0)
                } else {
                    Ok(0.0)
                }
            }
            ("NEWLIST", _) => Err(list_arguments_error(name, 0)),
            ("LISTLEN", _) => Err(list_arguments_error(name, 1)),
            _ => Err(list_arguments_error(name, 2)),
        }
    }

    /// Evaluate the value of LISTADD, LISTSET or LISTINSERT: a string or a number.
    fn eval_list_element(&mut self, value: &Expression) -> Result<Element> {
        if self.is_string_expr(value) {
            Ok(Element::Text(self.eval_string(value)?))
        } else {
            Ok(Element::Number(self.eval_number(value)?))
        }
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
            // A built-in numeric function in a string context, such as
            // A$ = SIN(1) (issue 146). The numeric evaluators are the table of
            // these functions. As on a BBC Micro, the value is made first, so
            // an error in it comes before Type mismatch.
            _ if !name.ends_with('$') => {
                let call = Expression::FunctionCall {
                    name: name.to_string(),
                    args: args.to_vec(),
                };
                match self.eval_number(&call) {
                    Ok(_) => Err(BBCBasicError::TypeMismatch),
                    Err(BBCBasicError::SyntaxError { message, .. })
                        if message == format!("Unknown function: {}", name) =>
                    {
                        Err(unknown_string_function(name))
                    }
                    Err(error) => Err(error),
                }
            }
            _ => Err(unknown_string_function(name)),
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
            loop_state.position = position;
        }
    }

    /// Push a REPEAT position onto the repeat stack. If a GOTO starts the
    /// same REPEAT again in the current call, use its entry again.
    pub fn push_repeat(&mut self, position: Position) -> Result<()> {
        let floor = self.loop_floor().repeat_depth;
        if let Some(index) = Self::loop_entry(&self.repeat_stack, floor, position) {
            self.repeat_stack.truncate(index + 1);
            return Ok(());
        }
        check_room(self.repeat_stack.len())?;
        self.repeat_stack.push(position);
        Ok(())
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

        // A GOTO can start the same WHILE again in the current call: use its
        // entry again, or close it when the condition is false
        let floor = self.loop_floor().while_depth;
        let entry = Self::loop_entry(&self.while_stack, floor, position);

        if result != 0 {
            // Condition is true - enter loop body
            match entry {
                Some(index) => self.while_stack.truncate(index + 1),
                None => {
                    check_room(self.while_stack.len())?;
                    self.while_stack.push(position);
                }
            }
            Ok(Some(position))
        } else {
            // Condition is false - skip loop body
            if let Some(index) = entry {
                self.while_stack.truncate(index);
            }
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
    pub fn push_gosub_return(&mut self, position: Position) -> Result<()> {
        check_room(self.return_stack.len())?;
        self.return_stack.push((ReturnKind::Gosub, position));
        Ok(())
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

    /// The number of entries on the return stack (GOSUBs and PROC calls)
    pub fn return_depth(&self) -> usize {
        self.return_stack.len()
    }

    /// Push the position of a PROC call onto the return stack, with the
    /// loop depths of the caller
    pub fn push_proc_return(&mut self, position: Position) -> Result<()> {
        check_room(self.return_stack.len())?;
        let depths = self.loop_depths();
        self.return_stack.push((ReturnKind::Proc(depths), position));
        Ok(())
    }

    /// Pop the newest PROC call, and the GOSUBs and loops that this PROC left
    /// open. With no PROC call on the stack, give an error and change nothing.
    pub fn pop_proc_return(&mut self) -> Result<Position> {
        let index = self
            .return_stack
            .iter()
            .rposition(|&(kind, _)| matches!(kind, ReturnKind::Proc(_)))
            .ok_or(BBCBasicError::BadCall)?;
        let (kind, position) = self.return_stack[index];
        self.return_stack.truncate(index);
        if let ReturnKind::Proc(depths) = kind {
            self.close_loops(depths);
        }
        Ok(position)
    }

    /// The depths of the FOR, REPEAT and WHILE stacks now
    fn loop_depths(&self) -> LoopDepths {
        LoopDepths {
            for_depth: self.for_loops.len(),
            repeat_depth: self.repeat_stack.len(),
            while_depth: self.while_stack.len(),
        }
    }

    /// The loop depths at the start of the current PROC or FN call. A loop
    /// below these depths belongs to a caller (issue 134).
    fn loop_floor(&self) -> LoopDepths {
        let proc_floor = self
            .return_stack
            .iter()
            .rev()
            .find_map(|&(kind, _)| match kind {
                ReturnKind::Proc(depths) => Some(depths),
                ReturnKind::Gosub => None,
            })
            .unwrap_or_default();
        let function_floor = self.function_loop_floor;
        LoopDepths {
            for_depth: proc_floor.for_depth.max(function_floor.for_depth),
            repeat_depth: proc_floor.repeat_depth.max(function_floor.repeat_depth),
            while_depth: proc_floor.while_depth.max(function_floor.while_depth),
        }
    }

    /// Make room for a new FOR loop of `target`. As in BBC BASIC, an open
    /// loop of the same target in the current call closes, with each loop
    /// above it. Otherwise give No room at the stack limit.
    fn open_for_loop(&mut self, target: &ForTarget) -> Result<()> {
        let floor = self.loop_floor().for_depth.min(self.for_loops.len());
        if let Some(index) = self.for_loops[floor..]
            .iter()
            .rposition(|open| open.target == *target)
        {
            self.for_loops.truncate(floor + index);
        }
        check_room(self.for_loops.len())
    }

    /// The index of `position` on a loop stack, above the floor of the
    /// current call
    fn loop_entry(stack: &[Position], floor: usize, position: Position) -> Option<usize> {
        let floor = floor.min(stack.len());
        stack[floor..]
            .iter()
            .rposition(|&entry| entry == position)
            .map(|index| floor + index)
    }

    /// The number of entries on the FOR, REPEAT and WHILE stacks, in that
    /// order (for tests)
    pub fn loop_stack_depths(&self) -> (usize, usize, usize) {
        let depths = self.loop_depths();
        (depths.for_depth, depths.repeat_depth, depths.while_depth)
    }

    /// Close the FOR, REPEAT and WHILE loops above the depths
    fn close_loops(&mut self, depths: LoopDepths) {
        self.for_loops.truncate(depths.for_depth);
        self.repeat_stack.truncate(depths.repeat_depth);
        self.while_stack.truncate(depths.while_depth);
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
    pub fn enter_local_scope(&mut self) -> Result<()> {
        check_room(self.local_stack.len())?;
        self.local_stack.push(LocalFrame::new());
        Ok(())
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
        if let Some(structure) = name.strip_suffix("{}") {
            self.declare_local_structure(structure);
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
        self.restore_structures(frame.saved_structures, frame.local_structures);

        // The DATA pointer of the first LOCAL DATA of the routine (issue #186)
        if let Some(&pointer) = frame.saved_data.first() {
            (self.data_set, self.data_pointer) = pointer;
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
        self.function_loop_floor = LoopDepths::default();
        self.next_loop_line = None;
    }

    /// Set the number of files that a program can open at one time. A test
    /// lowers it, because a test cannot open 255 real files.
    pub fn set_open_file_limit(&mut self, limit: usize) {
        self.open_file_limit = limit;
    }

    /// Set error handler (ON ERROR GOTO line)
    pub fn set_error_handler(&mut self, line_number: u16) {
        self.set_error_handler_line_key(line_number.into());
    }

    /// Set the error handler to the line with this line key (ON ERROR GOTO
    /// in a line set of the run loop)
    pub fn set_error_handler_line_key(&mut self, line_key: u32) {
        self.error_handler = Some(ErrorHandler::Line(line_key));
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

    /// The libraries of INSTALL. CLEAR, RUN and CHAIN keep them.
    pub fn libraries(&self) -> &LibraryStore {
        &self.libraries
    }

    /// The libraries of INSTALL, to add a library
    pub fn libraries_mut(&mut self) -> &mut LibraryStore {
        &mut self.libraries
    }

    /// Remove the libraries and each procedure and function (NEW and LOAD).
    /// A routine whose DEF is in a removed library must not stay, and a
    /// single-line function of the old program must not stay.
    pub fn clear_libraries_and_routines(&mut self) {
        self.libraries.clear();
        self.library_data.clear();
        self.clear_procedures();
        self.clear_functions();
    }

    /// The definition of a function, if one has this name
    pub fn get_function(&self, name: &str) -> Option<&FunctionDefinition> {
        self.functions.get(name)
    }

    /// The value of a string expression: the file name of INSTALL
    pub fn eval_file_name(&mut self, expression: &Expression) -> Result<String> {
        self.eval_string(expression)
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
        // The run collected this definition with its DEF line before the
        // start: keep that line
        let line = match self.functions.get(name) {
            Some(FunctionDefinition {
                params: old_params,
                body: FunctionBody::Expression(old_expression, line),
            }) if old_params.as_slice() == params && old_expression == expression => *line,
            _ => None,
        };
        self.define_function(
            name.to_string(),
            params.to_vec(),
            FunctionBody::Expression(expression.clone(), line),
        );
        Ok(())
    }

    /// Take the DEF line of the innermost single-line function whose body
    /// failed, if the last call failed in the body of one.
    pub fn take_function_error_line(&mut self) -> Option<u32> {
        self.function_error_line.take()
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
    ) -> Result<Vec<Argument>> {
        let mut values = Vec::with_capacity(args.len());
        for (param_name, arg_expr) in params.iter().zip(args.iter()) {
            let value = if param_name.ends_with("{}") {
                Argument::Structure(self.structure_argument(arg_expr)?)
            } else if matches!(arg_expr, Expression::Structure(_)) {
                return Err(BBCBasicError::BadStructure);
            } else if param_name.ends_with('%') {
                Argument::Value(Variable::Integer(self.eval_integer(arg_expr)?))
            } else if param_name.ends_with('$') {
                Argument::Value(Variable::String(self.eval_string(arg_expr)?))
            } else {
                Argument::Value(Variable::Real(self.eval_real(arg_expr)?))
            };
            values.push(value);
        }
        Ok(values)
    }

    /// Declare each parameter as local in the current scope, then assign its
    /// argument value. ENDPROC or ENDFN restores the caller variable.
    pub fn bind_parameters(&mut self, params: &[String], values: Vec<Argument>) -> Result<()> {
        for (param_name, value) in params.iter().zip(values) {
            self.declare_local(param_name)?;
            match value {
                Argument::Structure(id) => self.bind_structure_parameter(param_name, id),
                Argument::Value(Variable::Integer(v)) => {
                    self.variables.set_integer_var(param_name.clone(), v)
                }
                Argument::Value(Variable::String(v)) => {
                    self.variables.set_string_var(param_name.clone(), v)?
                }
                Argument::Value(Variable::Real(v)) => {
                    self.variables.set_real_var(param_name.clone(), v)
                }
                Argument::Value(_) => return Err(BBCBasicError::TypeMismatch),
            }
        }
        Ok(())
    }

    /// Run `call` as one more level of evaluation: an evaluator, a FN call
    /// or an EVAL call. At the depth limit, give No room and do not run it.
    /// The depth goes back down on each exit, also after an error.
    fn nested_call<T>(&mut self, call: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.evaluation_depth >= EVALUATION_DEPTH_LIMIT {
            return Err(BBCBasicError::NoRoom);
        }
        self.evaluation_depth += 1;
        let result = call(self);
        self.evaluation_depth -= 1;
        result
    }

    /// Evaluate an expression to an integer value
    pub fn eval_integer(&mut self, expr: &Expression) -> Result<i32> {
        self.nested_call(|executor| executor.eval_integer_level(expr))
    }

    /// Evaluate an expression to a real value
    fn eval_real(&mut self, expr: &Expression) -> Result<f64> {
        self.nested_call(|executor| executor.eval_real_level(expr))
    }

    /// Evaluate an expression to a string value
    fn eval_string(&mut self, expr: &Expression) -> Result<String> {
        self.nested_call(|executor| executor.eval_string_level(expr))
    }

    /// Evaluate a function call returning an integer
    fn eval_function_int(&mut self, name: &str, args: &[Expression]) -> Result<i32> {
        self.nested_call(|executor| executor.eval_function_int_level(name, args))
    }

    /// Evaluate a function call returning a real number
    fn eval_function_real(&mut self, name: &str, args: &[Expression]) -> Result<f64> {
        self.nested_call(|executor| executor.eval_function_real_level(name, args))
    }

    /// Evaluate a function call returning a string
    fn eval_function_string(&mut self, name: &str, args: &[Expression]) -> Result<String> {
        self.nested_call(|executor| executor.eval_function_string_level(name, args))
    }

    /// Call a user-defined function and evaluate its result with `eval`.
    fn call_function<T>(
        &mut self,
        name: &str,
        args: &[Expression],
        eval: fn(&mut Self, &Expression) -> Result<T>,
    ) -> Result<T> {
        self.nested_call(|executor| executor.call_function_body(name, args, eval))
    }

    /// The body of `call_function`, inside the depth count
    fn call_function_body<T>(
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
        let loop_depths = self.loop_depths();

        // Enter local scope for function
        self.enter_local_scope()?;
        let caller_floor = std::mem::replace(&mut self.function_loop_floor, loop_depths);
        let result = self.run_function(&func, values, eval);
        self.function_loop_floor = caller_floor;

        // Exit local scope (restore variables), also after an error in the
        // function. Scopes, returns and loops that the body did not close go
        // too (for example a `=` line inside a FOR loop).
        while self.local_stack.len() > local_depth {
            self.exit_local_scope()?;
        }
        self.return_stack.truncate(return_depth);
        self.close_loops(loop_depths);

        result
    }

    /// Bind the parameters, run the function body and evaluate the result.
    fn run_function<T>(
        &mut self,
        func: &FunctionDefinition,
        values: Vec<Argument>,
        eval: fn(&mut Self, &Expression) -> Result<T>,
    ) -> Result<T> {
        self.bind_parameters(&func.params, values)?;

        match &func.body {
            FunctionBody::Expression(expression, def_line) => {
                let result = eval(self, expression);
                match (&result, def_line) {
                    // The innermost failed DEF line stays
                    (Err(_), Some(line)) => {
                        self.function_error_line.get_or_insert(*line);
                    }
                    (Err(_), None) => {}
                    // The caller ignored the errors of earlier calls (for
                    // example a PRINT that tries each type). Forget their line.
                    (Ok(_), _) => self.function_error_line = None,
                }
                result
            }
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

        // Each item is one record: its text and a line feed (issue #157). The
        // separators `,` and `;` have no effect on the file. A real value gets
        // all its digits, so that INPUT# reads the same value. A record cannot
        // hold a line feed or a carriage return, so a string with one of them
        // gives "Bad record" and nothing is written. With no item, PRINT#
        // writes one empty record.
        use crate::parser::PrintItem;
        self.exact_reals = true;
        let records = self.format_file_records(items);
        self.exact_reals = false;
        let records = records?;
        if records.iter().any(|r| r.contains(['\n', '\r'])) {
            return Err(BBCBasicError::BadRecord);
        }
        let mut output = String::new();
        for record in &records {
            output.push_str(record);
            output.push('\n');
        }
        if !items.iter().any(|i| matches!(i, PrintItem::Expression(_))) {
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

    /// Give the text of each expression item of a PRINT# statement.
    fn format_file_records(&mut self, items: &[crate::parser::PrintItem]) -> Result<Vec<String>> {
        use crate::parser::PrintItem;
        let mut records = Vec::new();
        // After ~, numeric items are hex until the next comma or semicolon
        let mut hex = false;
        for item in items {
            match item {
                PrintItem::Expression(expr) => records.push(self.format_print_item(expr, hex)?.0),
                // The parser does not permit TAB, SPC and ' in PRINT#
                PrintItem::Tab(_)
                | PrintItem::TabXY(..)
                | PrintItem::Spc(_)
                | PrintItem::NewLine => {}
                PrintItem::Hex => hex = true,
                PrintItem::Semicolon | PrintItem::Comma => hex = false,
            }
        }
        Ok(records)
    }

    /// Execute INPUT# statement - read from file
    fn execute_input_file(
        &mut self,
        handle_expr: &Expression,
        targets: &[InputItem],
    ) -> Result<()> {
        // Evaluate the handle
        let handle = self.eval_integer(handle_expr)?;

        // Each target reads one record: one line of the file (issue #157)
        for target in targets {
            let var_name = match target {
                InputItem::Variable(var) => var,
                InputItem::ArrayElement { name, .. } => name,
                InputItem::Member(path) => path.member(),
                // The parser gives no prompt text and no ' in INPUT#
                InputItem::Prompt { .. } | InputItem::NewLine => continue,
            };
            let member_indices = self.check_member_target(target)?;
            let record = self.read_file_record(handle)?;

            // The value type comes from the name of the target. A string gets
            // the full record. A number that is not valid gives 0.
            let value = if var_name.ends_with('%') {
                Variable::Integer(record.trim().parse::<i32>().unwrap_or(0))
            } else if var_name.ends_with('$') {
                Variable::String(record)
            } else {
                Variable::Real(record.trim().parse::<f64>().unwrap_or(0.0))
            };
            // The indices of an element are evaluated now (issue #154)
            self.store_target(target, &member_indices, value)?;
        }

        Ok(())
    }

    /// Read one record of an INPUT# file: one line, with no line end. A last
    /// line with no line feed is one record. If no line is left, give "EOF".
    fn read_file_record(&mut self, handle: i32) -> Result<String> {
        let file_handle = self
            .open_files
            .get_mut(&handle)
            .ok_or(BBCBasicError::ChannelNotOpen(handle))?;

        // Only input files can be read from
        let reader = match file_handle {
            FileHandle::Input(reader) | FileHandle::Update(reader) => reader,
            FileHandle::Output(_) => return Err(BBCBasicError::BadCall),
        };

        let mut line = String::new();
        let count = reader
            .read_line(&mut line)
            .map_err(|e| BBCBasicError::DiskError(format!("Read error: {}", e)))?;
        if count == 0 {
            return Err(BBCBasicError::Eof);
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        Ok(line)
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
        assert_eq!(format_number(1e-7), "1E-7");
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
        assert_eq!(executor.get_output(), "        42\n");
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

        assert_eq!(executor.get_output(), "       100\n");
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
            variable: InputItem::Variable("I%".to_string()),
            start: Expression::Integer(1),
            end: Expression::Integer(10),
            step: None,
        };

        executor.execute_statement(&stmt).unwrap();

        // Loop variable should be set to start value
        assert_eq!(executor.get_variable_int("I%").unwrap(), 1);

        // Loop should be on the stack
        assert_eq!(executor.for_loops.len(), 1);
        assert_eq!(
            executor.for_loops[0].target,
            ForTarget::Variable("I%".to_string())
        );
        assert_eq!(
            executor.for_loops[0].kind,
            ForKind::Count {
                end: 10.0,
                step: 1.0
            }
        );
    }

    #[test]
    fn test_a_failed_for_each_next_keeps_the_inner_loops_and_stores_the_index() {
        // Issue #141 K4: as for a FOR loop, an error at NEXT does not close
        // the inner loops. The new index is stored, so the next NEXT goes on.
        let mut executor = Executor::new();
        for source in [
            "10 L = NEWLIST : LISTADD L, 1 : LISTADD L, \"A\"",
            "20 FOR EACH N IN L",
            "30 FOR I = 1 TO 2",
        ] {
            let tokens = crate::tokenizer::tokenize(source).unwrap();
            for statement in crate::parser::parse_line(&tokens).unwrap() {
                executor.execute_statement(&statement).unwrap();
            }
        }
        let next = Statement::Next {
            variables: vec![InputItem::Variable("N".to_string())],
        };

        assert_eq!(
            executor.execute_statement(&next),
            Err(BBCBasicError::TypeMismatch)
        );
        assert_eq!(executor.for_loops.len(), 2);
        assert_eq!(
            executor.for_loops[0].kind,
            ForKind::Each {
                handle: 1,
                index: 1
            }
        );
    }

    #[test]
    fn test_for_loop_with_step() {
        // RED: Test FOR I% = 10 TO 1 STEP -1
        let mut executor = Executor::new();
        let stmt = Statement::For {
            variable: InputItem::Variable("I%".to_string()),
            start: Expression::Integer(10),
            end: Expression::Integer(1),
            step: Some(Expression::Integer(-1)),
        };

        executor.execute_statement(&stmt).unwrap();

        // Loop variable should be set to start value
        assert_eq!(executor.get_variable_int("I%").unwrap(), 10);

        // Loop should be on the stack with correct step
        assert_eq!(executor.for_loops.len(), 1);
        assert_eq!(
            executor.for_loops[0].kind,
            ForKind::Count {
                end: 1.0,
                step: -1.0
            }
        );
    }

    #[test]
    fn test_next_statement() {
        // RED: Test FOR...NEXT loop execution
        let mut executor = Executor::new();

        // FOR I% = 1 TO 3
        let for_stmt = Statement::For {
            variable: InputItem::Variable("I%".to_string()),
            start: Expression::Integer(1),
            end: Expression::Integer(3),
            step: None,
        };
        executor.execute_statement(&for_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 1);

        // NEXT I%
        let next_stmt = Statement::Next {
            variables: vec![InputItem::Variable("I%".to_string())],
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
            variables: vec![InputItem::Variable("I%".to_string())],
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
            variable: InputItem::Variable("I%".to_string()),
            start: Expression::Integer(5),
            end: Expression::Integer(1),
            step: Some(Expression::Integer(-1)),
        };
        executor.execute_statement(&for_stmt).unwrap();
        assert_eq!(executor.get_variable_int("I%").unwrap(), 5);

        let next_stmt = Statement::Next {
            variables: vec![InputItem::Variable("I%".to_string())],
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
            variables: vec![
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B%".to_string()),
                InputItem::Variable("C%".to_string()),
            ],
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
            variables: vec![
                InputItem::Variable("A$".to_string()),
                InputItem::Variable("B$".to_string()),
                InputItem::Variable("C$".to_string()),
            ],
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
            variables: vec![
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B".to_string()),
                InputItem::Variable("C$".to_string()),
            ],
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
            variables: vec![
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B%".to_string()),
            ],
        };
        executor.execute_statement(&read_stmt1).unwrap();

        assert_eq!(executor.get_variable_int("A%").unwrap(), 10);
        assert_eq!(executor.get_variable_int("B%").unwrap(), 20);

        // RESTORE
        let restore_stmt = Statement::Restore { line_number: None };
        executor.execute_statement(&restore_stmt).unwrap();

        // READ C%, D%
        let read_stmt2 = Statement::Read {
            variables: vec![
                InputItem::Variable("C%".to_string()),
                InputItem::Variable("D%".to_string()),
            ],
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
            variables: vec![
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B%".to_string()),
            ],
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
            variables: vec![
                InputItem::Variable("C%".to_string()),
                InputItem::Variable("D%".to_string()),
            ],
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
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B%".to_string()),
                InputItem::Variable("C%".to_string()),
                InputItem::Variable("D%".to_string()),
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
            variables: vec![
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B%".to_string()),
            ],
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
            variables: vec![
                InputItem::Variable("A%".to_string()),
                InputItem::Variable("B%".to_string()),
                InputItem::Variable("C%".to_string()),
            ],
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
        executor.push_repeat(Position::line_start(20)).unwrap();

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
        executor
            .push_gosub_return(Position::line_start(20))
            .unwrap();

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

        executor
            .push_gosub_return(Position::line_start(10))
            .unwrap();
        executor
            .push_gosub_return(Position::line_start(100))
            .unwrap();

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
        executor.enter_local_scope().unwrap();

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
    fn install_outside_the_run_loop_gives_an_error() {
        // The run loop executes INSTALL (issue #185). The executor alone
        // cannot add a line set, so it must not do nothing with no error.
        let mut executor = Executor::new();
        let stmt = Statement::Install {
            file: Expression::String("lib".to_string()),
        };

        assert!(executor.execute_statement(&stmt).is_err());
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
        // Each item is one record (issue #157)
        assert_eq!(content, "Hello\nWorld\n");

        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_input_file_reads_data() {
        // RED: Test INPUT# reads from file
        use std::fs;
        let test_file = "test_input.txt";

        // Create test file with data
        // One record on each line (issue #157)
        fs::write(test_file, "42\nHello\n3.14").unwrap();

        let mut executor = Executor::new();
        let handle = executor.open_file_for_reading(test_file).unwrap();

        // Read data into variables
        let handle_expr = Expression::Integer(handle);
        let variables: Vec<InputItem> = ["A%", "B$", "C"]
            .map(|name| InputItem::Variable(name.to_string()))
            .to_vec();

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
        let variables = vec![InputItem::Variable("LINE$".to_string())];
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
