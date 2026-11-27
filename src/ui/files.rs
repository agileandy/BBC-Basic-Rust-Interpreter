//! The FILES pane: a list of one directory, with keys to open a program,
//! save it under a new name, start a new program and delete a file.
//!
//! The list shows `..` first, then the directories with a `/`, then the
//! program files with their size and date, each group in name order. Hidden
//! entries do not show. A question or the input line for a name uses the
//! last line of the pane.

use super::{Context, Document, Mode, Outcome};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The extensions of the program files that the list shows.
const PROGRAM_EXTENSIONS: [&str; 4] = ["bas", "bbas", "bbc", "txt"];
/// The extension that a name with no extension gets, as `SAVE` does.
const DEFAULT_EXTENSION: &str = "bbas";
/// The page for Page Up and Page Down before the first draw.
const DEFAULT_PAGE: usize = 20;
/// The keys that the last line shows when there is no question.
const HELP: &str = " Enter open  s save as  n new  d delete  ^S save  ^O files";
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// What the frame must do after a key of the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// The document is new: show it in the editor
    Opened,
    /// Go back to the mode of before the save
    Show(Mode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Parent,
    Directory,
    File {
        size: u64,
        modified: Option<SystemTime>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    name: String,
    kind: Kind,
}

/// The input line or the question on the last line of the pane.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Prompt {
    /// The name for a save as
    Name(String),
    /// The program has changes that are not saved: open this file?
    Open(PathBuf),
    /// The program has changes that are not saved: start a new program?
    New,
    Delete(PathBuf),
    Overwrite(PathBuf),
}

/// The state of the FILES pane.
#[derive(Debug, Default)]
pub struct FilesPane {
    dir: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    /// The first entry on the screen. The draw changes it, so that the
    /// selected entry is on the screen.
    top: Cell<usize>,
    /// The count of list rows at the last draw
    page: Cell<usize>,
    prompt: Option<Prompt>,
    /// The mode to go back to after a save that Ctrl-S started
    after_save: Option<Mode>,
    request: Option<Request>,
}

impl FilesPane {
    /// A pane for the directory of the file, or the working directory.
    pub fn new(file_name: Option<&str>) -> Self {
        let parent = file_name
            .and_then(|name| Path::new(name).parent())
            .filter(|parent| !parent.as_os_str().is_empty());
        let dir = parent
            .and_then(|parent| parent.canonicalize().ok())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            dir,
            ..Self::default()
        }
    }

    /// The pane gets the focus: read the directory again and remove a
    /// question of before.
    pub fn focus(&mut self, ctx: &mut Context) {
        self.prompt = None;
        self.after_save = None;
        self.refresh(ctx);
    }

    /// Open the input line for a save as, with the name of today. After the
    /// save, go back to `back_to` if it is given.
    pub fn start_save_as(&mut self, ctx: &Context, back_to: Option<Mode>) {
        let name = ctx
            .document
            .file_name()
            .map(|name| short_name(Path::new(name)))
            .unwrap_or_default();
        self.prompt = Some(Prompt::Name(name));
        self.after_save = back_to;
    }

    /// The request of the last key, if it has one.
    pub fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &mut Context) -> Outcome {
        match self.prompt.take() {
            Some(Prompt::Name(name)) => self.name_key(key, name, ctx),
            Some(question) => {
                // Each other key than y is no.
                if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                    self.answer_yes(question, ctx);
                } else {
                    self.end_save();
                }
            }
            None => return self.list_key(key, ctx),
        }
        Outcome::Used
    }

    /// A key with no question: move, or start an action.
    fn list_key(&mut self, key: KeyEvent, ctx: &mut Context) -> Outcome {
        let page = match self.page.get() {
            0 => DEFAULT_PAGE,
            page => page,
        };
        let last = self.entries.len().saturating_sub(1);
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(page),
            KeyCode::PageDown => self.selected = (self.selected + page).min(last),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = last,
            KeyCode::Enter => self.enter(ctx),
            KeyCode::Char('s' | 'S') if plain => self.start_save_as(ctx, None),
            KeyCode::Char('n' | 'N') if plain => {
                if ctx.document.is_changed() {
                    self.prompt = Some(Prompt::New);
                } else {
                    self.new_program(ctx);
                }
            }
            KeyCode::Char('d' | 'D') if plain => {
                if let Some((path, Kind::File { .. })) = self.selected_entry() {
                    self.prompt = Some(Prompt::Delete(path));
                }
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Used
    }

    /// A key in the input line for the name.
    fn name_key(&mut self, key: KeyEvent, mut name: String, ctx: &mut Context) {
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Enter => return self.submit_name(&name, ctx),
            KeyCode::Esc => return self.end_save(),
            KeyCode::Backspace => {
                name.pop();
            }
            KeyCode::Char(c) if plain && !c.is_control() => name.push(c),
            _ => {}
        }
        self.prompt = Some(Prompt::Name(name));
    }

    fn answer_yes(&mut self, question: Prompt, ctx: &mut Context) {
        match question {
            Prompt::Open(path) => self.open(&path, ctx),
            Prompt::New => self.new_program(ctx),
            Prompt::Delete(path) => self.delete(&path, ctx),
            Prompt::Overwrite(path) => self.save_as(&path, ctx),
            Prompt::Name(_) => {}
        }
    }

    /// Enter: go into a directory, or open a file.
    fn enter(&mut self, ctx: &mut Context) {
        let Some((path, kind)) = self.selected_entry() else {
            return;
        };
        match kind {
            Kind::Parent | Kind::Directory => self.change_dir(path, ctx),
            Kind::File { .. } if ctx.document.is_changed() => {
                self.prompt = Some(Prompt::Open(path));
            }
            Kind::File { .. } => self.open(&path, ctx),
        }
    }

    /// The path and the kind of the selected entry.
    fn selected_entry(&self) -> Option<(PathBuf, Kind)> {
        let entry = self.entries.get(self.selected)?;
        let path = match entry.kind {
            Kind::Parent => self.dir.parent()?.to_path_buf(),
            _ => self.dir.join(&entry.name),
        };
        Some((path, entry.kind.clone()))
    }

    /// Read the directory again. A fault keeps the list of before.
    fn refresh(&mut self, ctx: &mut Context) {
        match read_entries(&self.dir) {
            Ok(entries) => {
                self.entries = entries;
                self.selected = self.selected.min(self.entries.len().saturating_sub(1));
            }
            Err(message) => ctx.message = message,
        }
    }

    fn change_dir(&mut self, dir: PathBuf, ctx: &mut Context) {
        match read_entries(&dir) {
            Ok(entries) => {
                self.dir = dir;
                self.entries = entries;
                self.selected = 0;
                self.top.set(0);
            }
            Err(message) => ctx.message = message,
        }
    }

    fn open(&mut self, path: &Path, ctx: &mut Context) {
        match read_program(path) {
            Ok(document) => {
                ctx.document = document;
                self.request = Some(Request::Opened);
            }
            Err(message) => ctx.message = message,
        }
    }

    fn new_program(&mut self, ctx: &mut Context) {
        ctx.document = Document::new(None);
        self.request = Some(Request::Opened);
    }

    fn delete(&mut self, path: &Path, ctx: &mut Context) {
        let name = short_name(path);
        ctx.message = match std::fs::remove_file(path) {
            Ok(()) => format!("Deleted {}", name),
            Err(e) => format!("Cannot delete {}: {}", name, e),
        };
        self.refresh(ctx);
    }

    /// Check the typed name. Ask first if the file exists.
    fn submit_name(&mut self, name: &str, ctx: &mut Context) {
        match resolve_name(&self.dir, name) {
            Err(message) => {
                ctx.message = message;
                self.after_save = None;
            }
            Ok(path) if path.exists() => self.prompt = Some(Prompt::Overwrite(path)),
            Ok(path) => self.save_as(&path, ctx),
        }
    }

    /// Save under a new name. The document then has that name.
    fn save_as(&mut self, path: &Path, ctx: &mut Context) {
        if save_document(path, ctx) {
            ctx.document.set_file_name(Some(display_path(path)));
            self.refresh(ctx);
            self.end_save();
        } else {
            self.after_save = None;
        }
    }

    /// The end of a save as: go back to the mode of before, if Ctrl-S
    /// started it.
    fn end_save(&mut self) {
        if let Some(mode) = self.after_save.take() {
            self.request = Some(Request::Show(mode));
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &Context) {
        let plain = Style::new().fg(Color::White).bg(Color::Black);
        let cyan = Style::new().fg(Color::Cyan).bg(Color::Black);
        let yellow = Style::new().fg(Color::Yellow).bg(Color::Black);
        frame.render_widget(Paragraph::new("").style(plain), area);
        if area.height == 0 {
            return;
        }
        let row = |y: u16| Rect::new(area.x, area.y + y, area.width, 1);
        let header = format!(" {}", self.dir.display());
        frame.render_widget(Paragraph::new(header).style(cyan), row(0));

        // The rows between the header and the last line
        let height = usize::from(area.height.saturating_sub(2)).max(1);
        self.page.set(height);
        let mut top = self.top.get().min(self.selected);
        if self.selected >= top + height {
            top = self.selected + 1 - height;
        }
        self.top.set(top);
        let width = usize::from(area.width);
        let last = area.height - 1;
        for (offset, entry) in self.entries.iter().skip(top).take(height).enumerate() {
            let y = 1 + offset as u16;
            if y >= last {
                break;
            }
            let style = if top + offset == self.selected {
                plain.add_modifier(Modifier::REVERSED)
            } else {
                plain
            };
            frame.render_widget(
                Paragraph::new(entry_text(entry, width)).style(style),
                row(y),
            );
        }

        let (text, style) = match &self.prompt {
            None => (HELP.to_string(), cyan),
            Some(prompt) => (format!(" {}", prompt_text(prompt)), yellow),
        };
        let cursor = (text.chars().count() as u16).min(area.width.saturating_sub(1));
        frame.render_widget(Paragraph::new(text).style(style), row(last));
        if matches!(self.prompt, Some(Prompt::Name(_))) {
            frame.set_cursor_position((area.x + cursor, area.y + last));
        }
    }
}

/// The text of the input line or the question.
fn prompt_text(prompt: &Prompt) -> String {
    match prompt {
        Prompt::Name(name) => format!("Save as: {}", name),
        Prompt::Open(_) => "Not saved. Open? (y/n)".to_string(),
        Prompt::New => "Not saved. New? (y/n)".to_string(),
        Prompt::Delete(path) => format!("Delete {}? (y/n)", short_name(path)),
        Prompt::Overwrite(path) => format!("Overwrite {}? (y/n)", short_name(path)),
    }
}

/// One row of the list: the name, then the size and the date of a file.
fn entry_text(entry: &Entry, width: usize) -> String {
    let name = match entry.kind {
        Kind::Directory => format!("{}/", entry.name),
        _ => entry.name.clone(),
    };
    // A space, the name, a space, 7 for the size, 2 spaces and 6 for the date
    let name_width = width.saturating_sub(17).max(1);
    let name = cut(&name, name_width);
    match &entry.kind {
        Kind::File { size, modified } => {
            let date = modified.and_then(date_text).unwrap_or_default();
            format!(
                " {:<nw$} {:>7}  {:>6}",
                name,
                size_text(*size),
                date,
                nw = name_width
            )
        }
        _ => format!(" {}", name),
    }
}

/// Cut a long text to `width` characters, with a mark at the end.
fn cut(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut short: String = text.chars().take(width.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// The size in bytes, or in K from 1024 bytes.
fn size_text(size: u64) -> String {
    if size < 1024 {
        size.to_string()
    } else {
        format!("{}K", size.div_ceil(1024))
    }
}

/// The day and the month, such as `27 Nov`.
fn date_text(time: SystemTime) -> Option<String> {
    let (day, month) = day_and_month(time)?;
    Some(format!("{} {}", day, MONTHS.get(month)?))
}

/// The day of the month (from 1) and the month (from 0) in the local time.
#[cfg(unix)]
fn day_and_month(time: SystemTime) -> Option<(u32, usize)> {
    let secs = libc::time_t::try_from(time.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()?;
    // SAFETY: an all-zero tm is a valid value, and localtime_r writes only
    // to `tm`. Both pointers are valid for the call.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::localtime_r(&secs, &mut tm) };
    if result.is_null() {
        return None;
    }
    Some((
        u32::try_from(tm.tm_mday).ok()?,
        usize::try_from(tm.tm_mon).ok()?,
    ))
}

/// The day of the month (from 1) and the month (from 0) in UTC.
#[cfg(not(unix))]
fn day_and_month(time: SystemTime) -> Option<(u32, usize)> {
    let days = time.duration_since(UNIX_EPOCH).ok()?.as_secs() / 86_400;
    Some(utc_day_and_month(days))
}

/// The day and the month of a count of days from 1 January 1970, with the
/// civil calendar method of Howard Hinnant.
#[cfg_attr(unix, allow(dead_code))]
fn utc_day_and_month(days: u64) -> (u32, usize) {
    let z = days + 719_468;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 2 } else { mp - 10 } as usize;
    (day, month)
}

/// The entries of a directory: `..` if it has a parent, the directories and
/// the program files. An entry that cannot be read is left out.
fn read_entries(dir: &Path) -> Result<Vec<Entry>, String> {
    let read =
        std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {}", dir.display(), e))?;
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(metadata) = std::fs::metadata(entry.path()) else {
            continue;
        };
        if metadata.is_dir() {
            directories.push(Entry {
                name,
                kind: Kind::Directory,
            });
        } else if metadata.is_file() && is_program_file(&name) {
            let kind = Kind::File {
                size: metadata.len(),
                modified: metadata.modified().ok(),
            };
            files.push(Entry { name, kind });
        }
    }
    directories.sort_by(|a, b| a.name.cmp(&b.name));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let parent = dir.parent().map(|_| Entry {
        name: "..".to_string(),
        kind: Kind::Parent,
    });
    Ok(parent.into_iter().chain(directories).chain(files).collect())
}

fn is_program_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PROGRAM_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Read a file into a document. A line with a fault stays as text.
fn read_program(path: &Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", short_name(path), e))?;
    Ok(Document::from_text(Some(display_path(path)), &text))
}

/// The path for a typed name: in `dir` unless the name has a full path, and
/// with `.bbas` if it has no extension. An empty name and a directory are
/// refused.
fn resolve_name(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give a name".to_string());
    }
    let mut path = dir.join(name);
    if name.ends_with(std::path::is_separator) || path.is_dir() {
        return Err(format!("Cannot save {}: it is a directory", name));
    }
    if path.extension().is_none() {
        path.set_extension(DEFAULT_EXTENSION);
    }
    if path.is_dir() {
        return Err(format!(
            "Cannot save {}: it is a directory",
            short_name(&path)
        ));
    }
    Ok(path)
}

/// Save the text of the document to `path`, and clear the changed mark.
/// Give false, with the fault in the message, if the save fails.
pub fn save_document(path: &Path, ctx: &mut Context) -> bool {
    let name = short_name(path);
    match write_text(path, &ctx.document.text()) {
        Ok(()) => {
            ctx.document.set_changed(false);
            ctx.message = format!("Saved {}", name);
            true
        }
        Err(e) => {
            ctx.message = format!("Cannot save {}: {}", name, e);
            false
        }
    }
}

/// Write the text to a temporary file in the same directory, then rename it
/// over the target, so that a failed write does not damage the old file.
fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    if path.is_dir() {
        return Err(std::io::Error::other("it is a directory"));
    }
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = dir.join(format!(".{}.{}.tmp", short_name(path), std::process::id()));
    let result = std::fs::File::create(&temp)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// The last part of a path, for a message.
fn short_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The path for the status line: relative to the working directory if it is
/// in it, else the full path.
fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.canonicalize().ok())
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_text_gives_bytes_then_k() {
        assert_eq!(size_text(0), "0");
        assert_eq!(size_text(1023), "1023");
        assert_eq!(size_text(1024), "1K");
        assert_eq!(size_text(1025), "2K");
    }

    #[test]
    fn cut_puts_a_mark_at_the_end_of_a_long_name() {
        assert_eq!(cut("abc", 3), "abc");
        assert_eq!(cut("abcdef", 4), "abc…");
    }

    #[test]
    fn utc_day_and_month_gives_the_civil_date() {
        assert_eq!(utc_day_and_month(0), (1, 0));
        // 29 February 2024 is day 19782.
        assert_eq!(utc_day_and_month(19_782), (29, 1));
        // 31 December 2025 is day 20453.
        assert_eq!(utc_day_and_month(20_453), (31, 11));
    }

    #[test]
    fn program_files_are_the_4_extensions_in_any_case() {
        for name in ["a.bas", "a.BBAS", "a.bbc", "a.txt"] {
            assert!(is_program_file(name), "{name}");
        }
        for name in ["a.dat", "a", "bas"] {
            assert!(!is_program_file(name), "{name}");
        }
    }

    #[test]
    fn resolve_name_adds_the_extension_and_keeps_a_full_path() {
        let dir = Path::new("/base");
        assert_eq!(resolve_name(dir, "p"), Ok(PathBuf::from("/base/p.bbas")));
        assert_eq!(resolve_name(dir, "p.bas"), Ok(PathBuf::from("/base/p.bas")));
        assert_eq!(
            resolve_name(dir, "/other/p.bas"),
            Ok(PathBuf::from("/other/p.bas"))
        );
        assert_eq!(resolve_name(dir, "  "), Err("Give a name".to_string()));
    }
}
