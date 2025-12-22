use bbc_basic_interpreter::{
    tokenizer::tokenize,
    parser::parse_statement,
    executor::Executor,
};
use std::io::{self, Write};

fn main() {
    println!("BBC BASIC Interpreter v0.1.0");
    println!("Type 'EXIT' to quit, 'HELP' for help\n");

    let mut executor = Executor::new();
    let stdin = io::stdin();
    let mut line_buffer = String::new();

    loop {
        // Prompt
        print!("> ");
        io::stdout().flush().unwrap();

        // Read line
        line_buffer.clear();
        if stdin.read_line(&mut line_buffer).is_err() {
            break;
        }

        let input = line_buffer.trim();

        // Check for commands
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }

        if input.eq_ignore_ascii_case("help") {
            print_help();
            continue;
        }

        if input.is_empty() {
            continue;
        }

        // Execute the line
        match execute_line(&mut executor, input) {
            Ok(()) => {},
            Err(e) => println!("Error: {}", e),
        }
    }
}

fn execute_line(executor: &mut Executor, line: &str) -> Result<(), String> {
    // Tokenize
    let tokenized = tokenize(line)
        .map_err(|e| format!("Tokenization error: {:?}", e))?;

    // Parse
    let statement = parse_statement(&tokenized)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    // Execute
    executor.execute_statement(&statement)
        .map_err(|e| format!("Runtime error: {:?}", e))?;

    Ok(())
}

fn print_help() {
    println!("BBC BASIC Interpreter - Available Commands:");
    println!();
    println!("Statements:");
    println!("  LET A% = 42              - Assign integer variable");
    println!("  B = 3.14                 - Assign real variable (LET optional)");
    println!("  C$ = \"HELLO\"             - Assign string variable");
    println!("  PRINT \"text\", A%, B      - Print values");
    println!("  FOR I% = 1 TO 10         - Start FOR loop");
    println!("  NEXT I%                  - End FOR loop");
    println!("  INPUT A%, B$             - Input variables (test mode: sets to 0/\"\")");
    println!("  DIM A%(10)               - Dimension array");
    println!("  GOSUB 1000               - Call subroutine");
    println!("  RETURN                   - Return from subroutine");
    println!("  REM comment              - Comment");
    println!("  END                      - End program");
    println!();
    println!("Examples:");
    println!("  A% = 5 + 3");
    println!("  PRINT \"Result:\"; A%");
    println!("  FOR I% = 1 TO 3: PRINT I%: NEXT I%");
    println!();
    println!("Variable Types:");
    println!("  A%  - Integer variable");
    println!("  B   - Real (float) variable");
    println!("  C$  - String variable");
    println!();
}
