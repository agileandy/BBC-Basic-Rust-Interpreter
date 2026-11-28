use bbc_basic_interpreter::{
    commands::{run_command, CommandOutcome},
    executor::Executor,
    program::ProgramStore,
    sound::{self, map, WavRecorder},
    ui,
};
use std::io::{self, Write};
use std::time::Duration;

/// The usage line of the sound flags (issue #211).
const SOUND_USAGE: &str =
    "       bbc-basic-interpreter --sound-map [NOTE | HZ | --play]\n       bbc-basic-interpreter [--sound-wav file.wav]";

/// The longest wait at the end for the sound of a recording to end. Five
/// notes of the longest duration in each queue take about 64 seconds.
const RECORDING_TAIL_LIMIT: Duration = Duration::from_secs(80);

/// A request of the sound mapping tool (issue #211).
#[derive(Debug, Clone, PartialEq, Eq)]
enum MapRequest {
    /// `--sound-map`: the table of all pitch values
    Table,
    /// `--sound-map NOTE` or `--sound-map HZ`: the nearest pitch values
    Nearest(String),
    /// `--sound-map --play`: the interactive list
    Play,
}

/// The sound flags, and the arguments for `ui::parse_args`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SoundArgs {
    Map(MapRequest),
    Run {
        wav: Option<String>,
        rest: Vec<String>,
    },
}

/// Take the sound flags out of the arguments. `--sound-map` must be the
/// first argument and has no other arguments. `--sound-wav file` can come
/// with the other arguments.
fn take_sound_args(args: Vec<String>) -> Result<SoundArgs, ()> {
    if args.first().is_some_and(|arg| arg == "--sound-map") {
        return match &args[1..] {
            [] => Ok(SoundArgs::Map(MapRequest::Table)),
            [flag] if flag == "--play" => Ok(SoundArgs::Map(MapRequest::Play)),
            [query] if !query.starts_with('-') => {
                Ok(SoundArgs::Map(MapRequest::Nearest(query.clone())))
            }
            _ => Err(()),
        };
    }
    let mut wav = None;
    let mut rest = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--sound-wav" {
            match args.next() {
                Some(file) if wav.is_none() && !file.starts_with('-') => wav = Some(file),
                _ => return Err(()),
            }
        } else {
            rest.push(arg);
        }
    }
    Ok(SoundArgs::Run { wav, rest })
}

/// Show the usage lines and end with status 2.
fn usage_exit() -> ! {
    eprintln!("{}\n{}", ui::USAGE, SOUND_USAGE);
    std::process::exit(2);
}

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

    // `--sound-map` runs the sound mapping tool and ends (issue #211)
    let Ok(sound_args) = take_sound_args(std::env::args().skip(1).collect()) else {
        usage_exit();
    };
    let (wav, rest) = match sound_args {
        SoundArgs::Map(request) => std::process::exit(run_sound_map(request)),
        SoundArgs::Run { wav, rest } => (wav, rest),
    };

    // `--ide [file]` starts the IDE. No argument starts the command prompt.
    // `--no-sound` opens no audio device.
    let Ok(options) = ui::parse_options(rest) else {
        usage_exit();
    };
    sound::use_device(!options.no_sound);
    if let ui::Start::Ide(file_name) = options.start {
        // The recording is for the command prompt only
        if wav.is_some() {
            usage_exit();
        }
        std::process::exit(ui::main(file_name));
    }

    let recorder = match wav
        .as_deref()
        .map(|file| WavRecorder::create(file.as_ref()))
    {
        None => None,
        Some(Ok(recorder)) => Some(recorder),
        Some(Err(error)) => {
            eprintln!("Cannot create {}: {}", wav.unwrap_or_default(), error);
            std::process::exit(2);
        }
    };

    let on_thread = recorder.clone();
    if let Err(error) = run_on_thread(INTERPRETER_STACK_SIZE, move || run_interpreter(on_thread)) {
        // Deep calls can then overflow the smaller stack of the main thread
        eprintln!("Warning: no thread with a 64 MiB stack ({error}), so the main thread runs the interpreter");
        run_interpreter(recorder);
    }
}

/// Run a request of the sound mapping tool, and give the exit status.
fn run_sound_map(request: MapRequest) -> i32 {
    let mut out = io::stdout();
    let result = match request {
        MapRequest::Table => map::print_table(&mut out).map_err(|error| error.to_string()),
        MapRequest::Nearest(query) => map::print_nearest(&query, &mut out),
        MapRequest::Play => {
            // The device, or a note to stderr if it does not start (issue #210)
            sound::use_device(true);
            map::play_interactive(sound::factory(Box::new(io::stderr())))
                .map_err(|error| error.to_string())
        }
    };
    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    }
}

/// Run `run` on a new thread with a stack of `stack_size` bytes, and wait
/// for it to end. Give the error if the system refuses the thread. A panic
/// in the thread continues on this thread.
fn run_on_thread(stack_size: usize, run: impl FnOnce() + Send + 'static) -> io::Result<()> {
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

/// Read and run lines until EXIT, QUIT or the end of input. With a
/// recorder, record the sound, and at the end wait for the sound to end and
/// close the file.
fn run_interpreter(recorder: Option<WavRecorder>) {
    println!("BBC BASIC Interpreter v0.1.0");
    println!("Type 'EXIT' to quit, 'HELP' for help\n");

    let mut executor = Executor::new();
    // The first SOUND opens the audio device, or writes a note to stderr
    executor.set_sound_output(sound::factory(Box::new(io::stderr())));
    if let Some(recorder) = &recorder {
        executor.record_sound(recorder.clone());
    }
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

    if let Some(recorder) = recorder {
        executor.wait_for_sound(RECORDING_TAIL_LIMIT);
        executor.stop_sound();
        if let Err(error) = recorder.finish() {
            eprintln!("Error: the sound recording failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_closed_output_panic, run_on_thread, take_sound_args, MapRequest, SoundArgs};
    use std::sync::atomic::{AtomicBool, Ordering};

    static THREAD_RAN: AtomicBool = AtomicBool::new(false);

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn test_sound_map_takes_a_query_or_play() {
        let map = |list: &[&str]| take_sound_args(args(list));
        assert_eq!(map(&["--sound-map"]), Ok(SoundArgs::Map(MapRequest::Table)));
        assert_eq!(
            map(&["--sound-map", "--play"]),
            Ok(SoundArgs::Map(MapRequest::Play))
        );
        assert_eq!(
            map(&["--sound-map", "F#3"]),
            Ok(SoundArgs::Map(MapRequest::Nearest("F#3".to_string())))
        );
        assert_eq!(map(&["--sound-map", "C4", "--play"]), Err(()));
        assert_eq!(map(&["--sound-map", "--ide"]), Err(()));
    }

    #[test]
    fn test_sound_wav_takes_a_file_and_keeps_the_other_arguments() {
        assert_eq!(
            take_sound_args(args(&["--ide", "--sound-wav", "a.wav"])),
            Ok(SoundArgs::Run {
                wav: Some("a.wav".to_string()),
                rest: args(&["--ide"]),
            })
        );
        assert_eq!(
            take_sound_args(args(&[])),
            Ok(SoundArgs::Run {
                wav: None,
                rest: vec![]
            })
        );
        assert_eq!(take_sound_args(args(&["--sound-wav"])), Err(()));
        assert_eq!(take_sound_args(args(&["--sound-wav", "--ide"])), Err(()));
        assert_eq!(
            take_sound_args(args(&["--sound-wav", "a.wav", "--sound-wav", "b.wav"])),
            Err(())
        );
    }

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
