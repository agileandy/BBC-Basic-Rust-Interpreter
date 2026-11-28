//! The full-screen IDE (`--ide`): a tab bar for EDIT, RUN and FILES, one pane
//! for the current mode, and a status line.
//!
//! The state (`App`) is separate from the drawing and from the key reading.
//! A test sends key events to `App::key` and draws with a test backend.

mod canvas;
mod document;
mod editor;
mod files;
mod frame;
mod help;
mod run;
mod theme;

pub use document::{Document, LineCache, LineMark};

use crate::executor::Executor;
use crate::program::ProgramStore;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;
use ratatui::Terminal;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

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
/// The status text of the edit and files modes while a program runs.
const RUNNING: &str = "running";
/// The longest wait for a terminal event before the next draw.
const EVENT_WAIT: Duration = Duration::from_millis(20);

/// The state that the 3 panes share. The frame gives it to each pane:
/// `draw(&self, frame, area, &Context)` and `key(&mut self, key, &mut Context)`.
///
/// - `document` is the program text: the source. The edit mode changes it.
///   The files mode loads it (replace it with `Document::from_text`) and saves
///   it (`Document::text`, then `set_changed(false)`).
/// - `program` is the stored program. The frame sets it from the text when
///   the user leaves the edit mode. The run mode can make it again with
///   `document.to_program()`, which also gives the marks of the faults.
/// - `executor` is the interpreter for a run.
/// - `message` is the text of the status line. A pane can set it.
pub struct Context {
    pub document: Document,
    pub program: ProgramStore,
    pub executor: Executor,
    pub message: String,
}

/// The state of the IDE.
pub struct App {
    mode: Mode,
    quit_asked: bool,
    quit: bool,
    /// The help screen, while it shows (issue #174)
    help: Option<help::Help>,
    editor: editor::EditorPane,
    run: run::RunPane,
    files: files::FilesPane,
    context: Context,
}

impl App {
    /// The IDE with the text of the file, if it has a file name.
    pub fn new(file_name: Option<String>) -> Self {
        let (document, message) = Document::open(file_name);
        Self {
            mode: Mode::Edit,
            quit_asked: false,
            quit: false,
            help: None,
            editor: editor::EditorPane::new(&document),
            run: run::RunPane::default(),
            files: files::FilesPane::new(document.file_name()),
            context: Context {
                document,
                program: ProgramStore::new(),
                executor: Executor::new(),
                message: message.unwrap_or_default(),
            },
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn message(&self) -> &str {
        &self.context.message
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The program text.
    pub fn document(&self) -> &Document {
        &self.context.document
    }

    /// The stored program, from the text at the last change of mode.
    pub fn program(&self) -> &ProgramStore {
        &self.context.program
    }

    /// The cursor of the editor: the line index and the character index.
    pub fn cursor(&self) -> (usize, usize) {
        self.editor.cursor()
    }

    /// The faults in the text that the editor shows.
    pub fn marks(&self) -> &[LineMark] {
        self.editor.marks()
    }

    /// Set the mark for changes that are not saved.
    pub fn set_changed(&mut self, changed: bool) {
        self.context.document.set_changed(changed);
    }

    /// Put pasted text in the editor, or type it in the run mode. The files
    /// mode ignores a paste.
    pub fn paste(&mut self, text: &str) {
        match self.mode {
            Mode::Edit => self.editor.paste(text, &mut self.context),
            // The run mode takes the text as typed keys, with a line end as
            // Enter (issue #171)
            Mode::Run => {
                for c in text.replace("\r\n", "\n").chars() {
                    let code = match c {
                        '\n' | '\r' => KeyCode::Enter,
                        c => KeyCode::Char(c),
                    };
                    self.key(KeyEvent::new(code, KeyModifiers::NONE));
                }
            }
            Mode::Files => {}
        }
    }

    /// Change the mode. When the user leaves the edit mode, the text becomes
    /// the stored program.
    fn set_mode(&mut self, mode: Mode) {
        if self.mode == Mode::Edit {
            self.editor.leave(&mut self.context);
        }
        if mode == Mode::Edit {
            self.editor.resume(&self.context);
        }
        if mode == Mode::Files {
            self.files.focus(&mut self.context);
        }
        if mode == Mode::Run {
            self.run.open(&mut self.context);
        }
        self.mode = mode;
    }

    /// Use one terminal event: a key or a paste. A resize needs no action:
    /// the next draw uses the new size.
    pub fn event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.key(key),
            Event::Paste(text) => self.paste(&text),
            _ => {}
        }
    }

    /// Use one key: first the global keys, then the pane of the current mode.
    pub fn key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        // The status message shows until the next key. The quit question
        // is the only question in the status line: Ctrl-Q sets it again.
        self.context.message.clear();
        if let Some(help) = &mut self.help {
            // A key that does not scroll the help closes it, and does
            // nothing else
            if !help.key(key) {
                self.help = None;
            }
            return;
        }
        let is_quit = is_ctrl(key, 'q');
        if !is_quit && self.quit_asked {
            // Any other key cancels the question.
            self.quit_asked = false;
        }
        match key.code {
            KeyCode::F(1) => self.help = Some(help::Help::default()),
            _ if is_ctrl(key, 'h') => self.help = Some(help::Help::default()),
            // Tab never goes to a pane, so it never puts a tab in the text.
            // Some terminals give Shift-Tab as Tab with Shift.
            KeyCode::BackTab => self.set_mode(self.mode.previous()),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.set_mode(self.mode.previous())
            }
            KeyCode::Tab => self.set_mode(self.mode.next()),
            _ if is_quit => self.ask_to_quit(),
            _ if is_ctrl(key, 'r') => self.start_run(),
            _ if is_ctrl(key, 's') => self.save(),
            _ if is_ctrl(key, 'o') => self.set_mode(Mode::Files),
            _ => {
                let ctx = &mut self.context;
                let _ = match self.mode {
                    Mode::Edit => self.editor.key(key, ctx),
                    Mode::Run => self.run.key(key, ctx),
                    Mode::Files => self.files.key(key, ctx),
                };
                self.files_request();
                self.run_request();
            }
        }
    }

    /// Ctrl-S: save under the name of today, or ask for a name in the files
    /// mode, and then go back to this mode.
    fn save(&mut self) {
        match self.context.document.file_name() {
            Some(name) => {
                let path = std::path::PathBuf::from(name);
                files::save_document(&path, &mut self.context);
            }
            None => {
                let back_to = self.mode;
                self.set_mode(Mode::Files);
                self.files.start_save_as(&self.context, Some(back_to));
            }
        }
    }

    /// Do what the files pane asks after a key: show an opened program in
    /// the editor, or go back to the mode of before a save.
    fn files_request(&mut self) {
        match self.files.take_request() {
            Some(files::Request::Opened) => {
                self.editor = editor::EditorPane::new(&self.context.document);
                self.set_mode(Mode::Edit);
            }
            Some(files::Request::Show(mode)) => self.set_mode(mode),
            None => {}
        }
    }

    /// Quit, or ask first if the program has changes that are not saved.
    fn ask_to_quit(&mut self) {
        if !self.context.document.is_changed() || self.quit_asked {
            self.quit = true;
        } else {
            self.quit_asked = true;
            self.context.message = QUIT_QUESTION.to_string();
        }
    }

    /// Ctrl-R: make the program from the text and run it in the run mode.
    /// If a line has a fault, show the fault and its line in the edit mode.
    fn start_run(&mut self) {
        let (program, marks) = self.context.document.to_program();
        if let Some(mark) = marks.first() {
            let (row, message) = (mark.index, mark.message.clone());
            self.set_mode(Mode::Edit);
            self.editor.go_to_line(&self.context.document, row);
            self.context.message = message;
            return;
        }
        self.set_mode(Mode::Run);
        self.context.program = program.clone();
        self.run.start(program, &mut self.context);
    }

    /// True if a program or a typed command runs in the run mode.
    pub fn is_running(&self) -> bool {
        self.run.is_running()
    }

    /// Take the text and the state messages of the run for a short time.
    /// The event loop calls it before each draw. Give true if a message came.
    pub fn tick(&mut self) -> bool {
        let changed = self.run.tick(&mut self.context);
        self.run_request();
        changed
    }

    /// Show the document line that the run mode asks for in the edit mode:
    /// the line of an error, or a fault of the text.
    fn run_request(&mut self) {
        if let Some(row) = self.run.take_edit_request() {
            self.set_mode(Mode::Edit);
            self.editor.go_to_line(&self.context.document, row);
        }
    }

    /// Take the messages of the run until it ends, for a maximum of `limit`.
    /// Give true if no program runs. For tests: no fixed wait.
    pub fn wait_for_run(&mut self, limit: Duration) -> bool {
        let ended = self.run.wait_for_run(&mut self.context, limit);
        self.run_request();
        ended
    }

    /// Stop the worker of the run mode and wait for its end, for a maximum
    /// of `limit`. Give true if it ended. The drop of the app also does this.
    pub fn stop_worker(&mut self, limit: Duration) -> bool {
        self.run.stop(limit)
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
        // The text screen before the first MODE has the size of the pane,
        // also for a run that starts in another mode (issue #200)
        self.run.see_pane(pane);
        let ctx = &self.context;
        // The help screen is in place of the pane, so that the cursor of
        // the pane does not show in it
        match (&self.help, self.mode) {
            (Some(help), _) => help.draw(frame, pane),
            (None, Mode::Edit) => self.editor.draw(frame, pane, ctx),
            (None, Mode::Run) => self.run.draw(frame, pane, ctx),
            (None, Mode::Files) => self.files.draw(frame, pane, ctx),
        }
        let mut detail = match self.mode {
            Mode::Edit => self.editor.status(&ctx.document),
            Mode::Run => self.run.status(),
            Mode::Files => String::new(),
        };
        // The run mode says running or ready. The other modes say running
        // while a program runs (issue #199).
        if self.mode != Mode::Run && self.run.is_running() {
            detail = [RUNNING, detail.as_str()].join("  ").trim_end().to_string();
        }
        frame::draw_status(
            frame,
            status,
            &frame::Status {
                mode: self.mode,
                file_name: ctx.document.file_name(),
                changed: ctx.document.is_changed(),
                message: &ctx.message,
                detail: &detail,
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
    crossterm::execute!(
        out,
        EnterAlternateScreen,
        Clear(ClearType::All),
        Hide,
        EnableBracketedPaste
    )
}

/// Write the commands that go back from the alternate screen.
pub fn leave_screen(out: &mut impl Write) -> io::Result<()> {
    crossterm::execute!(out, DisableBracketedPaste, LeaveAlternateScreen, Show)
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
        // A panic of the run worker does not stop the IDE: the run pane
        // shows its text in the status line.
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if std::thread::current().name() == Some(run::WORKER_NAME) {
                return;
            }
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

/// The event loop: take the messages of a run, draw, and read the events
/// that came, until the app quits. The wait for an event has a time limit,
/// so that the text of a run shows while no key comes.
fn run(mut app: App) -> io::Result<()> {
    let _guard = TerminalGuard::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    // Draw only after an event or a message of a run, so that an idle IDE
    // writes nothing to the terminal
    let mut changed = true;
    while !app.should_quit() {
        changed |= app.tick();
        if changed {
            // A resize event only draws the frame again at the new size.
            terminal.draw(|frame| app.draw(frame))?;
            changed = false;
        }
        if event::poll(EVENT_WAIT)? {
            app.event(event::read()?);
            changed = true;
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
    fn the_screen_commands_turn_the_bracketed_paste_on_and_off() {
        // A paste then comes as one event, not as keys: a tab or a newline
        // in it is text, not the Tab key or the Enter key.
        let mut out = Vec::new();
        enter_screen(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\x1b[?2004h"), "{text:?}");
        let mut out = Vec::new();
        leave_screen(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\x1b[?2004l"), "{text:?}");
    }

    #[test]
    fn a_new_document_from_another_mode_moves_the_cursor_into_the_text() {
        let mut app = App::new(None);
        app.paste("10 PRINT 1\n20 PRINT 2\n30 PRINT \\");
        assert_eq!(app.cursor(), (2, 10));
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        // The files mode loads a shorter text.
        app.context.document = Document::from_text(None, "10 PRINT \\");
        app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.cursor(), (0, 10));
        assert_eq!(app.marks().len(), 1);
        assert_eq!(app.marks()[0].index, 0);
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
    }

    #[test]
    fn a_paste_event_goes_to_the_editor() {
        let mut app = App::new(None);
        app.event(Event::Paste("10 END".to_string()));
        assert_eq!(app.document().lines(), ["10 END"]);
        app.event(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
        assert_eq!(app.mode(), Mode::Run);
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
