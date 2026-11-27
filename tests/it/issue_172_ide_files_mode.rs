//! Issue #172: the files mode of the IDE. A list of the current directory,
//! with keys to open, save, save as, start a new program and delete a file.
//! Ctrl-S saves from each mode and Ctrl-O changes to the FILES tab.
//! Each test has its own temporary directory, and the app starts with a file
//! name in it, so that the list starts in that directory.

use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use std::path::{Path, PathBuf};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(letter: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.key(press(KeyCode::Char(c)));
    }
}

fn keys(app: &mut App, code: KeyCode, count: usize) {
    for _ in 0..count {
        app.key(press(code));
    }
}

/// A new empty directory for one test.
fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("issue_172_{}_{}", std::process::id(), name));
    if dir.exists() {
        make_writable(&dir);
        std::fs::remove_dir_all(&dir).expect("old test directory is removed");
    }
    std::fs::create_dir(&dir).expect("test directory is made");
    dir.canonicalize().expect("test directory has a path")
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("mode is set");
}

#[cfg(unix)]
fn make_writable(dir: &Path) {
    set_mode(dir, 0o755);
}

#[cfg(not(unix))]
fn make_writable(_dir: &Path) {}

/// The directory of the tests: `..`, then `sub/`, `a.bbas` and `b.bas`.
/// A hidden file and a file of another type do not show.
fn sample_dir(name: &str) -> PathBuf {
    let dir = fresh_dir(name);
    std::fs::create_dir(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub").join("inner.bbas"), "10 END\n").unwrap();
    std::fs::write(dir.join("b.bas"), "10 PRINT \"B\"\n").unwrap();
    std::fs::write(dir.join("a.bbas"), "10 PRINT \"A\"\n20 END\n").unwrap();
    std::fs::write(dir.join(".hidden.bbas"), "10 END\n").unwrap();
    std::fs::write(dir.join("notes.dat"), "x").unwrap();
    dir
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// An app with a new program in `dir`, in the FILES mode.
fn app_in(dir: &Path) -> App {
    let mut app = App::new(Some(text(&dir.join("start.bbas"))));
    app.key(ctrl('o'));
    assert_eq!(app.mode(), Mode::Files);
    app
}

fn draw(app: &App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

fn rows(app: &App) -> Vec<String> {
    let buffer = draw(app);
    (0..buffer.area.height)
        .map(|row| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, row)].symbol())
                .collect::<String>()
        })
        .collect()
}

/// The first word of each row that shows a name of the list.
fn listed_names(app: &App) -> Vec<String> {
    let rows = rows(app);
    let names = [
        "..",
        "sub/",
        "a.bbas",
        "b.bas",
        "inner.bbas",
        "c.bbas",
        "out.bas",
        "prog.bbas",
    ];
    rows.iter()
        .filter_map(|row| row.split_whitespace().next())
        .filter(|word| names.contains(word))
        .map(str::to_string)
        .collect()
}

/// The text of the screen, with each row on its own line.
fn screen(app: &App) -> String {
    rows(app).join("\n")
}

#[test]
fn the_list_shows_the_parent_then_the_directories_then_the_program_files() {
    let dir = sample_dir("list");
    let app = app_in(&dir);
    assert_eq!(listed_names(&app), ["..", "sub/", "a.bbas", "b.bas"]);
    let screen = screen(&app);
    assert!(!screen.contains("hidden"), "{screen}");
    assert!(!screen.contains("notes.dat"), "{screen}");
}

#[test]
fn a_program_file_shows_its_size_and_its_date() {
    let dir = sample_dir("size");
    let app = app_in(&dir);
    let row = rows(&app)
        .into_iter()
        .find(|row| row.trim_start().starts_with("a.bbas"))
        .expect("a row for a.bbas");
    // "10 PRINT \"A\"\n20 END\n" is 20 bytes.
    assert!(row.contains(" 20 "), "{row:?}");
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    assert!(months.iter().any(|m| row.contains(m)), "{row:?}");
}

#[test]
fn enter_on_a_directory_goes_in_and_enter_on_the_parent_goes_back() {
    let dir = sample_dir("enter_dir");
    let mut app = app_in(&dir);
    app.key(press(KeyCode::Down));
    app.key(press(KeyCode::Enter));
    assert_eq!(listed_names(&app), ["..", "inner.bbas"]);
    assert_eq!(app.mode(), Mode::Files);
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::Enter));
    assert_eq!(listed_names(&app), ["..", "sub/", "a.bbas", "b.bas"]);
}

#[test]
fn enter_on_a_file_loads_its_text_into_the_editor() {
    let dir = sample_dir("open");
    let mut app = app_in(&dir);
    keys(&mut app, KeyCode::Down, 2);
    app.key(press(KeyCode::Enter));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.document().lines(), ["10 PRINT \"A\"", "20 END"]);
    assert_eq!(
        app.document().file_name(),
        Some(text(&dir.join("a.bbas")).as_str())
    );
    assert!(!app.document().is_changed());
    assert_eq!(app.cursor(), (0, 0));
}

#[test]
fn a_file_with_a_fault_opens_as_text_with_its_marks() {
    let dir = fresh_dir("open_fault");
    std::fs::write(dir.join("bad.bbas"), "10 PRINT \\\n20 END\n").unwrap();
    let mut app = app_in(&dir);
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Enter));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.document().lines(), ["10 PRINT \\", "20 END"]);
    assert_eq!(app.marks().len(), 1);
    assert_eq!(
        app.marks()[0].message,
        "Syntax error at line 10: Unknown character '\\'"
    );
}

#[test]
fn open_with_changes_asks_first_and_only_y_opens() {
    let dir = sample_dir("open_ask");
    let mut app = App::new(Some(text(&dir.join("start.bbas"))));
    type_text(&mut app, "10 END");
    app.key(ctrl('o'));
    keys(&mut app, KeyCode::Down, 2);
    app.key(press(KeyCode::Enter));
    assert!(screen(&app).contains("Not saved. Open? (y/n)"));
    app.key(press(KeyCode::Char('n')));
    assert_eq!(app.mode(), Mode::Files);
    assert_eq!(app.document().lines(), ["10 END"]);
    assert!(!screen(&app).contains("Not saved. Open?"));
    app.key(press(KeyCode::Enter));
    app.key(press(KeyCode::Char('y')));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.document().lines(), ["10 PRINT \"A\"", "20 END"]);
}

#[test]
fn save_as_writes_the_editor_text_and_clears_the_changed_mark() {
    let dir = fresh_dir("save_as");
    let mut app = app_in(&dir);
    app.set_changed(true);
    type_text(&mut app, "s");
    // The start value is the name of today.
    assert!(screen(&app).contains("Save as: start.bbas"));
    keys(&mut app, KeyCode::Backspace, "start.bbas".len());
    // A name with no extension gets .bbas, as SAVE does.
    type_text(&mut app, "prog");
    app.key(press(KeyCode::Enter));
    let saved = dir.join("prog.bbas");
    assert_eq!(std::fs::read_to_string(&saved).unwrap(), "\n");
    assert_eq!(app.message(), "Saved prog.bbas");
    assert_eq!(app.document().file_name(), Some(text(&saved).as_str()));
    assert!(!app.document().is_changed());
    assert_eq!(app.mode(), Mode::Files);
    assert_eq!(listed_names(&app), ["..", "prog.bbas"]);
}

#[test]
fn save_as_of_typed_text_gives_the_same_text_in_the_file() {
    let dir = fresh_dir("save_as_text");
    let mut app = App::new(Some(text(&dir.join("start.bbas"))));
    type_text(&mut app, "10 PRINT 1");
    app.key(press(KeyCode::Enter));
    type_text(&mut app, "END");
    assert!(app.document().is_changed());
    app.key(ctrl('o'));
    type_text(&mut app, "s");
    keys(&mut app, KeyCode::Backspace, "start.bbas".len());
    type_text(&mut app, "out.bas");
    app.key(press(KeyCode::Enter));
    let content = std::fs::read_to_string(dir.join("out.bas")).unwrap();
    assert_eq!(content, "10 PRINT 1\n20 END\n");
    assert_eq!(content, app.document().text());
    assert!(!app.document().is_changed());
    assert_eq!(listed_names(&app), ["..", "out.bas"]);
}

#[test]
fn save_as_to_a_file_that_exists_asks_first() {
    let dir = fresh_dir("overwrite");
    std::fs::write(dir.join("c.bbas"), "10 OLD\n").unwrap();
    let mut app = App::new(Some(text(&dir.join("start.bbas"))));
    type_text(&mut app, "10 NEW");
    app.key(ctrl('o'));
    type_text(&mut app, "s");
    keys(&mut app, KeyCode::Backspace, "start.bbas".len());
    type_text(&mut app, "c");
    app.key(press(KeyCode::Enter));
    assert!(screen(&app).contains("Overwrite c.bbas? (y/n)"));
    app.key(press(KeyCode::Esc));
    assert_eq!(
        std::fs::read_to_string(dir.join("c.bbas")).unwrap(),
        "10 OLD\n"
    );
    assert!(app.document().is_changed());
    type_text(&mut app, "s");
    keys(&mut app, KeyCode::Backspace, "start.bbas".len());
    type_text(&mut app, "c");
    app.key(press(KeyCode::Enter));
    app.key(press(KeyCode::Char('y')));
    assert_eq!(
        std::fs::read_to_string(dir.join("c.bbas")).unwrap(),
        "10 NEW\n"
    );
    assert_eq!(app.message(), "Saved c.bbas");
}

#[test]
fn save_as_refuses_an_empty_name_and_a_directory() {
    let dir = sample_dir("save_as_bad");
    let mut app = app_in(&dir);
    type_text(&mut app, "s");
    keys(&mut app, KeyCode::Backspace, "start.bbas".len());
    app.key(press(KeyCode::Enter));
    assert_eq!(app.message(), "Give a name");
    type_text(&mut app, "s");
    keys(&mut app, KeyCode::Backspace, "start.bbas".len());
    type_text(&mut app, "sub/");
    app.key(press(KeyCode::Enter));
    assert!(
        app.message().starts_with("Cannot save sub/"),
        "{}",
        app.message()
    );
    assert!(dir.join("sub").is_dir());
    assert_eq!(
        app.document().file_name(),
        Some(text(&dir.join("start.bbas")).as_str())
    );
}

#[test]
fn ctrl_s_with_a_name_saves_with_no_question_and_stays_in_the_mode() {
    let dir = fresh_dir("ctrl_s");
    let name = dir.join("prog.bbas");
    let mut app = App::new(Some(text(&name)));
    type_text(&mut app, "10 PRINT 2");
    app.key(ctrl('s'));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.message(), "Saved prog.bbas");
    assert_eq!(std::fs::read_to_string(&name).unwrap(), "10 PRINT 2\n");
    assert!(!app.document().is_changed());
    assert!(!screen(&app).contains("(y/n)"));
}

#[test]
fn ctrl_s_with_no_name_asks_for_a_name_in_the_files_mode() {
    let mut app = App::new(None);
    type_text(&mut app, "10 END");
    app.key(ctrl('s'));
    assert_eq!(app.mode(), Mode::Files);
    assert!(screen(&app).contains("Save as: "));
    app.key(press(KeyCode::Esc));
    assert!(!screen(&app).contains("Save as: "));
    assert!(app.document().is_changed());
}

#[test]
fn a_save_that_fails_keeps_the_old_file() {
    let dir = fresh_dir("save_fails");
    let name = dir.join("keep.bbas");
    std::fs::write(&name, "10 OLD\n").unwrap();
    let mut app = App::new(Some(text(&name)));
    type_text(&mut app, "X");
    #[cfg(unix)]
    set_mode(&dir, 0o555);
    app.key(ctrl('s'));
    #[cfg(unix)]
    set_mode(&dir, 0o755);
    if cfg!(unix) {
        assert!(
            app.message().starts_with("Cannot save keep.bbas"),
            "{}",
            app.message()
        );
        assert_eq!(std::fs::read_to_string(&name).unwrap(), "10 OLD\n");
        assert!(app.document().is_changed());
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(names.len(), 1, "no temporary file stays");
    }
}

#[test]
fn new_with_changes_asks_first() {
    let dir = fresh_dir("new");
    let mut app = App::new(Some(text(&dir.join("start.bbas"))));
    type_text(&mut app, "10 END");
    app.key(ctrl('o'));
    type_text(&mut app, "n");
    assert!(screen(&app).contains("Not saved. New? (y/n)"));
    type_text(&mut app, "x");
    assert_eq!(app.document().lines(), ["10 END"]);
    type_text(&mut app, "n");
    type_text(&mut app, "y");
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.document().lines(), [""]);
    assert_eq!(app.document().file_name(), None);
    assert!(!app.document().is_changed());
}

#[test]
fn new_with_no_changes_asks_nothing() {
    let dir = fresh_dir("new_clean");
    let mut app = app_in(&dir);
    type_text(&mut app, "n");
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.document().file_name(), None);
}

#[test]
fn delete_asks_and_removes_the_file_only_on_yes() {
    let dir = sample_dir("delete");
    let mut app = app_in(&dir);
    keys(&mut app, KeyCode::Down, 3);
    type_text(&mut app, "d");
    assert!(screen(&app).contains("Delete b.bas? (y/n)"));
    type_text(&mut app, "n");
    assert!(dir.join("b.bas").exists());
    type_text(&mut app, "d");
    type_text(&mut app, "y");
    assert!(!dir.join("b.bas").exists());
    assert_eq!(listed_names(&app), ["..", "sub/", "a.bbas"]);
}

#[test]
fn delete_does_not_remove_a_directory() {
    let dir = sample_dir("delete_dir");
    let mut app = app_in(&dir);
    app.key(press(KeyCode::Down));
    type_text(&mut app, "d");
    assert!(!screen(&app).contains("(y/n)"));
    assert!(dir.join("sub").is_dir());
}

#[cfg(unix)]
#[test]
fn a_file_with_no_read_permission_gives_a_message() {
    let dir = fresh_dir("no_read");
    let name = dir.join("secret.bbas");
    std::fs::write(&name, "10 END\n").unwrap();
    set_mode(&name, 0o000);
    let mut app = app_in(&dir);
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Enter));
    set_mode(&name, 0o644);
    assert!(
        app.message().starts_with("Cannot read secret.bbas"),
        "{}",
        app.message()
    );
    assert_eq!(app.mode(), Mode::Files);
    assert_eq!(
        app.document().file_name(),
        Some(text(&dir.join("start.bbas")).as_str())
    );
}

#[test]
fn the_movement_keys_stay_in_the_list() {
    let dir = fresh_dir("move");
    for n in 0..40 {
        std::fs::write(dir.join(format!("p{:02}.bbas", n)), "10 END\n").unwrap();
    }
    let mut app = app_in(&dir);
    app.key(press(KeyCode::Up));
    app.key(press(KeyCode::PageDown));
    app.key(press(KeyCode::PageDown));
    app.key(press(KeyCode::PageDown));
    app.key(press(KeyCode::Enter));
    // Page Down 3 times goes to the end of the list.
    assert_eq!(
        app.document().file_name(),
        Some(text(&dir.join("p39.bbas")).as_str())
    );
    app.key(ctrl('o'));
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Down));
    let screen = screen(&app);
    assert!(screen.contains("p39.bbas"), "{screen}");
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::PageUp));
    let screen = self::screen(&app);
    assert!(screen.contains("p00.bbas"), "{screen}");
}

#[test]
fn ctrl_o_changes_to_the_files_tab_from_each_mode() {
    let dir = fresh_dir("ctrl_o");
    let mut app = App::new(Some(text(&dir.join("start.bbas"))));
    app.key(ctrl('o'));
    assert_eq!(app.mode(), Mode::Files);
    app.key(press(KeyCode::Tab));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    app.key(ctrl('o'));
    assert_eq!(app.mode(), Mode::Files);
}
