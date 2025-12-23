# BBC BASIC Interpreter - Implementation Plan

## Reference Documentation

**BBC BASIC for Windows Manual:** https://www.bbcbasic.co.uk/bbcwin/manual/index.html

This comprehensive manual covers BBC BASIC syntax, functions, statements, and extensions. Use this as the authoritative reference when implementing features.

---

## Current Status (December 23, 2024)

### Session Summary
**Completed Features:**
1. ✅ LOCAL variables (2024-12-22)
2. ✅ DEF FN (User-defined functions) (2024-12-22)
3. ✅ ON GOTO/ON GOSUB (Computed branching) (2024-12-22)
4. ✅ MOD, DIV, ^ operators (2024-12-22)
5. ✅ Error handling (ON ERROR/ERL/ERR) (2024-12-22)
6. ✅ File I/O (OPENIN/OPENOUT/PRINT#/INPUT#/CLOSE#/EOF#) (2024-12-22)
7. ✅ WHILE...ENDWHILE loops (2024-12-22)
8. ✅ Extended File I/O (BGET#/BPUT#/PTR#/EXT#) (2024-12-23)

**Test Count:** 198 passing unit tests (was 169)
**Lines of Code:** ~8300 LOC (was 7900)

### Active Branches
- `main` - Stable baseline
- All feature branches merged

### Implementation Progress
- **Core Language:** ~98% complete (all arithmetic, control flow, procedures, functions, error handling, all loop types)
- **Console I/O:** 100% complete (PRINT, INPUT, CLS)
- **File Operations:** 100% complete (SAVE/LOAD/CHAIN/OPENIN/OPENOUT/PRINT#/INPUT#/CLOSE#/EOF#/BGET#/BPUT#/PTR#/EXT# done) ✅
- **Math Functions:** 80% complete (basic math + trig done)
- **String Functions:** 70% complete (core operations done)
- **Error Handling:** 75% complete (ON ERROR/ERL/ERR done, ERROR/REPORT pending)
- **Graphics:** 0% (stub only)
- **Sound:** 0% (stub only)

---

## Architecture Overview

### Current Codebase Structure (~7000 LOC)

```
src/
├── main.rs              - REPL loop, command handlers, program execution
├── lib.rs               - Public API exports
├── tokenizer/           - Lexical analysis (text → tokens)
├── parser/              - Syntax analysis (tokens → AST)
├── executor/            - Runtime execution engine
├── variables/           - Variable storage (integer, real, string, arrays)
├── program/             - Program line storage and control flow
├── memory/              - Heap memory management
├── filesystem/          - File I/O (stub)
├── graphics/            - Graphics operations (stub)
├── sound/               - Sound operations (stub)
└── os/                  - OS interface (stub)
```

### Core Architecture Pattern

**Three-Stage Pipeline:**
1. **Tokenizer** → Convert text to token stream (handles BBC BASIC keywords as bytes 0x80-0xFF)
2. **Parser** → Convert tokens to Statement/Expression AST
3. **Executor** → Execute statements, manage runtime state

**Control Flow Handling:**
- Most control flow (GOTO, GOSUB, FOR, REPEAT, PROC) handled in `main.rs` runtime loop
- Executor maintains stacks for:
  - `return_stack` - GOSUB/RETURN and PROC/ENDPROC calls
  - `for_loops` - FOR loop state (variable, end, step, line)
  - `repeat_stack` - REPEAT loop line numbers
- ProgramStore manages line navigation (goto_line, next_line)

---

## Key Design Patterns

### 1. Stack-Based Control Flow

**Used for:** GOSUB/RETURN, PROC/ENDPROC, FOR/NEXT, REPEAT/UNTIL

```rust
// Pattern: Push state on entry, pop on exit
struct Executor {
    return_stack: Vec<u16>,           // Line numbers
    for_loops: Vec<(String, i32, i32, u16)>,  // (var, end, step, line)
    repeat_stack: Vec<u16>,           // Line numbers
}

// Entry point
pub fn push_return(&mut self, line: u16) {
    self.return_stack.push(line);
}

// Exit point
pub fn pop_return(&mut self) -> Result<u16> {
    self.return_stack.pop().ok_or(Error::NoReturn)
}
```

### 2. Two-Pass Execution

**Used for:** DATA collection, PROC definition collection

```rust
// First pass: Collect metadata before execution
for (line_number, line) in program.list() {
    let statement = parse_statement(line)?;
    
    if matches!(statement, Statement::Data { .. }) {
        executor.collect_data(&statement)?;
    }
    
    if let Statement::DefProc { name, params } = statement {
        executor.define_procedure(name, line_number, params);
    }
}

// Second pass: Execute program
program.start_execution();
while let Some(line_number) = program.get_current_line() {
    // Execute statements...
}
```

### 3. REPL vs Program Statement Duality

**Pattern:** Some commands work differently in REPL vs running program

```rust
// REPL-only commands (immediate mode):
// - RUN, LIST, NEW, SAVE, LOAD, CHAIN, *CAT, HELP, EXIT

// Program statements (can be in numbered lines):
// - PRINT, FOR, IF, PROC calls, etc.

// Hybrid approach:
// - Check in REPL loop first
// - Fall through to statement execution if not a command
```

### 4. Expression Evaluation by Type

**Used for:** All expression evaluation

```rust
impl Executor {
    fn eval_integer(&self, expr: &Expression) -> Result<i32>;
    fn eval_real(&self, expr: &Expression) -> Result<f64>;
    fn eval_string(&self, expr: &Expression) -> Result<String>;
    
    // Assignment uses type suffix to determine target type
    fn execute_assignment(&mut self, target: &str, expr: &Expression) -> Result<()> {
        if target.ends_with('%') {
            let value = self.eval_integer(expr)?;
            self.variables.set_integer_var(target, value);
        } else if target.ends_with('$') {
            let value = self.eval_string(expr)?;
            self.variables.set_string_var(target, value)?;
        } else {
            let value = self.eval_real(expr)?;
            self.variables.set_real_var(target, value);
        }
    }
}
```

---

## Implementation Guides for Outstanding Features

### ✅ COMPLETED: LOCAL Variables

**Status:** Implemented 2024-12-22

**What it does:** Create local variable scope within PROC/FN

**Implementation Notes:**
- LocalFrame stack stores saved variable values
- Variables shadowed in local scope without modifying globals
- Automatically restored on ENDPROC/function return
- Works with PROC parameter binding

**Files Modified:**
- `src/executor/mod.rs` - Added LocalFrame, enter/exit_local_scope()
- `src/parser/mod.rs` - Added Statement::Local
- `src/main.rs` - Integrated with PROC calls

---

### ✅ COMPLETED: DEF FN (User Functions)

**Status:** Implemented 2024-12-22

**What it does:** Define functions that return values (unlike PROC which doesn't return)

**Implementation Notes:**
- Single-line syntax: `DEF FN add(X,Y) = X + Y`
- Function calls are expressions: `PRINT FN add(5, 3)`
- Parameters automatically local (leverages LOCAL infrastructure)
- Function definitions stored in HashMap<String, FunctionDefinition>

**Files Modified:**
- `src/executor/mod.rs` - Added FunctionDefinition, call_function_*()
- `src/parser/mod.rs` - Added Statement::DefFn with expression
- Changed eval_* methods from &self to &mut self

---

### ✅ COMPLETED: ON GOTO / ON GOSUB

**Status:** Implemented 2024-12-22

**What it does:** Computed jump based on expression value

```rust
// 1. Add to Executor
struct Executor {
    local_stack: Vec<LocalFrame>,
}

struct LocalFrame {
    variables: HashMap<String, VariableValue>,
    count: usize,  // How many locals in this frame
}

enum VariableValue {
    Integer(i32),
    Real(f64),
    String(String),
    Unset,  // Variable didn't exist before
}

// 2. Add Statement
enum Statement {
    Local { variables: Vec<String> },
    // ...
}

// 3. Parser: LOCAL X, Y$, Z%
fn parse_local(tokens: &[Token]) -> Result<Statement> {
    // Parse comma-separated variable names
    let variables = parse_variable_list(tokens)?;
    Ok(Statement::Local { variables })
}

// 4. Executor methods
impl Executor {
    pub fn enter_local_scope(&mut self) {
        self.local_stack.push(LocalFrame {
            variables: HashMap::new(),
            count: 0,
        });
    }
    
    pub fn declare_local(&mut self, name: String) -> Result<()> {
        let frame = self.local_stack.last_mut()
            .ok_or(Error::NoLocalScope)?;
        
        // Save current value (or Unset if doesn't exist)
        let current = self.variables.get(&name);
        frame.variables.insert(name.clone(), current);
        frame.count += 1;
        
        // Clear the variable in main scope (create new local)
        self.variables.unset(&name);
        Ok(())
    }
    
    pub fn exit_local_scope(&mut self) -> Result<()> {
        let frame = self.local_stack.pop()
            .ok_or(Error::NoLocalScope)?;
        
        // Restore all saved values
        for (name, value) in frame.variables {
            match value {
                VariableValue::Unset => self.variables.unset(&name),
                VariableValue::Integer(v) => self.variables.set_integer_var(name, v),
                VariableValue::Real(v) => self.variables.set_real_var(name, v),
                VariableValue::String(v) => self.variables.set_string_var(name, v)?,
            }
        }
        Ok(())
    }
}

// 5. Runtime integration in main.rs
// When entering PROC:
executor.enter_local_scope();

// When executing LOCAL statement:
for var in variables {
    executor.declare_local(var)?;
}

// When exiting ENDPROC:
executor.exit_local_scope()?;
```

**Testing strategy:**
```basic
10 X = 10: Y = 20
20 PROC test
30 PRINT X, Y     REM Should print: 10  20
100 DEF PROC test
110 LOCAL X
120 X = 99        REM Local X
130 Y = 99        REM Global Y
140 PRINT X, Y    REM Should print: 99  99
150 ENDPROC
```

---

### HIGH PRIORITY: DEF FN (User Functions)

**Complexity:** Medium | **Impact:** High | **Est. Time:** 3-4 hours

**What it does:** Define functions that return values (unlike PROC which doesn't return)

**Implementation:**

```rust
// 1. Add to Executor
struct FunctionDefinition {
    line_number: u16,
    params: Vec<String>,
    return_type: VarType,  // Determined by FN name suffix
}

struct Executor {
    functions: HashMap<String, FunctionDefinition>,
}

// 2. Add Statements
enum Statement {
    DefFn { name: String, params: Vec<String>, expression: Expression },
    // DEF FN is single-line: DEF FN add(X, Y) = X + Y
}

enum Expression {
    FunctionCall { name: String, args: Vec<Expression> },
    // FN calls are expressions: PRINT FN add(5, 3)
}

// 3. Parser: DEF FN name(params) = expression
fn parse_def_fn(tokens: &[Token]) -> Result<Statement> {
    // Parse: FN name(params) = expression
    let name = parse_identifier()?;
    let params = parse_parameter_list()?;
    expect_token(Token::Operator('='))?;
    let expression = parse_expression()?;
    Ok(Statement::DefFn { name, params, expression })
}

// 4. Executor - function calls are expressions!
impl Executor {
    fn eval_integer(&self, expr: &Expression) -> Result<i32> {
        match expr {
            Expression::FunctionCall { name, args } => {
                self.call_function_int(name, args)
            }
            // ... other cases
        }
    }
    
    fn call_function_int(&self, name: &str, args: &[Expression]) -> Result<i32> {
        let func = self.functions.get(name)
            .ok_or(Error::UndefinedFunction)?;
        
        // Evaluate arguments
        let arg_values: Vec<_> = args.iter()
            .map(|arg| self.eval_integer(arg))
            .collect::<Result<_>>()?;
        
        // Bind parameters (create temporary local scope)
        let saved_vars = self.save_variables(&func.params);
        for (param, value) in func.params.iter().zip(arg_values) {
            self.variables.set_integer_var(param.clone(), value);
        }
        
        // Evaluate function expression
        let result = self.eval_integer(&func.expression)?;
        
        // Restore variables
        self.restore_variables(saved_vars);
        
        Ok(result)
    }
}
```

**Key differences from PROC:**
- FN is **single-line definition**: `DEF FN add(X,Y) = X + Y`
- FN is called as **expression**: `PRINT FN add(5, 3)`
- PROC is **multi-line**: `DEF PROC name ... ENDPROC`
- PROC is called as **statement**: `PROC name(args)`

---

### MEDIUM PRIORITY: ON GOTO / ON GOSUB

**Complexity:** Low | **Impact:** Medium | **Est. Time:** 1-2 hours

**What it does:** Computed jump based on expression value

```basic
ON X% GOTO 100, 200, 300    REM If X%=1 goto 100, X%=2 goto 200, etc.
ON Y% GOSUB 1000, 2000       REM If Y%=1 gosub 1000, etc.
```

**Implementation Notes:**
- 1-based indexing (value of 1 jumps to first target)
- Out-of-range values fall through to next statement (no error)
- Expression evaluated before branching decision
- Made eval_integer() public to support expression evaluation in main.rs

**Files Modified:**
- `src/parser/mod.rs` - Added Statement::OnGoto and Statement::OnGosub
- `src/main.rs` - Added control flow handling for computed branching
- `src/executor/mod.rs` - Made eval_integer() public

---

### ✅ COMPLETED: MOD, DIV, and ^ Operators

**Status:** Implemented 2024-12-22

**What it does:** Complete arithmetic operator set with modulo, integer division, and power

**Implementation Notes:**
- DIV (0x81) and MOD (0x83) are keyword operators, not character operators
- ^ (caret) is a character operator (already tokenized)
- Added keyword operator parsing to precedence climbing algorithm
- Evaluation logic already existed, only parsing needed work
- Precedence: ^ (60) > */DIV/MOD (50) > +- (40)

**Files Modified:**
- `src/parser/mod.rs` - Added get_keyword_precedence(), keyword_to_binary_op()
- Updated parse_expr_precedence() to handle Token::Keyword operators
- `src/executor/mod.rs` - Added evaluation tests

---

## Next Priority Features

### ✅ COMPLETED: Error Handling (ON ERROR / ERL / ERR)

**Status:** Implemented 2024-12-22

**What it does:** Catch runtime errors and handle them gracefully

**Implementation Notes:**
- ErrorInfo structure tracks error number, line, and message
- ON ERROR GOTO sets error handler line number
- ON ERROR OFF clears error handler
- ERL returns line number where error occurred
- ERR returns error number (BBC BASIC error codes)
- Runtime loop catches errors and jumps to handler
- Automatic error code mapping (e.g., DivisionByZero → 18)

**Files Modified:**
- `src/executor/mod.rs` - Added ErrorInfo struct, error_handler/last_error fields, get/set methods
- `src/parser/mod.rs` - Added Statement::OnError and Statement::OnErrorOff
- `src/main.rs` - Integrated error handler into runtime loop with error code mapping
- Added 7 unit tests for error handling
- Added test_error_handling.bas integration test

---

### ✅ COMPLETED: File I/O (OPENIN/OPENOUT/PRINT#/INPUT#/CLOSE#/EOF#)

**Status:** Implemented 2024-12-22

**What it does:** Read and write data files with file handle management

**BBC BASIC Syntax:**
```basic
10 F% = OPENOUT("data.txt")   REM Open for writing
20 PRINT# F%, "Hello, World!" REM Write to file
30 PRINT# F%, 42, 3.14        REM Write numbers
40 CLOSE# F%                   REM Close file
50 F% = OPENIN("data.txt")    REM Open for reading
60 INPUT# F%, LINE$           REM Read from file
70 IF EOF(F%) THEN PRINT "End of file"
80 CLOSE# F%
```

**Implementation Notes:**
- File handles tracked in `HashMap<i32, FileHandle>` in Executor
- BufReader/BufWriter for efficient I/O operations
- OPENIN/OPENOUT/EOF are functions (return values)
- PRINT#/INPUT#/CLOSE# are statements
- Proper error handling for invalid handles, file not found, etc.
- Maximum 255 open files (BBC BASIC standard limit)
- Handle allocation starts at 1, increments automatically

**Files Modified:**
- `src/lib.rs` - Added `ChannelNotOpen`, `TooManyOpenFiles` error types
- `src/parser/mod.rs` - Added `PrintFile`, `InputFile`, `CloseFile` statements + parser support
- `src/executor/mod.rs` - Added complete file I/O implementation (~250 lines)
  - FileHandle enum (Input/Output with BufReader/BufWriter)
  - open_file_for_reading(), open_file_for_writing()
  - execute_print_file(), execute_input_file(), execute_close_file()
  - check_eof() function
- Added 13 new tests (8 executor unit tests + 5 parser tests)
- Created `test_file_io.bas` integration test (43 lines)

**Key Discovery:**
- Initially forgot parser support - would have been unusable!
- Parser modifications required to detect `#` after PRINT/INPUT/CLOSE keywords
- Added parse_print_file_statement(), parse_input_file_statement(), parse_close_file_statement()

---

### ✅ COMPLETED: WHILE...ENDWHILE Loops

**Status:** Implemented 2024-12-22

**What it does:** Loop while a condition is TRUE (alternative to REPEAT...UNTIL)

**BBC BASIC Syntax:**
```basic
10 X% = 0
20 WHILE X% < 5
30 PRINT "X% = "; X%
40 X% = X% + 1
50 ENDWHILE
```

**Behavior:**
- Condition evaluated at loop start (pre-condition loop)
- If TRUE (non-zero), enters loop body
- If FALSE (zero), skips to after ENDWHILE
- After ENDWHILE, re-evaluates condition and loops back if still TRUE
- Supports nested WHILE loops

**Implementation Notes:**
- WHILE is an **extended statement** token: `ExtendedKeyword(0xC8, 0x95)`
- ENDWHILE token: `ExtendedKeyword(0xC8, 0xA4)` (newly added)
- while_stack tracks line numbers of WHILE statements
- Condition stored in WHILE statement, retrieved at ENDWHILE
- Skip-to-ENDWHILE logic scans forward, tracking depth for nested loops

**Files Modified:**
- `src/tokenizer/mod.rs` - Added ENDWHILE token (0xC8, 0xA4) to EXTENDED_STATEMENTS
- `src/parser/mod.rs` - Added Statement::While and Statement::EndWhile variants
  - Added parse_while_statement() function
  - Added ExtendedKeyword handling in parse_statement()
- `src/executor/mod.rs` - Added WHILE loop management (~50 lines)
  - Added while_stack: Vec<u16> field
  - push_while() - Check condition and enter loop
  - check_endwhile() - Re-evaluate condition and loop back or exit
  - check_endwhile_get_while_line() - Helper to retrieve WHILE line number
- `src/main.rs` - Runtime loop integration (~50 lines)
  - is_while and is_endwhile detection
  - WHILE: evaluate condition, skip to ENDWHILE if false
  - ENDWHILE: retrieve WHILE condition, re-evaluate, loop back if true
- Added 3 unit tests (basic loop, false condition, nested loops)
- Created `test_while.bas` integration test

**Key Differences from REPEAT...UNTIL:**
- **WHILE:** Checks condition at START (may never execute)
- **REPEAT:** Checks condition at END (always executes at least once)
- **WHILE:** Continues while TRUE
- **REPEAT:** Continues until TRUE (exits when condition TRUE)

**Implementation Time:** ~2 hours actual (matching estimate)
**LOC Added:** ~300 lines
**Tests Added:** +3 unit tests (169 total)

---

### HIGH PRIORITY: File I/O (OPENIN/OPENOUT/PRINT#/INPUT#)

**Complexity:** Medium | **Impact:** High | **Est. Time:** 4-5 hours

**What it does:** Read/write data files

```basic
10 file% = OPENOUT "data.txt"
20 PRINT# file%, "Hello"
30 PRINT# file%, 42
40 CLOSE# file%
```

**Implementation:**

```rust
// 1. Add to Executor
struct Executor {
    file_handles: HashMap<i32, FileHandle>,
    next_handle: i32,
}

enum FileHandle {
    Input(std::fs::File),
    Output(std::fs::File),
    ReadWrite(std::fs::File),
}

// 2. Add Functions (returns channel number)
enum Expression {
    OpenIn(String),   // OPENIN("file") - returns handle
    OpenOut(String),  // OPENOUT("file")
    OpenUp(String),   // OPENUP("file")
    Eof(i32),         // EOF#channel
}

// 3. Add Statements
enum Statement {
    PrintFile { channel: Expression, items: Vec<PrintItem> },
    InputFile { channel: Expression, variables: Vec<String> },
    Close { channel: Expression },
}

// 4. Executor implementation
impl Executor {
    pub fn open_input(&mut self, path: &str) -> Result<i32> {
        let file = std::fs::File::open(path)?;
        let handle = self.next_handle;
        self.file_handles.insert(handle, FileHandle::Input(file));
        self.next_handle += 1;
        Ok(handle)
    }
    
    pub fn print_file(&mut self, channel: i32, data: &str) -> Result<()> {
        let handle = self.file_handles.get_mut(&channel)
            .ok_or(Error::InvalidChannel)?;
        
        match handle {
            FileHandle::Output(f) | FileHandle::ReadWrite(f) => {
                use std::io::Write;
                writeln!(f, "{}", data)?;
                Ok(())
            }
            _ => Err(Error::NotOpenForOutput)
        }
    }
}
```

---

### LOW PRIORITY: Graphics (PLOT/DRAW/MOVE)

**Complexity:** High | **Impact:** Low (legacy) | **Est. Time:** 10+ hours

**Recommendation:** Use a modern Rust graphics crate:
- **pixels** - Simple pixel buffer rendering
- **minifb** - Minimal cross-platform window
- **sdl2** - Full SDL2 bindings

**Minimal Implementation Pattern:**

```rust
// 1. In graphics/mod.rs
use minifb::{Window, WindowOptions};

pub struct GraphicsContext {
    window: Window,
    buffer: Vec<u32>,  // RGBA pixels
    width: usize,
    height: usize,
    cursor_x: i32,
    cursor_y: i32,
    color: u32,
}

impl GraphicsContext {
    pub fn plot(&mut self, x: i32, y: i32) {
        let idx = (y as usize * self.width + x as usize);
        self.buffer[idx] = self.color;
    }
    
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        // Bresenham's line algorithm
    }
    
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.cursor_x = x;
        self.cursor_y = y;
    }
    
    pub fn update(&mut self) {
        self.window.update_with_buffer(&self.buffer, self.width, self.height);
    }
}

// 2. Add to Executor
struct Executor {
    graphics: Option<GraphicsContext>,
}

// 3. Defer complexity - implement only when needed
```

---

### LOW PRIORITY: Sound (SOUND/ENVELOPE)

**Complexity:** High | **Impact:** Low (legacy) | **Est. Time:** 8+ hours

**Recommendation:** Use **rodio** crate for audio

```rust
use rodio::{OutputStream, Sink, Source};

pub struct SoundContext {
    _stream: OutputStream,
    sink: Sink,
}

impl SoundContext {
    pub fn play_tone(&mut self, freq: f32, duration_ms: u32) {
        let source = rodio::source::SineWave::new(freq)
            .take_duration(std::time::Duration::from_millis(duration_ms as u64));
        self.sink.append(source);
    }
}
```

---

## Testing Strategy

### Unit Tests Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature_basic() {
        let mut executor = Executor::new();
        // Setup
        // Execute
        // Assert
    }
    
    #[test]
    fn test_feature_edge_case() {
        // Test boundary conditions
    }
    
    #[test]
    fn test_feature_error_handling() {
        // Verify errors are caught properly
    }
}
```

### Integration Tests

Create `.bbas` test files:
```basic
REM test_local.bbas
10 X = 10
20 PROC test
30 PRINT X  REM Should still be 10
40 END
100 DEF PROC test
110 LOCAL X
120 X = 99
130 ENDPROC
```

Run with:
```bash
./target/release/bbc-basic-interpreter test_local.bbas
```

---

## Useful Code Snippets

### Adding a New Statement Type

```rust
// 1. Add to parser/mod.rs
pub enum Statement {
    NewFeature { param: String },
}

// 2. Add parser function
fn parse_new_feature(tokens: &[Token]) -> Result<Statement> {
    // Parse tokens
    Ok(Statement::NewFeature { param })
}

// 3. Add to parse_statement match
match tokens[0] {
    Token::Keyword(0xXX) => parse_new_feature(&tokens[1..]),
    // ...
}

// 4. Add to executor/mod.rs
impl Executor {
    pub fn execute_statement(&mut self, stmt: &Statement) -> Result<()> {
        match stmt {
            Statement::NewFeature { param } => {
                self.execute_new_feature(param)
            }
            // ...
        }
    }
    
    fn execute_new_feature(&mut self, param: &str) -> Result<()> {
        // Implementation
        Ok(())
    }
}

// 5. Add tests
#[test]
fn test_new_feature() {
    let mut executor = Executor::new();
    let stmt = Statement::NewFeature { param: "test".to_string() };
    executor.execute_statement(&stmt).unwrap();
    // Assert expected behavior
}
```

### Adding a New Function

```rust
// 1. Add to Expression enum (if needed)
pub enum Expression {
    FunctionCall { name: String, args: Vec<Expression> },
}

// 2. Add to eval_integer/eval_real/eval_string
fn eval_integer(&self, expr: &Expression) -> Result<i32> {
    match expr {
        Expression::FunctionCall { name, args } => {
            match name.as_str() {
                "NEWFUNCTION" => self.eval_newfunction_int(args),
                // ...
            }
        }
    }
}

// 3. Implement function
fn eval_newfunction_int(&self, args: &[Expression]) -> Result<i32> {
    if args.len() != 1 {
        return Err(Error::WrongArgCount);
    }
    let arg = self.eval_integer(&args[0])?;
    Ok(arg * 2)  // Example
}
```

---

## Performance Optimization Notes

### Current Bottlenecks (to address later):
1. **Tokenization** on every line execution (cache tokenized form)
2. **HashMap lookups** for variables (consider faster data structures)
3. **String allocations** in expression evaluation (use Cow/references)

### Optimization Strategy:
- **Profile first** - Use `cargo flamegraph` to identify hotspots
- **Optimize execution loop** - Most time spent here
- **Cache parsed statements** - Avoid re-parsing on loops
- **Consider JIT compilation** - For hot loops (advanced, future)

---

## Development Workflow

### Feature Implementation Checklist:
- [ ] Create feature branch: `git checkout -b feature/feature-name`
- [ ] Update `specs/features.md` - mark feature as in-progress
- [ ] Write failing test (TDD Red phase)
- [ ] Implement parser changes
- [ ] Implement executor changes
- [ ] Implement runtime changes (main.rs)
- [ ] Make test pass (TDD Green phase)
- [ ] Refactor if needed (TDD Refactor phase)
- [ ] Add integration test (.bbas file)
- [ ] Update HELP text if user-facing
- [ ] Commit with descriptive message
- [ ] Merge to main
- [ ] Update `specs/features.md` - mark complete

---

## Quick Reference: Key Files to Modify

| Feature Type | Files to Touch |
|--------------|----------------|
| New statement | `parser/mod.rs`, `executor/mod.rs` |
| New function | `parser/mod.rs`, `executor/mod.rs` (eval_*) |
| New operator | `tokenizer/mod.rs`, `parser/mod.rs`, `executor/mod.rs` |
| Control flow | `parser/mod.rs`, `main.rs` (runtime loop) |
| REPL command | `main.rs` only |
| File operations | `filesystem/mod.rs`, `executor/mod.rs` |
| Graphics | `graphics/mod.rs`, `executor/mod.rs` |

---

## External Dependencies

**Current (Cargo.toml):**
```toml
[dependencies]
rand = "0.8"  # For RND function
```

**Recommended for future features:**
```toml
rodio = "0.17"         # Sound (SOUND/ENVELOPE)
minifb = "0.25"        # Graphics (PLOT/DRAW)
# OR
pixels = "0.13"        # Alternative graphics
```

---

## Summary Priorities (Updated December 22, 2024)

### ✅ Recently Completed (December 22, 2024):
1. ✅ **LOCAL** - Complete PROC/FN scoping (DONE)
2. ✅ **DEF FN** - User-defined functions (DONE)
3. ✅ **ON GOTO/GOSUB** - Computed jumps (DONE)
4. ✅ **Missing operators** - MOD/DIV/^ (DONE)
5. ✅ **Error handling** - ON ERROR/ERL/ERR (DONE)
6. ✅ **File I/O** - OPENIN/OPENOUT/PRINT#/INPUT#/CLOSE#/EOF# (DONE)
7. ✅ **WHILE...ENDWHILE** - While loops (DONE - 2 hours actual)

### Immediate Next Steps (HIGH PRIORITY):

1. **Missing string functions** - INSTR with start pos, STRING$, UPPER$, LOWER$ (2-3 hours)
   - Complete string manipulation toolkit
   - INSTR(string, search, start) - Find substring from position
   - STRING$(count, char) - Repeat character n times
   - Add UPPER$ and LOWER$ if not present

2. **Missing math functions** - LN (natural log), ACS/ASN (inverse trig) (1-2 hours)
   - Complete scientific math library
   - LN(x) - Natural logarithm (base e)
   - ACS(x) - Arc cosine (inverse cos)
   - ASN(x) - Arc sine (inverse sin)

### Short-term (MEDIUM PRIORITY):

3. **Enhanced error handling** - ERROR statement, REPORT statement (1 hour)
   - ERROR num, "message" - Raise custom error
   - REPORT - Print last error message

### Long-term (LOW PRIORITY):

4. **Graphics** - PLOT/DRAW/CIRCLE (10+ hours)
   - Requires graphics library integration (minifb or pixels)
   - Legacy feature, low utility for modern use

5. **Sound** - SOUND/ENVELOPE (8+ hours)
   - Requires audio library (rodio)
   - Legacy feature, low utility

6. **Advanced features** - CALL/USR, memory operations (!/?/$)
   - Machine code integration
   - Very low priority

**Current Status (2024-12-22 - End of Session):**
- **~80%** feature complete (up from 78%)
- **~98%** core language complete (up from 95%)
- **100%** console I/O complete
- **80%** file I/O complete
- **169** unit tests passing (up from 166, +3 WHILE tests)
- **~7900** lines of code (up from 7600, +300 LOC)

**Estimated to "fully usable":** ACHIEVED ✅ (Error handling + File I/O + All loop types complete)
**Estimated to "feature complete":** +3-5 hours (missing string/math functions only)
**Estimated to "complete":** +30 hours (all features except graphics/sound)

