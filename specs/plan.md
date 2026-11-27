# BBC BASIC Interpreter - Implementation Plan

## Reference Documentation

**BBC BASIC for Windows Manual:** https://www.bbcbasic.co.uk/bbcwin/manual/index.html

This comprehensive manual covers BBC BASIC syntax, functions, statements, and extensions. Use this as the authoritative reference when implementing features.

---

## Current Status

The interpreter is partial. It runs many BBC BASIC programs, but some features are missing or have defects.

- The measured state is in `README.md`: the test counts and the section "Known Limitations".
- The open work is in the Gitea issue tracker, in milestones.

This document does not copy test counts or lists of limitations, because they go out of date. The history of the work is in `development-log.md`, `SESSION_SUMMARY_2024-12-22.md` and the git log.

---

## Architecture Overview

### Codebase Structure

```
src/
├── main.rs              - REPL loop and command handlers (LIST, SAVE, LOAD, CAT, HELP)
├── lib.rs               - Public API exports
├── runner/              - Program run loop and control flow between statements
├── tokenizer/           - Lexical analysis (text → tokens)
├── parser/              - Syntax analysis (tokens → AST)
├── executor/            - Statement execution and expression evaluation
├── variables/           - Variable storage (integer, real, string, arrays)
├── program/             - Program line storage and positions
├── memory/              - Heap memory management
├── filesystem/          - File I/O implementation
├── graphics/            - Graphics operations
├── sound/               - Sound system (stub, not implemented)
├── extensions/          - Documentation of the non-standard functions (UPPER$/LOWER$/STRING$/REPORT$)
└── os/                  - OS interface (stub)
```

### Core Architecture Pattern

**Three-Stage Pipeline:**
1. **Tokenizer** → Convert text to token stream (handles BBC BASIC keywords as bytes 0x80-0xFF)
2. **Parser** → Convert tokens to Statement/Expression AST
3. **Executor** → Execute statements, manage runtime state

**Control Flow Handling:**
- The run loop is in `src/runner/mod.rs`. It handles GOTO, GOSUB, IF, the loops, PROC, multi-line FN and ON ERROR.
- A `Position` (in `src/program/mod.rs`) identifies one statement: a line number and a statement index in that line.
- The runner makes a run list for each line. An IF becomes a list of items, so that each statement in a THEN or ELSE branch has a position.
- The executor keeps these stacks of positions:
  - `return_stack` - GOSUB/RETURN and PROC/ENDPROC calls
  - `for_loops` - FOR loop state (variable, end, step, position)
  - `repeat_stack` - REPEAT positions
  - `while_stack` - WHILE positions
- The executor also keeps `local_stack`, the LOCAL variable frames for PROC and FN.
- `ProgramStore` manages the current position (`goto`, next statement, next line).

---

## Key Design Patterns

### 1. Stack-Based Control Flow

**Used for:** GOSUB/RETURN, PROC/ENDPROC, FOR/NEXT, REPEAT/UNTIL, WHILE/ENDWHILE

```rust
struct Executor {
    return_stack: Vec<Position>,
    for_loops: Vec<(String, f64, f64, Position)>,  // (var, end, step, position)
    repeat_stack: Vec<Position>,
    while_stack: Vec<Position>,
    local_stack: Vec<LocalFrame>,
    // ... other fields
}
```

A PROC call pushes its position on `return_stack`. ENDPROC removes the LOCAL frame, then goes back to the statement after the call.

### 2. Array Access Pattern

**Used for:** Reading and writing array elements

```rust
// Assignment: numbers(I) = value
Statement::ArrayAssignment { name, indices, expression }

// Expression evaluation: x = numbers(I)
Expression::ArrayAccess { name, indices }
```

**Key implementation details:**
- `parse_assignment` detects `identifier(` pattern and creates `ArrayAssignment`
- `parse_primary` detects `identifier(` pattern and creates `ArrayAccess`
- `execute_array_assignment` evaluates indices and stores values
- `eval_integer`/`eval_real`/`eval_string` handle `ArrayAccess` for reading

### 3. Variable Type Handling

**Used for:** All variable access and assignment

In `eval_integer` and `eval_real`, a name with the suffix `%` is an integer variable. For a name with no suffix, the executor tries a real variable first, then an integer variable. If neither exists, the result is `NoSuchVariable`. The pseudo-variables (`TIME`, `HIMEM`, `LOMEM`, `ERR`, `ERL`) are checked before the variable store.

---

## Feature List

This list shows the scope of the implementation. It does not show that each feature is free of defects. See "Known Limitations" in `README.md` and the open Gitea issues.

### Core Language

**Variables & Types:**
- Integer variables (suffix `%`)
- Real/Float variables (no suffix)
- String variables (suffix `$`)
- Arrays (multi-dimensional with DIM)

**Operators:**
- Arithmetic: `+`, `-`, `*`, `/`, `^`, `DIV`, `MOD`
- Comparison: `=`, `<>`, `<`, `>`, `<=`, `>=`
- Logical: `AND`, `OR`, `NOT`
- Bitwise: `AND`, `OR`, `EOR`

**Control Flow:**
- IF...THEN...ELSE
- FOR...NEXT (including STEP)
- REPEAT...UNTIL
- WHILE...ENDWHILE
- GOTO
- GOSUB...RETURN
- ON...GOTO
- ON...GOSUB
- END, STOP

**Procedures & Functions:**
- DEF PROC...ENDPROC
- DEF FN (single-line and multi-line functions)
- LOCAL variables
- Parameters

**Data Structures:**
- DATA
- READ
- RESTORE
- DIM (arrays)

**Other Statements:**
- REM (comments)
- LET (assignment)
- CLS
- LIST
- NEW
- OLD
- SAVE
- LOAD
- CHAIN

### Math Functions

- SIN, COS, TAN, ASN, ACS, ATN
- LOG, LN, EXP, SQR
- ABS, INT, SGN
- PI, DEG, RAD
- RND (random number)
- LEN (string length)

### String Functions

- LEFT$, RIGHT$, MID$
- CHR$, ASC
- STR$, VAL
- INSTR
- STRING$ (extension)
- UPPER$, LOWER$ (extensions)

### Graphics

- MOVE, DRAW, PLOT (all modes 0-191)
- CIRCLE, ELLIPSE, RECTANGLE
- FILL
- CLG (clear graphics)
- GCOL, COLOUR
- ORIGIN
- POINT (pixel reading)

### Sound (not implemented)

SOUND, ENVELOPE, TEMPO and VOICE are tokenized, but they are not implemented. `src/sound/mod.rs` is a stub.

### File I/O

- OPENIN, OPENOUT, OPENUP
- PRINT#, INPUT#
- BGET#, BPUT#
- PTR#, EXT#
- EOF#
- CLOSE#

### Error Handling

- ON ERROR GOTO
- ON ERROR OFF
- ERR (error number)
- ERL (error line)
- REPORT$ (error message)
- ERROR statement (raise errors)

### Memory Operations (not available in programs)

`src/memory/mod.rs` has `peek` and `poke` functions. A program cannot use them: PEEK, POKE and the `?` and `!` indirection operators do not work.

---

## Development Workflow

### Testing Strategy

```bash
# Run all tests
cargo test --locked

# Run with output
cargo test --locked -- --nocapture

# Run specific module
cargo test --locked executor::tests

# Run one test file in tests/
cargo test --locked --test program_run_tests

# Run an example program (end the input with EXIT and use a time limit)
cargo build --locked
printf 'LOAD "examples/test_while.bas"\nRUN\nEXIT\n' | perl -e 'alarm 5; exec @ARGV' ./target/debug/bbc-basic-interpreter
```

Unit tests are in each module. Program tests are in `tests/`, with one file for each issue (`tests/issue_<N>_<name>.rs`). Example programs are in `examples/`.

### Code Quality

These 3 commands must pass before a merge:

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Errors use `Result<T>` with `BBCBasicError`.

---

## Quick Reference: Key Files

| Feature Type | Files to Touch |
|--------------|----------------|
| New statement | `parser/mod.rs`, `executor/mod.rs`, `runner/mod.rs` (if it changes control flow) |
| New function | `parser/mod.rs`, `executor/mod.rs` (eval_*) |
| New operator | `tokenizer/mod.rs`, `parser/mod.rs`, `executor/mod.rs` |
| Control flow | `parser/mod.rs`, `runner/mod.rs` (run loop) |
| REPL command | `main.rs` only |
| File operations | `filesystem/mod.rs`, `executor/mod.rs` |
| Graphics | `graphics/mod.rs`, `executor/mod.rs` |
| Sound | `sound/mod.rs`, `executor/mod.rs` |
