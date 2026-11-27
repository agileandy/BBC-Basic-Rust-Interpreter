//! The full-screen IDE (`--ide`): a tab bar for EDIT, RUN and FILES, one pane
//! for the current mode, and a status line.
//!
//! The state (`App`) is separate from the drawing and from the key reading.
//! A test sends key events to `App::key` and draws with a test backend.

mod editor;
mod files;
mod frame;
mod run;

use crate::executor::Executor;
use crate::program::ProgramStore;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;
use ratatui::Terminal;
use std::io::{self, IsTerminal, Write};

/// The usage line for an argument that is not known.
pub const USAGE: &str = "Usage: bbc-basic-interpreter [--ide [file]]";

/// The smallest terminal that shows the frame.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 16;

/// How the binary starts, from its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// The command prompt of today
    Prompt,
    /// The IDE, with an optional program file
    Ide(Option<String>),
}

/// Read the arguments after the binary name. An argument that is not known
/// gives the usage line as the error.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Start, &'static str> {
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(Start::Prompt),
        [flag] if flag == "--ide" => Ok(Start::Ide(None)),
        [flag, file] if flag == "--ide" && !file.starts_with('-') => {
            Ok(Start::Ide(Some(file.clone())))
        }
        _ => Err(USAGE),
    }
}

/// The mode of the IDE. Each mode has the full width of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Run,
    Files,
}

impl Mode {
    /// All modes in the order of the tab bar.
    pub const ALL: [Mode; 3] = [Mode::Edit, Mode::Run, Mode::Files];

    pub fn title(self) -> &'static str {
        match self {
            Mode::Edit => "EDIT",
            Mode::Run => "RUN",
            Mode::Files => "FILES",
        }
    }

    fn next(self) -> Mode {
        match self {
            Mode::Edit => Mode::Run,
            Mode::Run => Mode::Files,
            Mode::Files => Mode::Edit,
        }
    }

    fn previous(self) -> Mode {
        match self {
            Mode::Edit => Mode::Files,
            Mode::Run => Mode::Edit,
            Mode::Files => Mode::Run,
        }
    }
}

/// The result of a key that the frame sends to a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The pane used the key
    Used,
    /// The pane did not use the key
    Ignored,
}

/// The text of the question when the program has changes that are not saved.
const QUIT_QUESTION: &str = "Not saved. Press ^Q again to quit";
/// The text for a global key that a later issue gives its action.
const NOT_AVAILABLE: &str = "not available yet";

/// The state of the IDE.
pub struct App {
    mode: Mode,
    file_name: Option<String>,
    changed: bool,
    message: String,
    quit_asked: bool,
    quit: bool,
    editor: editor::EditorPane,
    run: run::RunPane,
    files: files::FilesPane,
    /// The interpreter that the panes share
    pub executor: Executor,
    /// The program that the panes share
    pub program: ProgramStore,
}

impl App {
    pub fn new(file_name: Option<String>) -> Self {
        Self {
            mode: Mode::Edit,
            file_name,
            changed: false,
            message: String::new(),
            quit_asked: false,
            quit: false,
            editor: editor::EditorPane::default(),
            run: run::RunPane::default(),
            files: files::FilesPane::default(),
            executor: Executor::new(),
            program: ProgramStore::new(),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Set the mark for changes that are not saved.
    pub fn set_changed(&mut self, changed: bool) {
        self.changed = changed;
    }

    /// Use one key: first the global keys, then the pane of the current mode.
    pub fn key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        let is_quit = is_ctrl(key, 'q');
        if !is_quit && self.quit_asked {
            // Any other key cancels the question.
            self.quit_asked = false;
            self.message.clear();
        }
        match key.code {
            // Tab never goes to a pane, so it never puts a tab in the text.
            // Some terminals give Shift-Tab as Tab with Shift.
            KeyCode::BackTab => self.mode = self.mode.previous(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.mode = self.mode.previous()
            }
            KeyCode::Tab => self.mode = self.mode.next(),
            _ if is_quit => self.ask_to_quit(),
            _ if ['r', 's', 'o'].iter().any(|&c| is_ctrl(key, c)) => {
                self.message = NOT_AVAILABLE.to_string();
            }
            _ => {
                let _ = match self.mode {
                    Mode::Edit => self.editor.key(key),
                    Mode::Run => self.run.key(key),
                    Mode::Files => self.files.key(key),
                };
            }
        }
    }

    /// Quit, or ask first if the program has changes that are not saved.
    fn ask_to_quit(&mut self) {
        if !self.changed || self.quit_asked {
            self.quit = true;
        } else {
            self.quit_asked = true;
            self.message = QUIT_QUESTION.to_string();
        }
    }

    /// Draw the tab bar, the pane of the current mode and the status line.
    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            frame::draw_too_small(frame, area);
            return;
        }
        let [tabs, pane, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        frame::draw_tabs(frame, tabs, self.mode);
        match self.mode {
            Mode::Edit => self.editor.draw(frame, pane),
            Mode::Run => self.run.draw(frame, pane),
            Mode::Files => self.files.draw(frame, pane),
        }
        frame::draw_status(
            frame,
            status,
            &frame::Status {
                mode: self.mode,
                file_name: self.file_name.as_deref(),
                changed: self.changed,
                message: &self.message,
            },
        );
    }
}

/// True if the key is Ctrl and the letter (any case).
fn is_ctrl(key: KeyEvent, letter: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&letter))
}

/// Write the commands that change to the alternate screen of the terminal and
/// clear it. `Terminal::clear` is not used: it asks the terminal for the
/// cursor position, and some terminals do not answer.
pub fn enter_screen(out: &mut impl Write) -> io::Result<()> {
    crossterm::execute!(out, EnterAlternateScreen, Clear(ClearType::All), Hide)
}

/// Write the commands that go back from the alternate screen.
pub fn leave_screen(out: &mut impl Write) -> io::Result<()> {
    crossterm::execute!(out, LeaveAlternateScreen, Show)
}

/// Leave the raw mode and the alternate screen. Use it on each exit path.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = leave_screen(&mut io::stdout());
}

/// Puts the terminal in the raw mode and the alternate screen, and restores
/// it when it goes out of scope or at a panic.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        // The hook of today runs after the restore, so that its text shows.
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            previous_hook(info);
        }));
        enable_raw_mode()?;
        let guard = TerminalGuard;
        enter_screen(&mut io::stdout())?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Start the IDE. Give the exit code of the process.
pub fn main(file_name: Option<String>) -> i32 {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("The IDE needs a terminal");
        return 2;
    }
    let result = run(App::new(file_name));
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// The event loop: draw, read one event, until the app quits.
fn run(mut app: App) -> io::Result<()> {
    let _guard = TerminalGuard::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    while !app.should_quit() {
        // A resize event only draws the frame again at the new size.
        terminal.draw(|frame| app.draw(frame))?;
        if let Event::Key(key) = event::read()? {
            app.key(key);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn no_argument_starts_the_prompt() {
        assert_eq!(parse_args(args(&[])), Ok(Start::Prompt));
    }

    #[test]
    fn ide_flag_starts_the_ide_with_an_optional_file() {
        assert_eq!(parse_args(args(&["--ide"])), Ok(Start::Ide(None)));
        assert_eq!(
            parse_args(args(&["--ide", "a.bbas"])),
            Ok(Start::Ide(Some("a.bbas".to_string())))
        );
    }

    #[test]
    fn other_arguments_are_not_known() {
        for list in [
            &["--foo"][..],
            &["a.bbas"][..],
            &["--ide", "--ide"][..],
            &["--ide", "a.bbas", "b.bbas"][..],
        ] {
            assert_eq!(parse_args(args(list)), Err(USAGE), "{list:?}");
        }
    }

    #[test]
    fn enter_screen_writes_the_alternate_screen_and_hides_the_cursor() {
        let mut out = Vec::new();
        enter_screen(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\x1b[?1049h"), "{text:?}");
        assert!(text.contains("\x1b[2J"), "{text:?}");
        assert!(text.contains("\x1b[?25l"), "{text:?}");
    }

    #[test]
    fn leave_screen_restores_the_main_screen_and_the_cursor() {
        let mut out = Vec::new();
        leave_screen(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\x1b[?1049l"), "{text:?}");
        assert!(text.contains("\x1b[?25h"), "{text:?}");
    }

    #[test]
    fn a_key_release_is_ignored() {
        let mut app = App::new(None);
        let mut key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        app.key(key);
        assert_eq!(app.mode(), Mode::Edit);
    }

    #[test]
    fn ctrl_q_in_upper_case_also_quits() {
        let mut app = App::new(None);
        app.key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::CONTROL));
        assert!(app.should_quit());
    }
}
