//! The RUN pane (issue #171). Ctrl-R runs the program on a worker thread
//! with a large stack. The pane shows the text of the program on a text
//! screen, gives the typed keys to the program, and stops it at Escape.
//! When no program runs, the pane has the prompt `>` with the commands of
//! the prompt mode.
//!
//! The frame stays live during a run: the worker sends its text through a
//! channel, and the frame takes the messages at each tick. The worker also
//! sends the picture of the graphics canvas, and the pane shows it under
//! the text (issue #173, `src/ui/canvas.rs`).
//!
//! The editor text is the source. Each typed line runs with the program of
//! the editor text, and a typed line is refused while the text has a fault.

mod worker;

use super::canvas::{self, Picture};
use super::{theme, Context, Outcome};
use crate::executor::CursorMove;
use crate::graphics::ScreenMode;
use crate::program::ProgramStore;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use worker::{pack_size, Answer, Flags, FromWorker, Link, ToWorker, DELETE, ENTER};

/// The name of the worker thread. The panic hook of the IDE uses it.
pub(super) const WORKER_NAME: &str = "ide-interpreter";
/// The native stack size of the worker, as for the prompt mode (issue #151).
const STACK_SIZE: usize = 64 * 1024 * 1024;
/// The count of lines that the screen keeps for Page Up.
const HISTORY: usize = 1000;
/// The count of messages that wait for the pane before the worker waits.
const QUEUE: usize = 1024;
/// The longest time that one tick takes messages, so that the frame draws.
const TICK: Duration = Duration::from_millis(20);
/// The width and the height of the screen before the first draw.
const DEFAULT_WIDTH: usize = 80;
const DEFAULT_PAGE: usize = 20;
/// The status message when a typed command changed the program of the pane.
const CHANGED: &str = "The RUN program changed. The editor text is the source";
/// The status message for Ctrl-R during a run.
const RUNNING: &str = "A program runs. Press Escape to stop it";
/// The status message after EXIT, QUIT or a program that ran QUIT.
const QUIT: &str = "The program asked to quit: press Ctrl-Q to leave the IDE";

/// The key codes of a BBC Micro for the cursor keys.
const LEFT: char = '\u{88}';
const RIGHT: char = '\u{89}';
const DOWN: char = '\u{8a}';
const UP: char = '\u{8b}';

/// One character of the screen, with the text colours of its print.
#[derive(Debug, Clone, Copy)]
struct Glyph {
    c: char,
    style: Style,
}

/// One row of the screen.
#[derive(Debug, Default, Clone)]
struct Row {
    glyphs: Vec<Glyph>,
    /// True if the row continues the row above it: the text wrapped
    continued: bool,
}

/// The text screen: a grid of rows and columns with a cursor (issue #200).
/// A line feed goes to column 0 of the next row, a carriage return goes to
/// column 0, and a backspace goes back one column. A backspace at column 0
/// of a wrapped row goes to the last column of the row above it. A print
/// after the last column continues on the next row. A line feed at the last
/// row scrolls the grid up one row, and the top row goes to the history.
///
/// The pane and the worker each keep a screen, and they apply the same
/// changes in the same sequence. Thus VPOS on the worker gives the row that
/// the pane shows, and the pane shows the text in the sequence of the other
/// messages of the worker.
#[derive(Debug, Clone)]
struct Screen {
    /// The rows that scrolled off the top, for Page Up
    history: VecDeque<Row>,
    /// The rows of the grid, from the top
    grid: Vec<Row>,
    /// The count of columns of the grid
    width: usize,
    /// The cursor: the row of the grid and the column
    row: usize,
    col: usize,
    /// The text colours for the next print. The default style shows the
    /// colours of the pane.
    style: Style,
    /// The colours of a cell with no text: the text colours at the last
    /// clear, as CLS does on a BBC Micro
    paper: Style,
    /// The screen mode of the last MODE. None before the first MODE: the
    /// grid then has the size of the pane and 2 colours, as mode 0.
    mode: Option<ScreenMode>,
}

impl Default for Screen {
    fn default() -> Self {
        Self::new(DEFAULT_WIDTH, DEFAULT_PAGE)
    }
}

/// A change of the text screen. The worker applies it to its screen, then
/// sends it to the pane.
#[derive(Debug, Clone, PartialEq)]
enum ScreenChange {
    /// Text to print
    Text(String),
    /// Clear the screen (CLS)
    Clear,
    /// MODE selected this screen mode. A Clear comes after it.
    Mode(ScreenMode),
    /// The logical text colours: the foreground and the background
    Colours(u8, u8),
    /// A move of the cursor
    Move(CursorMove),
    /// The pane has this count of columns and rows
    Fit(usize, usize),
}

impl Screen {
    fn apply(&mut self, change: &ScreenChange) {
        match change {
            ScreenChange::Text(text) => self.write(text),
            ScreenChange::Clear => self.clear(),
            ScreenChange::Mode(mode) => self.set_mode(*mode),
            ScreenChange::Colours(foreground, background) => {
                self.set_colours(*foreground, *background)
            }
            ScreenChange::Move(movement) => self.move_cursor(*movement),
            ScreenChange::Fit(width, height) => self.fit_pane(*width, *height),
        }
    }

    /// An empty screen of `width` columns and `height` rows.
    fn new(width: usize, height: usize) -> Self {
        Self {
            history: VecDeque::new(),
            grid: vec![Row::default(); height.max(1)],
            width: width.max(1),
            row: 0,
            col: 0,
            style: Style::default(),
            paper: Style::default(),
            mode: None,
        }
    }

    fn write(&mut self, text: &str) {
        for c in text.chars() {
            match c {
                '\n' => self.next_row(false),
                '\r' => self.col = 0,
                '\x08' if self.col > 0 => self.col -= 1,
                '\x08' if self.grid[self.row].continued && self.row > 0 => {
                    self.row -= 1;
                    self.col = self.width - 1;
                }
                c if c.is_control() => {}
                c => {
                    // The cursor is in the grid after each change, but keep
                    // the print in the grid also if it is not
                    if self.col >= self.width {
                        self.next_row(true);
                    }
                    let col = self.col;
                    let glyph = Glyph {
                        c,
                        style: self.style,
                    };
                    let blank = Glyph {
                        c: ' ',
                        style: self.paper,
                    };
                    let glyphs = &mut self.grid[self.row].glyphs;
                    if col < glyphs.len() {
                        glyphs[col] = glyph;
                    } else {
                        glyphs.resize(col, blank);
                        glyphs.push(glyph);
                    }
                    // A print in the last column wraps at once, as on a BBC
                    // Micro: at the last row, the grid scrolls
                    self.col += 1;
                    if self.col >= self.width {
                        self.next_row(true);
                    }
                }
            }
        }
    }

    /// Go to column 0 of the next row. `continued` is true for a wrap.
    fn next_row(&mut self, continued: bool) {
        self.line_feed();
        self.grid[self.row].continued = continued;
        self.col = 0;
    }

    /// Go one row down, in the same column. At the last row, scroll the
    /// grid up one row: the top row goes to the history.
    fn line_feed(&mut self) {
        if self.row + 1 < self.grid.len() {
            self.row += 1;
            return;
        }
        let top = self.grid.remove(0);
        self.grid.push(Row::default());
        self.history.push_back(top);
        if self.history.len() > HISTORY {
            self.history.pop_front();
        }
    }

    /// Go one row up, in the same column. At the top row, scroll the grid
    /// down one row, as on a BBC Micro: the bottom row goes off the grid.
    fn line_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            return;
        }
        self.grid.pop();
        self.grid.insert(0, Row::default());
    }

    /// Move the cursor for VDU 31, TAB(x, y) and the cursor codes. A move
    /// to a place outside the grid does nothing, as on a BBC Micro. Left at
    /// column 0 goes to the last column of the row above, and right at the
    /// last column goes to the start of the next row. Each of them scrolls
    /// at the edge of the grid.
    fn move_cursor(&mut self, movement: CursorMove) {
        match movement {
            CursorMove::To(x, y) => {
                let (x, y) = (usize::from(x), usize::from(y));
                if x < self.width && y < self.grid.len() {
                    self.col = x;
                    self.row = y;
                }
            }
            CursorMove::Left if self.col > 0 => self.col -= 1,
            CursorMove::Left => {
                self.line_up();
                self.col = self.width - 1;
            }
            CursorMove::Right if self.col + 1 < self.width => self.col += 1,
            CursorMove::Right => {
                self.line_feed();
                self.col = 0;
            }
            CursorMove::Down => self.line_feed(),
            CursorMove::Up => self.line_up(),
            CursorMove::Home => {
                self.row = 0;
                self.col = 0;
            }
        }
    }

    /// Clear the grid and the history, and go to the top left.
    fn clear(&mut self) {
        self.history.clear();
        self.grid.fill(Row::default());
        self.row = 0;
        self.col = 0;
        self.paper = self.style;
    }

    /// MODE selected a screen mode: the grid gets its columns and rows. A
    /// clear comes after this.
    fn set_mode(&mut self, mode: ScreenMode) {
        self.mode = Some(mode);
        self.width = mode.text_columns.max(1);
        self.grid = vec![Row::default(); mode.text_rows.max(1)];
        self.row = 0;
        self.col = 0;
    }

    /// The logical text colours changed: show them with the palette of the
    /// mode.
    fn set_colours(&mut self, foreground: u8, background: u8) {
        let colours = self.mode.map_or(2, |mode| mode.colours);
        self.style = theme::screen(foreground, background, colours);
    }

    /// A new executor starts in mode 0 with the default colours. The grid
    /// of the last mode then gets the size of the pane, `width` by `height`.
    fn reset_mode(&mut self, width: usize, height: usize) {
        self.mode = None;
        self.style = Style::default();
        self.fit_pane(width, height);
    }

    /// Before the first MODE, the grid has the size of the pane. Thus the
    /// rows that TAB(x, y) can address then depend on the terminal: in a
    /// small terminal TAB(0, 25) is outside the grid and does nothing.
    ///
    /// A smaller pane removes the empty rows below the cursor, and moves the
    /// top rows to the history while the cursor is at the last row. A row
    /// with text below the cursor stays: the grid then has more rows than
    /// the pane, and the pane shows the rows to the cursor. A larger pane
    /// takes rows back from the history.
    fn fit_pane(&mut self, width: usize, height: usize) {
        if self.mode.is_some() || width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.col = self.col.min(width - 1);
        while self.grid.len() > height {
            let last = self.grid.len() - 1;
            if self.row == last {
                let top = self.grid.remove(0);
                self.history.push_back(top);
                self.row -= 1;
            } else if self.grid[last].glyphs.is_empty() {
                self.grid.pop();
            } else {
                break;
            }
        }
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
        while self.grid.len() < height {
            match self.history.pop_back() {
                Some(row) => {
                    self.grid.insert(0, row);
                    self.row += 1;
                }
                None => self.grid.push(Row::default()),
            }
        }
    }

    /// The index in the history and the grid of the top row that a pane of
    /// `height` rows shows with no scroll. A pane that has fewer rows than
    /// the grid shows the top of the grid, or the rows to the cursor if the
    /// cursor is below them. A pane that has more rows than the grid (before
    /// the fit to a new pane size) also shows the last rows of the history.
    fn view_start(&self, height: usize) -> usize {
        let total = self.history.len() + self.grid.len();
        if height >= self.grid.len() {
            return total.saturating_sub(height);
        }
        self.history.len() + (self.row + 1).saturating_sub(height)
    }

    /// True if the cursor is at column 0 of a row with no text: a prompt
    /// there does not write over text.
    fn at_free_line(&self) -> bool {
        self.col == 0
            && self.grid[self.row]
                .glyphs
                .iter()
                .all(|glyph| glyph.c == ' ')
    }

    /// The row at an index in the history and the grid.
    fn line(&self, index: usize) -> Option<&Row> {
        match index.checked_sub(self.history.len()) {
            Some(index) => self.grid.get(index),
            None => self.history.get(index),
        }
    }
}

/// The pane end of the worker.
struct Worker {
    input: Sender<ToWorker>,
    output: Receiver<FromWorker>,
    flags: Arc<Flags>,
    thread: JoinHandle<()>,
}

/// The state of the RUN pane.
#[derive(Default)]
pub struct RunPane {
    /// The text screen. The worker gives each change of it.
    screen: Screen,
    worker: Option<Worker>,
    /// Worker threads that a stop asked to end, which did not end in time
    stopping: Vec<JoinHandle<()>>,
    /// True from the Busy message to the Ready message of the worker
    busy: bool,
    /// True from a run request or an Enter at the prompt to the Busy message
    starting: bool,
    /// True from a run request (Ctrl-R) to its Busy message
    run_pending: bool,
    /// The line number of the error of the last run of the editor program
    error_line: Option<u16>,
    /// The document line for the edit mode to show
    edit_request: Option<usize>,
    /// The program of the editor text that the worker has
    synced: Option<ProgramStore>,
    /// The count of rows above the last page of the screen
    scroll: usize,
    /// The count of rows of the text screen at the last draw
    page: Cell<usize>,
    /// The count of columns that the last draw showed, if the pane was
    /// narrower than the screen of the mode
    cut: Cell<Option<usize>>,
    /// True if a message came after the last tick
    changed: bool,
    /// The picture of the graphics canvas, if the mode has graphics
    picture: Option<Picture>,
    /// The width and the height in cells of the picture area at the last
    /// draw
    picture_area: Cell<(u16, u16)>,
    /// The width and the height in cells of the pane at the last draw
    pane_size: Cell<(u16, u16)>,
}

impl RunPane {
    /// True if a program or a command runs.
    pub fn is_running(&self) -> bool {
        self.busy || self.starting
    }

    /// The text for the status line.
    pub fn status(&self) -> String {
        let state = if self.is_running() {
            "running"
        } else {
            "ready"
        };
        match (self.cut.get(), self.screen.mode) {
            (Some(shown), Some(mode)) => {
                format!("{}  left {} of {} columns", state, shown, mode.text_columns)
            }
            _ => state.to_string(),
        }
    }

    /// The document line to show in the edit mode: the line of an error
    /// after Enter on an empty prompt line, or the fault of the text.
    pub fn take_edit_request(&mut self) -> Option<usize> {
        self.edit_request.take()
    }

    /// The user comes to the run mode: start the worker, so that the prompt
    /// shows.
    pub fn open(&mut self, ctx: &mut Context) {
        self.start_worker(ctx);
    }

    /// Run the program (Ctrl-R). The frame made it from the document.
    pub fn start(&mut self, program: ProgramStore, ctx: &mut Context) {
        if self.is_running() {
            ctx.message = RUNNING.to_string();
            return;
        }
        if !self.start_worker(ctx) {
            return;
        }
        self.synced = Some(program.clone());
        self.clear_escape();
        self.send_size();
        self.send(ToWorker::Run(program));
        self.starting = true;
        self.run_pending = true;
        self.error_line = None;
        self.scroll = 0;
    }

    /// Start the worker thread if it does not run. Give false if the system
    /// refuses the thread.
    fn start_worker(&mut self, ctx: &mut Context) -> bool {
        if self.worker.is_some() {
            return true;
        }
        let (input, worker_input) = mpsc::channel();
        let (worker_output, output) = mpsc::sync_channel(QUEUE);
        let flags = Arc::new(Flags::default());
        flags.size.store(self.size_flag(), Ordering::Relaxed);
        flags.text.store(self.text_flag(), Ordering::Relaxed);
        // A new executor starts in mode 0 with the default colours. The
        // worker starts with a copy of the screen of the pane.
        let (width, height) = self.pane_size.get();
        self.screen
            .reset_mode(usize::from(width), usize::from(height));
        let link = Link::new(
            worker_input,
            worker_output,
            Arc::clone(&flags),
            self.screen.clone(),
        );
        let spawned = thread::Builder::new()
            .name(WORKER_NAME.to_string())
            .stack_size(STACK_SIZE)
            .spawn(move || worker::run_worker(link));
        match spawned {
            Ok(thread) => {
                self.worker = Some(Worker {
                    input,
                    output,
                    flags,
                    thread,
                });
                self.synced = None;
                // The canvas of a new executor is clear
                self.picture = None;
                true
            }
            Err(error) => {
                ctx.message = format!("No interpreter thread: {}", error);
                false
            }
        }
    }

    fn send(&mut self, message: ToWorker) {
        if let Some(worker) = &self.worker {
            let _ = worker.input.send(message);
        }
    }

    /// The size flag for the worker: the picture area of the last draw, or
    /// of the default pane before the first draw.
    fn size_flag(&self) -> u32 {
        let (columns, rows) = match self.picture_area.get() {
            (0, 0) => {
                let pane = Rect::new(0, 0, DEFAULT_WIDTH as u16, DEFAULT_PAGE as u16);
                let area = canvas::picture_area(pane);
                (area.width, area.height)
            }
            size => size,
        };
        pack_size(columns, rows)
    }

    /// The frame gives the area of the pane at each draw, in each mode.
    pub fn see_pane(&self, pane: Rect) {
        self.pane_size.set((pane.width, pane.height));
    }

    /// The text size flag for the worker: the pane at the last draw of the
    /// frame. Before the first draw it is 0, and the screen keeps its
    /// default size.
    fn text_flag(&self) -> u32 {
        let (columns, rows) = self.pane_size.get();
        pack_size(columns, rows)
    }

    /// If the picture area or the pane changed, give the new size to the
    /// worker, and wake it if it waits at the prompt: it sends a new
    /// picture, and a new size for the text screen before the first MODE.
    fn send_size(&self) {
        let size = self.size_flag();
        let text = self.text_flag();
        if let Some(worker) = &self.worker {
            let picture_changed = worker.flags.size.swap(size, Ordering::Relaxed) != size;
            let text_changed = worker.flags.text.swap(text, Ordering::Relaxed) != text;
            if picture_changed || text_changed {
                let _ = worker.input.send(ToWorker::Wake);
            }
        }
    }

    /// A new command starts: an Escape before this belongs to the command
    /// before it. The worker starts a command only after this go-ahead.
    fn clear_escape(&self) {
        if let Some(worker) = &self.worker {
            worker.flags.escape.store(false, Ordering::SeqCst);
        }
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &mut Context) -> Outcome {
        let idle = !self.is_running();
        match key.code {
            KeyCode::PageUp if idle => {
                let max = self.screen.view_start(self.page());
                self.scroll = (self.scroll + self.page()).min(max);
                return Outcome::Used;
            }
            KeyCode::PageDown if idle => {
                self.scroll = self.scroll.saturating_sub(self.page());
                return Outcome::Used;
            }
            KeyCode::Esc => {
                if let (false, Some(worker)) = (idle, &self.worker) {
                    worker.flags.escape.store(true, Ordering::SeqCst);
                    let _ = worker.input.send(ToWorker::Wake);
                }
                return Outcome::Used;
            }
            _ => {}
        }
        let Some(code) = key_code(key) else {
            return Outcome::Ignored;
        };
        if !self.start_worker(ctx) {
            return Outcome::Used;
        }
        if idle && code == ENTER {
            // The worker sends the typed line for an answer
            self.starting = true;
        }
        self.send(ToWorker::Key(code));
        self.scroll = 0;
        Outcome::Used
    }

    /// The answer for a typed line of the worker.
    ///
    /// - During a run request, skip the line: the run comes first.
    /// - An empty line after an error of the editor program goes to the line
    ///   of the error in the edit mode.
    /// - While the editor text has a fault, refuse each line that is not
    ///   empty: each statement can call a PROC or FN of the program, so no
    ///   typed line can run with a part of the program. Show the fault in
    ///   the edit mode, as Ctrl-R does.
    /// - If not, run the line with the program of the editor text.
    fn answer(&mut self, line: &str, ctx: &mut Context) -> Answer {
        if self.run_pending {
            return Answer::Skip;
        }
        let error_line = self.error_line.take();
        let doc = &ctx.document;
        if line.trim().is_empty() {
            let row = error_line.and_then(|number| {
                (0..doc.lines().len()).find(|&r| doc.line_number(r) == Some(number))
            });
            if let Some(row) = row {
                self.edit_request = Some(row);
                return Answer::Skip;
            }
            return Answer::Run(None);
        }
        let (program, marks) = doc.to_program();
        if let Some(mark) = marks.first() {
            self.edit_request = Some(mark.index);
            ctx.message = mark.message.clone();
            return Answer::Skip;
        }
        let same = self.synced.as_ref().is_some_and(|synced| {
            // The file gives the first search place of INSTALL (issue #186)
            synced.list() == program.list() && synced.file() == program.file()
        });
        if same {
            return Answer::Run(None);
        }
        self.synced = Some(program.clone());
        Answer::Run(Some(program))
    }

    /// Take the messages of the worker for a short time, so that the frame
    /// can draw and read the keys. Give true if a message came.
    pub fn tick(&mut self, ctx: &mut Context) -> bool {
        self.take_messages(ctx, Instant::now() + TICK, false);
        std::mem::take(&mut self.changed)
    }

    /// Take the messages of the worker until the run ends, for a maximum of
    /// `limit`. Give true if no program runs.
    pub fn wait_for_run(&mut self, ctx: &mut Context, limit: Duration) -> bool {
        self.take_messages(ctx, Instant::now() + limit, true)
    }

    /// Take each message that waits. Then, with `until_ready`, wait for more
    /// messages while a program runs. Give true if no program runs.
    fn take_messages(&mut self, ctx: &mut Context, deadline: Instant, until_ready: bool) -> bool {
        self.send_size();
        loop {
            let Some(worker) = &self.worker else {
                return !self.is_running();
            };
            let message = match worker.output.try_recv() {
                Ok(message) => Ok(message),
                Err(TryRecvError::Empty) if until_ready && self.is_running() => worker
                    .output
                    .recv_timeout(deadline.saturating_duration_since(Instant::now())),
                Err(TryRecvError::Empty) => return !self.is_running(),
                Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
            };
            match message {
                Ok(message) => self.apply(message, ctx),
                Err(RecvTimeoutError::Timeout) => return !self.is_running(),
                Err(RecvTimeoutError::Disconnected) => {
                    self.worker_ended(ctx);
                    return true;
                }
            }
            if Instant::now() >= deadline {
                return !self.is_running();
            }
        }
    }

    fn apply(&mut self, message: FromWorker, ctx: &mut Context) {
        self.changed = true;
        match message {
            FromWorker::Screen(change) => {
                if matches!(change, ScreenChange::Mode(_)) {
                    self.scroll = 0;
                }
                self.screen.apply(&change);
            }
            FromWorker::Picture(picture) => self.picture = picture,
            FromWorker::Line(line) => {
                let answer = self.answer(&line, ctx);
                match answer {
                    Answer::Run(_) => {
                        self.clear_escape();
                        self.starting = true;
                    }
                    Answer::Skip => self.starting = self.run_pending,
                }
                self.send(ToWorker::Answer(answer));
            }
            FromWorker::Busy => {
                self.busy = true;
                self.starting = false;
                self.run_pending = false;
            }
            FromWorker::Ready {
                error_line,
                changed,
                quit,
            } => {
                self.busy = false;
                self.error_line = error_line;
                if changed {
                    // The worker program is not the editor program now: the
                    // next typed line gets the editor program again
                    self.synced = None;
                    ctx.message = CHANGED.to_string();
                }
                if quit {
                    ctx.message = QUIT.to_string();
                }
            }
        }
    }

    /// The state of a run that no worker runs now.
    fn reset_run(&mut self) {
        self.busy = false;
        self.starting = false;
        self.run_pending = false;
        self.error_line = None;
        self.edit_request = None;
        self.synced = None;
    }

    /// The worker thread ended with no request: the interpreter panicked.
    /// The next run starts a new worker, with its prompt on a new line.
    fn worker_ended(&mut self, ctx: &mut Context) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        let panic = worker.thread.join().err();
        let detail = panic
            .as_ref()
            .and_then(|payload| {
                payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
            })
            .unwrap_or("no detail");
        ctx.message = format!("The interpreter stopped: {}", detail);
        self.reset_run();
        if !self.screen.at_free_line() {
            self.screen.write("\n");
        }
        self.changed = true;
    }

    /// Stop the worker and wait for its end, for a maximum of `limit`. Give
    /// true if it ended. A program stops at its next statement, also if it
    /// traps Escape. A worker that does not end in time stays in a list, and
    /// the next stop waits for it again.
    pub fn stop(&mut self, limit: Duration) -> bool {
        if let Some(worker) = self.worker.take() {
            worker.flags.quit.store(true, Ordering::SeqCst);
            let _ = worker.input.send(ToWorker::Wake);
            // The worker cannot wait to send text or to read a key now
            drop(worker.input);
            drop(worker.output);
            self.stopping.push(worker.thread);
        }
        self.reset_run();
        let deadline = Instant::now() + limit;
        while self.stopping.iter().any(|thread| !thread.is_finished()) {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        for thread in self.stopping.drain(..) {
            let _ = thread.join();
        }
        true
    }

    /// The part of the pane for the text screen. A pane that is wider than
    /// the screen of the mode has the screen in the centre, with no border
    /// (issue #203). A pane that is narrower shows the left part, and the
    /// status line says so. A pane that is higher than the screen of the
    /// mode has the screen at the top.
    fn screen_area(&self, area: Rect, mode: Option<ScreenMode>) -> Rect {
        self.cut.set(None);
        let Some(mode) = mode else {
            return area;
        };
        let columns = u16::try_from(mode.text_columns).unwrap_or(u16::MAX);
        let rows = u16::try_from(mode.text_rows).map_or(area.height, |rows| rows.min(area.height));
        if columns > area.width {
            self.cut.set(Some(usize::from(area.width)));
            return Rect::new(area.x, area.y, area.width, rows);
        }
        let x = area.x + (area.width - columns) / 2;
        Rect::new(x, area.y, columns, rows)
    }

    fn page(&self) -> usize {
        match self.page.get() {
            0 => DEFAULT_PAGE,
            page => page,
        }
    }

    /// Draw the text screen, and the picture of the canvas under the text:
    /// a text cell that is not a space shows above the picture.
    pub fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &Context) {
        let screen = &self.screen;
        frame.render_widget(Paragraph::new("").style(theme::text()), area);
        let pane = area;
        let area = self.screen_area(pane, screen.mode);
        // The picture and the text screen of the mode share one rectangle,
        // if the picture stays readable (issue #200)
        let picture_area = match screen.mode {
            Some(mode) => canvas::text_picture_area(pane, area, mode.text_columns, mode.text_rows),
            None => canvas::picture_area(pane),
        };
        self.picture_area
            .set((picture_area.width, picture_area.height));
        let height = usize::from(area.height);
        self.page.set(height);
        let top = screen.view_start(height);
        let start = top.saturating_sub(self.scroll);
        let rows: Vec<Line> = (start..start + height)
            .map_while(|index| screen.line(index))
            .map(|row| {
                let spans: Vec<Span> = row
                    .glyphs
                    .iter()
                    .map(|glyph| Span::styled(glyph.c.to_string(), glyph.style))
                    .collect();
                Line::from(spans)
            })
            .collect();
        let paper = theme::text().patch(screen.paper);
        frame.render_widget(Paragraph::new(rows).style(paper), area);
        if let Some(picture) = &self.picture {
            canvas::draw(frame.buffer_mut(), picture_area, picture, area);
        }
        let cursor = screen.history.len() + screen.row;
        let on_page = (start..start + height).contains(&cursor);
        if start == top && on_page && area.width > 0 {
            let x = screen.col.min(usize::from(area.width) - 1);
            let y = cursor - start;
            let position = Position::new(area.x + x as u16, area.y + y as u16);
            if let Some(cell) = frame.buffer_mut().cell_mut(position) {
                cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
            }
        }
    }
}

impl Drop for RunPane {
    fn drop(&mut self) {
        self.stop(Duration::from_secs(2));
    }
}

/// The key code that a key gives to the program, if it gives one.
fn key_code(key: KeyEvent) -> Option<char> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    Some(match key.code {
        KeyCode::Char(c) => c,
        KeyCode::Enter => ENTER,
        KeyCode::Backspace => DELETE,
        KeyCode::Left => LEFT,
        KeyCode::Right => RIGHT,
        KeyCode::Down => DOWN,
        KeyCode::Up => UP,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    use crate::ui::Document;

    const LIMIT: Duration = Duration::from_secs(5);

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn context(text: &str) -> Context {
        let document = Document::from_text(None, text);
        let program = document.to_program().0;
        Context {
            document,
            program,
            executor: Executor::new(),
            message: String::new(),
        }
    }

    /// The rows of the history and the grid, with no empty rows at the end.
    fn text(screen: &Screen) -> Vec<String> {
        let mut rows: Vec<String> = screen
            .history
            .iter()
            .chain(&screen.grid)
            .map(|row| {
                row.glyphs
                    .iter()
                    .map(|glyph| glyph.c)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows
    }

    /// A worker whose thread runs `run`. The channels of the worker end
    /// are closed.
    fn fake_worker(run: impl FnOnce() + Send + 'static) -> Worker {
        let (input, _) = mpsc::channel();
        let (_, output) = mpsc::sync_channel(1);
        Worker {
            input,
            output,
            flags: Arc::new(Flags::default()),
            thread: thread::spawn(run),
        }
    }

    #[test]
    fn the_screen_wraps_at_the_width_and_keeps_1000_lines() {
        let mut screen = Screen::new(3, 20);
        screen.write("abcde");
        assert_eq!(text(&screen), ["abc", "de"]);
        for _ in 0..HISTORY + 25 {
            screen.write("x\n");
        }
        // The grid keeps 20 rows, and the history keeps the 1000 rows above
        assert_eq!(screen.history.len(), HISTORY);
        assert_eq!(screen.grid.len(), 20);
        assert_eq!(screen.row, 19);
    }

    #[test]
    fn a_carriage_return_and_a_backspace_move_the_cursor_on_the_line() {
        let mut screen = Screen::new(80, 20);
        screen.write("abc\rX\x08Y");
        assert_eq!(text(&screen), ["Ybc"]);
    }

    #[test]
    fn a_backspace_at_column_0_of_a_wrapped_row_goes_to_the_row_above() {
        let mut screen = Screen::new(3, 20);
        screen.write("abcde");
        screen.write("\x08 \x08\x08 \x08\x08 \x08");
        // 3 backspaces remove e, d and c
        assert_eq!(text(&screen), ["ab"]);
        assert_eq!((screen.row, screen.col), (0, 2));
        // No row above a row that did not wrap
        screen.write("\n\x08Z");
        assert_eq!(text(&screen), ["ab", "Z"]);
    }

    #[test]
    fn before_the_first_mode_the_grid_follows_the_height_of_the_pane() {
        let mut screen = Screen::new(80, 4);
        screen.write("a\nb\nc\nd\ne");
        // The grid has 4 rows: "a" is in the history
        assert_eq!((screen.history.len(), screen.row), (1, 3));
        // A smaller pane moves the top rows to the history
        screen.fit_pane(80, 2);
        assert_eq!((screen.history.len(), screen.row), (3, 1));
        // A larger pane takes the rows back
        screen.fit_pane(80, 6);
        assert_eq!((screen.history.len(), screen.row), (0, 4));
        assert_eq!(text(&screen), ["a", "b", "c", "d", "e"]);
        // After a MODE, the grid has the rows of the mode
        screen.set_mode(ScreenMode::from_number(7).expect("mode 7"));
        screen.fit_pane(80, 6);
        assert_eq!((screen.width, screen.grid.len()), (40, 25));
    }

    #[test]
    fn a_smaller_pane_keeps_the_text_below_the_cursor() {
        // 30 lines in a pane of 38 rows, then VDU 30: the cursor is at the
        // top, with text below it
        let mut screen = Screen::new(80, 38);
        for n in 0..30 {
            screen.write(&format!("{n}\n"));
        }
        screen.move_cursor(CursorMove::Home);
        screen.fit_pane(80, 10);
        screen.fit_pane(80, 38);
        let lines: Vec<String> = (0..30).map(|n| n.to_string()).collect();
        assert_eq!(text(&screen), lines);
        assert_eq!((screen.row, screen.col), (0, 0));
    }

    #[test]
    fn a_new_worker_fits_the_grid_of_the_last_mode_to_the_pane() {
        let mut ctx = context("10 END");
        let mut pane = RunPane::default();
        pane.screen
            .set_mode(ScreenMode::from_number(1).expect("mode 1"));
        pane.see_pane(Rect::new(0, 1, 80, 22));
        assert!(pane.start_worker(&mut ctx));
        assert_eq!(pane.screen.mode, None);
        assert_eq!((pane.screen.width, pane.screen.grid.len()), (80, 22));
        assert!(pane.stop(LIMIT));
    }

    #[test]
    fn an_escape_before_a_typed_line_starts_does_not_stop_it() {
        // K2: the first run ends, the line typed during it waits for its
        // answer, and then Escape comes, before the pane sees the end
        let program_text = "10 FOR I% = 1 TO 20000: NEXT\n20 PRINT \"A\"";
        let mut ctx = context(program_text);
        let mut pane = RunPane::default();
        pane.start(ctx.program.clone(), &mut ctx);
        for c in "RUN".chars() {
            pane.key(press(KeyCode::Char(c)), &mut ctx);
        }
        pane.key(press(KeyCode::Enter), &mut ctx);
        let mut waiting = Vec::new();
        let deadline = Instant::now() + LIMIT;
        loop {
            let output = &pane.worker.as_ref().expect("a worker").output;
            let message = output.recv_timeout(LIMIT).expect("a message");
            let is_line = matches!(message, FromWorker::Line(_));
            waiting.push(message);
            if is_line {
                break;
            }
            assert!(Instant::now() < deadline);
        }
        pane.key(press(KeyCode::Esc), &mut ctx);
        for message in waiting {
            pane.apply(message, &mut ctx);
        }
        assert!(pane.wait_for_run(&mut ctx, LIMIT));
        let rows = text(&pane.screen);
        assert_eq!(
            rows,
            [
                "BBC Computer 32K",
                "",
                "BASIC",
                "",
                "> RUN",
                "A",
                "> RUN",
                "A",
                ">"
            ]
        );
    }

    #[test]
    fn a_worker_that_ended_leaves_no_state_of_its_run() {
        // K7
        let mut ctx = context("10 END");
        let mut pane = RunPane::default();
        pane.worker = Some(fake_worker(|| {}));
        pane.busy = true;
        pane.error_line = Some(10);
        pane.screen.write("> PRI");
        assert!(pane.wait_for_run(&mut ctx, LIMIT));
        assert!(!pane.is_running());
        assert_eq!(pane.error_line, None);
        assert_eq!(pane.screen.col, 0);
        assert!(
            ctx.message.starts_with("The interpreter stopped"),
            "{}",
            ctx.message
        );
    }

    #[test]
    fn a_stop_that_times_out_leaves_no_run_and_waits_again_later() {
        // K8
        let (release, wait) = mpsc::channel::<()>();
        let mut pane = RunPane::default();
        pane.worker = Some(fake_worker(move || {
            let _ = wait.recv();
        }));
        pane.busy = true;
        pane.starting = true;
        assert!(!pane.stop(Duration::from_millis(10)));
        assert!(!pane.is_running());
        drop(release);
        assert!(pane.stop(LIMIT));
        assert!(pane.stopping.is_empty());
    }
}
