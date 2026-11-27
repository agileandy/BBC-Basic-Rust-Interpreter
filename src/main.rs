use bbc_basic_interpreter::{
    commands::{run_command, CommandOutcome},
    executor::Executor,
    program::ProgramStore,
    ui,
};
use std::io::{self, Write};

/// True if a panic message comes from a write to a closed output pipe.
fn is_closed_output_panic(message: &str) -> bool {
    message.contains("Broken pipe") || message.contains("BrokenPipe")
}

/// End the process with status 0 when the output closes early.
/// Show all other panics as before, with a status that is not 0.
fn install_closed_output_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload_as_str().is_some_and(is_closed_output_panic) {
            std::process::exit(0);
        }
        default_hook(info);
    }));
}

/// The native stack size of the interpreter thread. A nested FN call can use
/// about 25 KiB of it in a debug build. The main thread has 8 MiB, which
/// overflows before the limits of the executor and the parser (issue 151).
const INTERPRETER_STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() {
    install_closed_output_hook();

    // `--ide [file]` starts the IDE. No argument starts the command prompt.
    match ui::parse_args(std::env::args().skip(1)) {
        Ok(ui::Start::Prompt) => {}
        Ok(ui::Start::Ide(file_name)) => std::process::exit(ui::main(file_name)),
        Err(usage) => {
            eprintln!("{}", usage);
            std::process::exit(2);
        }
    }

    if let Err(error) = run_on_thread(INTERPRETER_STACK_SIZE, run_interpreter) {
        // Deep calls can then overflow the smaller stack of the main thread
        eprintln!("Warning: no thread with a 64 MiB stack ({error}), so the main thread runs the interpreter");
        run_interpreter();
    }
}

/// Run `run` on a new thread with a stack of `stack_size` bytes, and wait
/// for it to end. Give the error if the system refuses the thread. A panic
/// in the thread continues on this thread.
fn run_on_thread(stack_size: usize, run: fn()) -> io::Result<()> {
    let thread = std::thread::Builder::new()
        .name("interpreter".to_string())
        .stack_size(stack_size)
        .spawn(run)?;
    if let Err(panic) = thread.join() {
        // The hook showed the panic: end with the panic status
        std::panic::resume_unwind(panic);
    }
    Ok(())
}

/// Read and run lines until EXIT, QUIT or the end of input
fn run_interpreter() {
    println!("BBC BASIC Interpreter v0.1.0");
    println!("Type 'EXIT' to quit, 'HELP' for help\n");

    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    let stdin = io::stdin();
    let mut line_buffer = String::new();

    loop {
        // Prompt
        print!("> ");
        io::stdout().flush().unwrap();

        // Read line
        line_buffer.clear();
        // Ok(0) is end of input. Stop for it and for an error.
        if matches!(stdin.read_line(&mut line_buffer), Ok(0) | Err(_)) {
            println!();
            break;
        }

        // The commands are shared with the run pane of the IDE
        match run_command(&mut executor, &mut program, line_buffer.trim()) {
            Ok(CommandOutcome::Done) => {}
            Ok(CommandOutcome::Quit) => {
                println!("Goodbye!");
                break;
            }
            Ok(CommandOutcome::InputEnded) => {
                println!();
                break;
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_closed_output_panic, run_on_thread};
    use std::sync::atomic::{AtomicBool, Ordering};

    static THREAD_RAN: AtomicBool = AtomicBool::new(false);

    fn mark_thread_ran() {
        THREAD_RAN.store(true, Ordering::SeqCst);
    }

    #[test]
    fn test_run_on_thread_runs_the_function() {
        assert!(run_on_thread(1024 * 1024, mark_thread_ran).is_ok());
        assert!(THREAD_RAN.load(Ordering::SeqCst));
    }

    #[test]
    fn test_run_on_thread_gives_an_error_when_the_system_refuses_the_stack() {
        fn not_called() {
            panic!("the thread must not start");
        }
        assert!(run_on_thread(1 << 50, not_called).is_err());
    }

    #[test]
    fn test_print_to_closed_stdout_is_closed_output() {
        assert!(is_closed_output_panic(
            "failed printing to stdout: Broken pipe (os error 32)"
        ));
    }

    #[test]
    fn test_unwrap_of_broken_pipe_error_is_closed_output() {
        assert!(is_closed_output_panic(
            "called `Result::unwrap()` on an `Err` value: Os { code: 32, kind: BrokenPipe, message: \"Broken pipe\" }"
        ));
    }

    #[test]
    fn test_other_panic_is_not_closed_output() {
        assert!(!is_closed_output_panic("index out of bounds"));
        assert!(!is_closed_output_panic(
            "failed printing to stdout: Permission denied (os error 13)"
        ));
    }
}
