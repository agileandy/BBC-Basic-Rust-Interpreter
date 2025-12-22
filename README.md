# BBC BASIC Interpreter

A Rust implementation of the BBC BASIC language as found on the BBC Micro Model B.

## Features

- ✅ **Tokenizer**: Converts BBC BASIC source to internal token representation
- ✅ **Parser**: Parses tokenized lines into an Abstract Syntax Tree (AST)
- ✅ **Executor**: Executes parsed statements with full variable and control flow support
- ✅ **32K RAM Emulation**: Authentic BBC Micro memory model
- ✅ **Variable Types**: Integer (%), Real (float), and String ($)
- ✅ **Arrays**: Multi-dimensional arrays with DIM statement
- ✅ **Control Flow**: FOR...NEXT loops, GOTO, GOSUB/RETURN
- ✅ **I/O**: PRINT and INPUT statements

## Quick Start

### Build and Run

```bash
# Build the interpreter
cargo build --release

# Run interactively
cargo run --release

# Or run the binary directly
./target/release/bbc-basic-interpreter
```

### Interactive REPL

```
BBC BASIC Interpreter v0.1.0
Type 'EXIT' to quit, 'HELP' for help

> A% = 42
> PRINT "The answer is"; A%
The answer is42
> FOR I% = 1 TO 5
> PRINT I%
> NEXT I%
1
2
3
4
5
> EXIT
Goodbye!
```

## Supported Statements

### Variables & Assignment
```basic
LET A% = 42              ' Integer variable
B = 3.14                 ' Real variable (LET optional)
C$ = "HELLO"             ' String variable
```

### Expressions
```basic
X = 2 + 3 * 4            ' Arithmetic with correct precedence
Y = (5 + 3) / 2          ' Parentheses
Z% = 2 ^ 8               ' Power operator (256)
```

### Output
```basic
PRINT "Hello, World!"    ' Print string
PRINT A%                 ' Print variable
PRINT "X="; X            ' Multiple items with semicolon
PRINT A%, B%, C%         ' Comma separator (tab stops)
```

### Input
```basic
INPUT A%                 ' Read integer
INPUT B$                 ' Read string
INPUT X, Y, Z            ' Multiple variables
```

### Loops
```basic
FOR I% = 1 TO 10         ' Simple loop
  PRINT I%
NEXT I%

FOR I% = 10 TO 1 STEP -1 ' Countdown
  PRINT I%
NEXT I%
```

### Arrays
```basic
DIM A%(10)               ' 1D array (0-10, 11 elements)
DIM B%(5, 10)            ' 2D array
DIM C$(20)               ' String array
```

### Control Flow
```basic
GOTO 100                 ' Jump to line
GOSUB 1000               ' Call subroutine
RETURN                   ' Return from subroutine
END                      ' End program
STOP                     ' Stop execution
REM This is a comment    ' Comment
```

## Variable Types

- **Integer (%)**: 32-bit signed integers
  - Example: `A%`, `COUNT%`, `INDEX%`
- **Real (no suffix)**: 64-bit floating point
  - Example: `X`, `PI`, `RESULT`
- **String ($)**: Variable-length strings (max 255 chars)
  - Example: `NAME$`, `MESSAGE$`, `LINE$`

## Binary Operators (by precedence)

1. `^` - Power
2. `*`, `/` - Multiply, Divide
3. `DIV`, `MOD` - Integer division, Modulo
4. `+`, `-` - Add, Subtract
5. `=`, `<>`, `<`, `>`, `<=`, `>=` - Comparison

## Development

### Run Tests
```bash
# Run all tests
cargo test

# Run specific module tests
cargo test tokenizer::tests
cargo test parser::tests
cargo test executor::tests

# Run with output
cargo test -- --nocapture
```

### Project Structure
```
src/
├── main.rs           # REPL entry point
├── lib.rs            # Library root
├── tokenizer/        # Source → Tokens
├── parser/           # Tokens → AST
├── executor/         # AST → Execution
├── variables/        # Variable storage
├── memory/           # 32K RAM emulation
└── [other modules]   # Future expansion
```

## Test Coverage

- **84 tests** covering all major components
- **Test-Driven Development (TDD)** methodology
- Property-based testing for variable storage
- Unit tests for all statement types

## Limitations (Current Version)

This is v0.1.0 with core functionality. Not yet implemented:

- Program storage with line numbers
- IF...THEN...ELSE conditionals
- Array element access
- Built-in functions (SIN, COS, ABS, etc.)
- Graphics commands (PLOT, DRAW, MOVE, etc.)
- Sound commands (SOUND, ENVELOPE)
- File I/O (LOAD, SAVE, OPENIN, OPENOUT)
- PROC/FN user-defined procedures/functions

## Examples

### Simple Countdown
```basic
> FOR I% = 5 TO 1 STEP -1
> PRINT I%
> NEXT I%
5
4
3
2
1
```

### Variable Math
```basic
> A% = 10
> B% = 20
> C% = A% + B% * 2
> PRINT "Result:"; C%
Result:50
```

### String Variables
```basic
> NAME$ = "BBC Micro"
> PRINT "Welcome to "; NAME$
Welcome to BBC Micro
```

## License

This project is an educational implementation of BBC BASIC.

## Acknowledgments

Based on the BBC BASIC language specification from the BBC Micro Model B.
