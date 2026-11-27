//! File system operations for BBC BASIC
//!
//! Handles file I/O operations and star commands.

use crate::error::{BBCBasicError, Result};

/// File system interface
#[derive(Debug)]
pub struct FileSystem {
    // Implementation will be added in later tasks
}

impl FileSystem {
    /// Create a new file system interface
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FileSystem {
    fn default() -> Self {
        Self::new()
    }
}

// Directory functions: LISTDIR$, MKDIR and RMDIR. These are modern
// extensions, not BBC BASIC.

/// The maximum length of a BBC BASIC string
const MAX_STRING_LENGTH: usize = 255;

/// Convert an I/O error of a directory function to a BBC BASIC error.
fn directory_error(path: &str, error: std::io::Error) -> BBCBasicError {
    match error.kind() {
        std::io::ErrorKind::NotFound => BBCBasicError::FileNotFound(path.to_string()),
        std::io::ErrorKind::NotADirectory | std::io::ErrorKind::DirectoryNotEmpty => {
            BBCBasicError::BadDirectory
        }
        _ => BBCBasicError::DiskError(error.to_string()),
    }
}

/// Give an error if `path` is not a directory.
fn require_directory(path: &str) -> Result<()> {
    let metadata = std::fs::metadata(path).map_err(|e| directory_error(path, e))?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(BBCBasicError::BadDirectory)
    }
}

/// The names of the entries of a directory, in name order, with a line feed
/// between them. A sub-directory name has a `/` at its end.
pub fn list_directory(path: &str) -> Result<String> {
    require_directory(path)?;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| directory_error(path, e))? {
        let entry = entry.map_err(|e| directory_error(path, e))?;
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            name.push('/');
        }
        names.push(name);
    }
    names.sort();
    let listing = names.join("\n");
    if listing.len() > MAX_STRING_LENGTH {
        return Err(BBCBasicError::StringTooLong);
    }
    Ok(listing)
}

/// Make one directory. A directory that exists already gives no error.
pub fn make_directory(path: &str) -> Result<()> {
    match std::fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => require_directory(path),
        Err(e) => Err(directory_error(path, e)),
    }
}

/// Remove one empty directory. This is never a recursive delete: a directory
/// with content and a file each give Bad directory.
pub fn remove_directory(path: &str) -> Result<()> {
    require_directory(path)?;
    std::fs::remove_dir(path).map_err(|e| directory_error(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// Make a new empty directory for one test.
    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bbc_filesystem_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).expect("test directory is made");
        dir
    }

    fn text(path: &Path) -> String {
        path.to_str().expect("path is text").to_string()
    }

    #[test]
    fn test_list_directory_gives_names_in_order_with_slash_on_directories() {
        let dir = fresh_dir("list");
        std::fs::write(dir.join("b.dat"), "x").unwrap();
        std::fs::create_dir(dir.join("a")).unwrap();
        std::fs::write(dir.join("c"), "x").unwrap();

        assert_eq!(list_directory(&text(&dir)), Ok("a/\nb.dat\nc".to_string()));
    }

    #[test]
    fn test_list_directory_of_empty_directory_is_empty() {
        let dir = fresh_dir("list_empty");

        assert_eq!(list_directory(&text(&dir)), Ok(String::new()));
    }

    #[test]
    fn test_list_directory_errors() {
        let dir = fresh_dir("list_errors");
        std::fs::write(dir.join("f"), "x").unwrap();

        assert!(matches!(
            list_directory(&text(&dir.join("missing"))),
            Err(BBCBasicError::FileNotFound(_))
        ));
        assert_eq!(
            list_directory(&text(&dir.join("f"))),
            Err(BBCBasicError::BadDirectory)
        );
    }

    #[test]
    fn test_list_directory_longer_than_a_string_gives_string_too_long() {
        let dir = fresh_dir("list_long");
        for n in 0..30 {
            std::fs::write(dir.join(format!("file_number_{:02}", n)), "x").unwrap();
        }

        assert_eq!(
            list_directory(&text(&dir)),
            Err(BBCBasicError::StringTooLong)
        );
    }

    #[test]
    fn test_make_directory_makes_one_directory_and_accepts_one_that_exists() {
        let dir = fresh_dir("make");
        let new_dir = dir.join("new");

        assert_eq!(make_directory(&text(&new_dir)), Ok(()));
        assert!(new_dir.is_dir());
        assert_eq!(make_directory(&text(&new_dir)), Ok(()));
    }

    #[test]
    fn test_make_directory_errors() {
        let dir = fresh_dir("make_errors");
        std::fs::write(dir.join("f"), "x").unwrap();

        assert!(matches!(
            make_directory(&text(&dir.join("missing").join("new"))),
            Err(BBCBasicError::FileNotFound(_))
        ));
        assert!(!dir.join("missing").exists());
        assert_eq!(
            make_directory(&text(&dir.join("f"))),
            Err(BBCBasicError::BadDirectory)
        );
    }

    #[test]
    fn test_remove_directory_removes_an_empty_directory() {
        let dir = fresh_dir("remove");
        std::fs::create_dir(dir.join("empty")).unwrap();

        assert_eq!(remove_directory(&text(&dir.join("empty"))), Ok(()));
        assert!(!dir.join("empty").exists());
    }

    #[test]
    fn test_remove_directory_errors_and_removes_nothing() {
        let dir = fresh_dir("remove_errors");
        std::fs::create_dir(dir.join("full")).unwrap();
        std::fs::write(dir.join("full").join("data"), "x").unwrap();
        std::fs::write(dir.join("f"), "x").unwrap();

        assert_eq!(
            remove_directory(&text(&dir.join("full"))),
            Err(BBCBasicError::BadDirectory)
        );
        assert!(dir.join("full").join("data").is_file());
        assert_eq!(
            remove_directory(&text(&dir.join("f"))),
            Err(BBCBasicError::BadDirectory)
        );
        assert!(dir.join("f").is_file());
        assert!(matches!(
            remove_directory(&text(&dir.join("missing"))),
            Err(BBCBasicError::FileNotFound(_))
        ));
    }
}
