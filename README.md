# BBC BASIC Interpreter

A Rust implementation of the BBC BASIC language as found on the BBC Micro Model B.

## Features

- ✅ **Program Storage**: Store and execute complete programs with line numbers
- ✅ **Tokenizer**: Converts BBC BASIC source to internal token representation
- ✅ **Parser**: Parses tokenized lines into an Abstract Syntax Tree (AST)
- ✅ **Executor**: Executes parsed statements with full variable and control flow support
- ✅ **Dual Mode**: Immediate execution OR program storage
- ✅ **32K RAM Emulation**: Authentic BBC Micro memory model
- ✅ **Variable Types**: Integer (%), Real (float), and String ($)
- ✅ **Arrays**: Multi-dimensional arrays with DIM statement
- ✅ **Control Flow**: FOR...NEXT loops, GOTO, GOSUB/RETURN
- ✅ **I/O**: PRINT and INPUT statements
- ✅ **Program Commands**: RUN, LIST, NEW

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

The interpreter supports two modes:

**Program Mode (with line numbers):**
```
BBC BASIC Interpreter v0.1.0
Type 'EXIT' to quit, 'HELP' for help

> 10 FOR I% = 1 TO 5
> 20 PRINT I%
> 30 NEXT I%
> 40 END
> LIST
10 FOR I% = 1 TO 5
20 PRINT I%
30 NEXT I%
40 END
> RUN
1
2
3
4
5
> NEW
Program cleared
> EXIT
Goodbye!
```

**Immediate Mode (no line numbers):**
```
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
- **String ($)**: Variable-length strings (max 65535 chars)
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

- **2243 tests** covering all major components
- **Test-Driven Development (TDD)** methodology
- Property-based testing for variable storage
- Unit tests for all statement types
- Graphics tests with PLOT modes, CIRCLE, ELLIPSE, ORIGIN, POINT function
- Program tests that run complete programs and the example programs

## Implementation Status: Partial

This interpreter implements most of the standard BBC BASIC features from the BBC Micro Model B. See "Known Limitations" for the features that do not work.

### Implemented Features

- **Control Flow**: IF...THEN...ELSE (THEN is optional), CASE...OF...WHEN...OTHERWISE...ENDCASE, FOR...NEXT, REPEAT...UNTIL, WHILE...ENDWHILE, GOTO, GOSUB...RETURN
- **Variables**: Integer (%), Real, String ($), Arrays (multi-dimensional)
- **Functions**: DEF FN with parameters, user-defined procedures (DEF PROC...ENDPROC)
- **Built-in Functions**: SIN, COS, TAN, ASN, ACS, ATN, LOG, LN, EXP, SQR, ABS, SGN, INT, PI, DEG, RAD, RND, EVAL. A function of one value works with no brackets (`LEN A$`, `SIN X`)
- **String Functions**: LEFT$, RIGHT$, MID$, CHR$, ASC, STR$, VAL, LEN, INSTR
- **Graphics**: MOVE, DRAW, PLOT (lines, points, triangles, circles and ellipses), CIRCLE, ELLIPSE, RECTANGLE (each with FILL), FILL, CLG, GCOL with a colour for each pixel, COLOUR
- **Graphics Origin**: ORIGIN x,y command for coordinate transformation
- **Pixel Reading**: POINT(x,y) function returns pixel state
- **File I/O**: OPENIN, OPENOUT, OPENUP, PRINT#, INPUT#, BGET#, BPUT#, PTR#, EXT#, EOF#, CLOSE#
- **Error Handling**: ON ERROR GOTO, ON ERROR with statements, ON ERROR OFF, ERR, ERL, REPORT, ERROR statement
- **Other**: DATA, READ, RESTORE, DIM, LOCAL, SWAP, END, STOP, QUIT, CLS, LIST, NEW, OLD, SAVE, LOAD, CHAIN

### 📊 Test Results

- **2243 total tests** - All passing (511 library + 5 binary + 1727 program tests in one test program, `tests/it`, with a corpus of 31 programs in `tests/corpus`)
- Zero warnings with `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check` passes

### Known Limitations

- **Sound**: SOUND, ENVELOPE, TEMPO and VOICE are tokenized, but they are not implemented.
- **Numbers**: integer `+`, `-` and `*` give "Too big" on overflow. A BBC Micro changes to real arithmetic. `PRINT` does not put a number in a field of 10 characters (issue #91).
- **Limits**: a call or an expression that nests too deep gives "No room": 1200 levels of evaluation (a simple recursive function to a depth of about 280), and 10000 entries for GOSUB, PROC and each loop type.
- **Screen**: the command prompt shows no graphics and no colours. The terminal IDE (`--ide`) is in work: its frame exists, and its panes are not complete. The interpreter starts in mode 0, not mode 7. `POINT` gives the colour number, and -1 off the screen.
- **Keyboard**: `GET` and `GET$` use the raw terminal mode. That mode is tested through a pseudo-terminal only.
- **Files**: `PRINT#` writes text, not the binary records of a BBC Micro. Each value is one line of the file, and `INPUT#` reads one line for each variable. A string that holds a line feed or a carriage return gives `Bad record` (`ERR` 155). A read past the end of the file gives `EOF` (`ERR` 223).
- **Immediate mode**: a loop typed on more than one line with no line numbers does not work. A typed line is one unit.

### Non-Standard Extensions

The following functions are **NOT** part of the original BBC BASIC specification but are provided as modern extensions:

- `STRING$` - Repeat a string N times
- `LOADJSON$(file$)` - Read a JSON file as compact JSON text. The keys keep the order of the file
- `SAVEJSON file$, json$` - Write JSON text to a file with indentation
- `JSONVALID(json$)` - TRUE when the text is correct JSON
- `LISTDIR$(path$)` - The names in a directory, in name order, with `CHR$(10)` between them. A sub-directory name ends with `/`
- `MKDIR path$` - Make one directory
- `RMDIR path$` - Remove one empty directory
- `NEWLIST`, `LISTADD l, v`, `LISTSET l, i, v`, `LISTINSERT l, i, v`, `LISTREMOVE l, i`, `LISTCLEAR l`, `LISTLEN(l)`, `LISTGET(l, i)`, `LISTGET$(l, i)`, `LISTISSTR(l, i)` - Lists that grow. A program holds a list by a handle in a numeric variable. The index starts at 0
- `NEWMAP`, `MAPPUT m, key$, v`, `MAPGET(m, key$)`, `MAPGET$(m, key$)`, `MAPHAS(m, key$)`, `MAPLEN(m)`, `MAPKEY$(m, n)`, `MAPISSTR(m, key$)`, `MAPREMOVE m, key$`, `MAPCLEAR m` - Maps from a text key to a value, with the keys in the order of the puts
- `FOR EACH v IN collection ... NEXT` - One pass for each element of a list or each key of a map. `NEXT EACH` also ends the loop
- `JSONTOLIST(json$)`, `JSONTOMAP(json$)`, `TOJSON$(handle)`, `LISTKIND$(l, i)`, `MAPKIND$(m, key$)`, `LISTADDCOL l, h`, `MAPPUTCOL m, key$, h` - Convert between JSON text and lists and maps. A nested collection that a program puts by hand needs `LISTADDCOL` or `MAPPUTCOL`
- `UPPER$(s$)`, `LOWER$(s$)` - Change the case of a string
- `REPORT$` - The text of the last error, with no bracket
- `JSONGET$(json$, path$)`, `JSONGET(json$, path$)` - Read a string or a number at a path of a JSON text
- `JSONTYPE$(json$, path$)` - `object`, `array`, `string`, `number`, `boolean`, `null` or `missing`
- `JSONLEN(json$, path$)`, `JSONKEY$(json$, path$, n)` - The count of an array, an object or a string, and key n of an object (from 0)
- `JSONSET$(json$, path$, value)`, `JSONSETJSON$(json$, path$, fragment$)` - A copy of the text with a value or a JSON fragment at the path
- `JSONADD$(json$, path$, value)`, `JSONDEL$(json$, path$)` - A copy of the text with a value added to an array, or with a value removed

A JSON path has keys with dots (`user.name`), array indices from 0 in square brackets (`items[2]`), and `["my key"]` for a key with a space or a dot. An empty path is the root.

Errors of the extensions: "Bad JSON" is `ERR` 150, "Bad directory" 151, "Bad path" 152, "Bad collection" 153, "No such key" 154, "Bad record" 155 and "FOR EACH without matching NEXT" 156. `ON ERROR` catches each one.


These are documented in `src/extensions/mod.rs`.

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

### Graphics Demo
```basic
> CLG
> GCOL 0, 255
> MOVE 400, 400
> DRAW 600, 400
> DRAW 600, 600
> DRAW 400, 600
> DRAW 400, 400
> CIRCLE 500, 500, 100
```

### User-Defined Functions
```basic
> DEF FNfactorial(n) = IF n<=1 THEN 1 ELSE n*FNfactorial(n-1)
> PRINT FNfactorial(5)
120
```

### Procedures with Parameters
```basic
> DEF PROCSquare(x, y, size)
>   RECTANGLE x, y, size, size
> ENDPROC
>
> PROCSquare(100, 100, 50)
> PROCSquare(200, 200, 75)
```

## Demo Programs

The project includes several demo programs:

| File | Description |
|------|-------------|
| `examples/graphics_demo.bas` | Graphics primitives demo |
| `examples/advanced_graphics_demo.bas` | Advanced graphics with shapes |
| `examples/test_while.bas` | WHILE loop demonstration |
| `examples/test_error_handling.bas` | Error handling demo |
| `examples/sinewave.bas` | Sine wave text animation |

## License

This project is an educational implementation of BBC BASIC.

## Acknowledgments

Based on the BBC BASIC language specification from the BBC Micro Model B.
