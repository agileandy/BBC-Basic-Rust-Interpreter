# BBC BASIC Interpreter - Implementation Plan

## Architecture Overview

### Current Codebase Structure (~6700 LOC)

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

### HIGH PRIORITY: LOCAL Variables

**Complexity:** Medium | **Impact:** High | **Est. Time:** 2-3 hours

**What it does:** Create local variable scope within PROC/FN

**Implementation:**

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

**Implementation:**

```rust
// 1. Add Statements
enum Statement {
    OnGoto { expression: Expression, targets: Vec<u16> },
    OnGosub { expression: Expression, targets: Vec<u16> },
}

// 2. Parser
fn parse_on_statement(tokens: &[Token]) -> Result<Statement> {
    let expr = parse_expression()?;
    
    if next_token_is(Keyword::GOTO) {
        let targets = parse_line_number_list()?;
        Ok(Statement::OnGoto { expression: expr, targets })
    } else if next_token_is(Keyword::GOSUB) {
        let targets = parse_line_number_list()?;
        Ok(Statement::OnGosub { expression: expr, targets })
    }
}

// 3. Runtime in main.rs (control flow)
if is_on_goto {
    if let Statement::OnGoto { expression, targets } = statement {
        let index = executor.eval_integer(&expression)? as usize;
        if index >= 1 && index <= targets.len() {
            let target = targets[index - 1];  // 1-indexed
            program.goto_line(target)?;
        } else {
            // Out of range - fall through to next line
            program.next_line();
        }
    }
}
```

---

### MEDIUM PRIORITY: Error Handling (ON ERROR / ERL / ERR)

**Complexity:** High | **Impact:** Medium | **Est. Time:** 4-6 hours

**What it does:** Catch runtime errors and handle them gracefully

```basic
10 ON ERROR GOTO 1000      REM Set error handler
20 X = 1 / 0                REM Would normally crash
30 PRINT "Continued"
1000 PRINT "Error "; ERR; " at line "; ERL
1010 END
```

**Implementation:**

```rust
// 1. Add to Executor
struct Executor {
    error_handler: Option<u16>,      // Line number of error handler
    last_error: Option<ErrorInfo>,
}

struct ErrorInfo {
    error_number: i32,
    error_line: u16,
    message: String,
}

// 2. Add Statements
enum Statement {
    OnError { line_number: u16 },    // ON ERROR GOTO line
    OnErrorOff,                       // ON ERROR OFF
}

// 3. Add Functions
enum Expression {
    ERL,  // Error line number
    ERR,  // Error number
}

// 4. Runtime error handling wrapper in main.rs
loop {
    // Execute statement
    match executor.execute_statement(&statement) {
        Ok(()) => { /* continue */ }
        Err(e) => {
            if let Some(handler_line) = executor.get_error_handler() {
                // Save error info
                executor.set_last_error(ErrorInfo {
                    error_number: e.code(),
                    error_line: current_line,
                    message: e.message(),
                });
                
                // Jump to error handler
                program.goto_line(handler_line)?;
                continue;
            } else {
                // No handler - crash as usual
                return Err(e);
            }
        }
    }
}
```

---

### MEDIUM PRIORITY: File I/O (OPENIN/OPENOUT/PRINT#/INPUT#)

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

## Summary Priorities

### Immediate Next Steps (HIGH):
1. **LOCAL** - Complete PROC/FN scoping (2-3 hours)
2. **DEF FN** - User-defined functions (3-4 hours)
3. **ON GOTO/GOSUB** - Computed jumps (1-2 hours)

### Short-term (MEDIUM):
4. **File I/O** - OPENIN/OPENOUT/PRINT# (4-5 hours)
5. **Error handling** - ON ERROR/ERL/ERR (4-6 hours)
6. **Missing operators** - MOD/DIV/^ (1 hour)

### Long-term (LOW):
7. **Graphics** - PLOT/DRAW (10+ hours)
8. **Sound** - SOUND/ENVELOPE (8+ hours)
9. **Advanced features** - CALL/USR, memory operations

**Current Status:** ~30% feature complete, core language 80% done
**Estimated to "usable":** +20 hours (HIGH + MEDIUM priorities)
**Estimated to "complete":** +50 hours (all features)
