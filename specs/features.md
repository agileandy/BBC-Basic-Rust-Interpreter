# BBC BASIC Feature Checklist

## Core Language Features

### Variables & Data Types
- [x] Integer variables (%) - 32-bit signed integers with % suffix
- [x] Real variables (default) - Floating-point numbers
- [x] String variables ($) - Character strings with $ suffix
- [x] Array variables - Multi-dimensional arrays with DIM
- [x] Variable assignment - LET keyword (optional) for assignment

### Operators

#### Arithmetic Operators
- [x] Addition (+) - Add two numbers
- [x] Subtraction (-) - Subtract two numbers
- [x] Multiplication (*) - Multiply two numbers
- [x] Division (/) - Divide two numbers
- [x] Unary minus (-) - Negate a number
- [x] Integer division (DIV) - Integer quotient of division
- [x] Modulo (MOD) - Remainder of integer division
- [x] Exponentiation (^) - Raise to power

#### Comparison Operators
- [x] Equal (=) - Test equality
- [x] Not equal (<>) - Test inequality
- [x] Less than (<) - Test less than
- [x] Greater than (>) - Test greater than
- [x] Less than or equal (<=) - Test less than or equal
- [x] Greater than or equal (>=) - Test greater than or equal

#### Logical Operators
- [x] AND - Logical AND operation
- [x] OR - Logical OR operation
- [x] NOT - Logical NOT operation
- [x] EOR - Exclusive OR operation

#### Bitwise Operators
- [ ] Left shift (<<) - Shift bits left
- [ ] Right shift (>>) - Shift bits right

### Control Flow

#### Conditional Statements
- [x] IF...THEN - Simple conditional execution
- [x] IF...THEN...ELSE - Conditional with alternative
- [ ] IF...THEN...ELSE...ENDIF - Multi-line conditional blocks

#### Loop Statements
- [x] FOR...NEXT - Counted loop with optional STEP
- [x] FOR...TO...STEP - Loop with custom increment
- [x] REPEAT...UNTIL - Loop until condition true
- [ ] WHILE...ENDWHILE - Loop while condition true

#### Branch Statements
- [x] GOTO - Unconditional jump to line number
- [x] GOSUB - Call subroutine at line number
- [x] RETURN - Return from GOSUB subroutine
- [x] ON...GOTO - Computed GOTO based on expression
- [x] ON...GOSUB - Computed GOSUB based on expression
- [x] ON ERROR - Error handler setup
- [x] ON ERROR OFF - Disable error handler

#### Procedures and Functions
- [x] DEF PROC - Define named procedure
- [x] PROC - Call named procedure
- [x] ENDPROC - End procedure definition
- [x] DEF FN - Define named function (returns value)
- [x] FN - Call named function
- [x] LOCAL - Declare local variables in PROC/FN
- [ ] RETURN (expression) - Return value from function

### Program Control
- [x] END - Terminate program execution
- [x] STOP - Halt program (same as END)
- [ ] QUIT - Exit interpreter
- [x] REM - Comment/remark (ignored)
- [ ] ' - Alternative comment syntax (apostrophe)

### Data Storage
- [x] DATA - Define inline data values
- [x] READ - Read values from DATA statements
- [x] RESTORE - Reset DATA pointer to start
- [ ] RESTORE line_number - Reset DATA pointer to specific line

## Input/Output

### Console I/O
- [x] PRINT - Output text and values
- [x] PRINT ; - Suppress newline (semicolon)
- [x] PRINT , - Tab to next column (comma)
- [x] TAB(n) - Tab to specific column
- [x] SPC(n) - Print n spaces
- [x] INPUT - Read user input into variables
- [x] INPUT "prompt", var - Input with prompt string
- [ ] INPUT LINE - Read entire line as string
- [ ] GET - Read single character (no echo)
- [ ] GET$ - Read single character as string
- [ ] INKEY(n) - Timed keyboard read (n centiseconds)
- [ ] INKEY$(n) - Timed keyboard read returning string

### Screen Control
- [x] CLS - Clear screen
- [ ] MODE n - Set screen mode
- [ ] VDU code - Send control code to display
- [ ] COLOUR n - Set text color (0-15)
- [ ] GCOL mode, color - Set graphics color

### File I/O
- [x] SAVE "filename" - Save program to disk
- [x] LOAD "filename" - Load program from disk
- [x] CHAIN "filename" - Load and run program
- [x] *CAT - Catalog disk files
- [ ] OPENIN "file" - Open file for reading
- [ ] OPENOUT "file" - Open file for writing
- [ ] OPENUP "file" - Open file for read/write
- [ ] CLOSE# channel - Close file channel
- [ ] INPUT# channel, var - Read from file
- [ ] PRINT# channel, data - Write to file
- [ ] BGET# channel - Read byte from file
- [ ] BPUT# channel, byte - Write byte to file
- [ ] PTR# channel - Get file pointer position
- [ ] PTR# channel = pos - Set file pointer position
- [ ] EXT# channel - Get file size
- [ ] EOF# channel - Test for end of file

## Graphics

### Drawing Commands
- [ ] PLOT mode, x, y - Plot point or draw line
- [ ] DRAW x, y - Draw line to coordinates
- [ ] MOVE x, y - Move graphics cursor
- [ ] POINT(x, y) - Read pixel color at coordinates
- [ ] LINE x1, y1, x2, y2 - Draw line between points
- [ ] CIRCLE x, y, radius - Draw circle
- [ ] ELLIPSE x, y, a, b - Draw ellipse
- [ ] RECTANGLE x, y, w, h - Draw rectangle
- [ ] FILL x, y - Flood fill from point
- [ ] ORIGIN x, y - Set graphics origin
- [ ] CLG - Clear graphics area

### Graphics Settings
- [ ] GCOL mode, color - Set graphics color and mode
- [ ] VDU 29, x; y; - Set graphics origin

## Sound & Music

### Sound Commands
- [ ] SOUND channel, amplitude, pitch, duration - Play sound
- [ ] ENVELOPE n, params... - Define sound envelope
- [ ] TEMPO n - Set music tempo
- [ ] VOICE n, name - Select voice/waveform
- [ ] STEREO channel, position - Set stereo position
- [ ] ADVAL(n) - Read analog input (including sound)

## String Functions

### String Manipulation
- [x] LEFT$(string, n) - Extract leftmost n characters
- [x] RIGHT$(string, n) - Extract rightmost n characters
- [x] MID$(string, start, len) - Extract substring
- [x] LEN(string) - Get string length
- [x] CHR$(n) - Convert ASCII code to character
- [x] ASC(string) - Get ASCII code of first character
- [x] STR$(number) - Convert number to string
- [x] VAL(string) - Convert string to number
- [x] INSTR(string, substring) - Find substring position
- [ ] INSTR(string, substring, start) - Find substring from position
- [ ] STRING$(n, char) - Create string of n repeated characters
- [ ] UPPER$(string) - Convert to uppercase
- [ ] LOWER$(string) - Convert to lowercase

### String Operators
- [x] + - String concatenation

## Mathematical Functions

### Basic Math
- [x] ABS(n) - Absolute value
- [x] SGN(n) - Sign of number (-1, 0, 1)
- [x] INT(n) - Integer part (floor)
- [x] SQR(n) - Square root
- [ ] LN(n) - Natural logarithm
- [x] LOG(n) - Base-10 logarithm
- [x] EXP(n) - Exponential (e^n)
- [ ] SQRT(n) - Square root (alternative to SQR)

### Trigonometric Functions
- [x] SIN(n) - Sine (radians)
- [x] COS(n) - Cosine (radians)
- [x] TAN(n) - Tangent (radians)
- [x] ATN(n) - Arctangent (radians)
- [ ] ACS(n) - Arccosine (radians)
- [ ] ASN(n) - Arcsine (radians)
- [ ] DEG(n) - Convert radians to degrees
- [ ] RAD(n) - Convert degrees to radians

### Random Numbers
- [x] RND - Random number 0-1
- [x] RND(n) - Random integer 1 to n

### Constants
- [x] PI - Value of π (3.14159...)
- [ ] TRUE - Boolean true value (-1)
- [ ] FALSE - Boolean false value (0)

## System Functions

### Memory & System
- [ ] HIMEM - Top of available memory
- [ ] LOMEM - Bottom of available memory
- [ ] PAGE - Start of program memory
- [ ] TOP - End of program
- [ ] FREE - Amount of free memory
- [ ] END - Address of end of variables

### System Information
- [ ] TIME - Centisecond counter since power-on
- [ ] TIME$ - System time as string
- [ ] INKEY$ - Read keyboard buffer

### Error Handling
- [ ] ERROR n, "message" - Generate error
- [x] ERL - Line number of last error
- [x] ERR - Error number of last error
- [ ] REPORT - Print last error message
- [ ] REPORT$ - Get last error message as string

### Program Execution
- [ ] RUN - Run program from start
- [ ] RUN line_number - Run from specific line
- [ ] GOTO line_number - Jump to line
- [ ] GOSUB line_number - Call subroutine
- [ ] CHAIN "file" - Load and run program (✓ implemented)
- [ ] CALL address - Call machine code
- [ ] USR(address) - Call machine code function

## Operating System Commands

### File System
- [x] *CAT - Catalog current directory
- [ ] *CAT directory - Catalog specific directory
- [ ] *.filename - Run OS command with filename
- [ ] OSCLI command - Execute OS command

### Cassette/Tape Operations (Historical)
- [x] SAVE "filename" - Save program (disk version implemented)
- [x] LOAD "filename" - Load program (disk version implemented)
- [ ] SAVE "filename" start end - Save memory block
- [ ] LOAD "filename" address - Load to specific address
- [ ] *TAPE - Select tape filing system
- [ ] *DISC - Select disk filing system

## Advanced Features

### Assembler
- [ ] [ - Start assembler section
- [ ] ] - End assembler section
- [ ] OPT n - Set assembler options

### Miscellaneous
- [ ] LIST - List program (REPL command implemented)
- [ ] NEW - Clear program (REPL command implemented)
- [ ] OLD - Recover last NEW'd program
- [ ] RENUMBER - Renumber program lines
- [ ] AUTO - Auto line numbering mode
- [ ] DELETE start, end - Delete line range
- [ ] LVAR - List variables
- [ ] CLEAR - Clear all variables
- [ ] TRACE ON/OFF - Enable/disable trace mode
- [ ] TRACE ENDPROC - Trace procedure calls
- [ ] TRACE line_number - Trace from line

### Memory Operations
- [ ] ! address - Indirect integer access (4 bytes)
- [ ] ? address - Indirect byte access (1 byte)
- [ ] $ address - Indirect string access
- [ ] DIM array LOCAL - Local array declaration

## Summary Statistics

**Total Features**: ~200
**Implemented**: ~60 (30%)
**Core Language Complete**: ~80% (control flow, variables, operators)
**I/O Complete**: ~40% (console done, files partial, graphics/sound none)
**Functions Complete**: ~70% (math/string mostly done)

## Implementation Priority

### High Priority (Core Usage)
- [x] Variables and arrays
- [x] Control flow (IF/FOR/REPEAT/GOTO/GOSUB)
- [x] Procedures (PROC/ENDPROC)
- [x] Console I/O (PRINT/INPUT)
- [x] File operations (SAVE/LOAD/CHAIN)
- [x] Basic math and string functions

### Medium Priority (Enhanced Functionality)
- [x] User-defined functions (DEF FN)
- [x] LOCAL variables
- [x] ON GOTO/GOSUB
- [x] Missing operators (MOD/DIV/^)
- [ ] File I/O (OPENIN/OPENOUT/PRINT#/INPUT#)
- [ ] Error handling (ON ERROR/ERL/ERR)

### Low Priority (Advanced/Legacy)
- [ ] Graphics commands (PLOT/DRAW/CIRCLE)
- [ ] Sound commands (SOUND/ENVELOPE)
- [ ] Assembler support
- [ ] Machine code calls (CALL/USR)
- [ ] Memory manipulation (!/?/$)
- [ ] Cassette operations (historical)
