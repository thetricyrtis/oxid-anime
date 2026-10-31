use directories::ProjectDirs;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum PathError {
    Unavailable,
    Io(io::Error),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Unavailable => write!(f, "Could not determine project directories"),
            PathError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for PathError {}

impl From<io::Error> for PathError {
    fn from(e: io::Error) -> Self {
        PathError::Io(e)
    }
}

fn project_dirs() -> Result<ProjectDirs, PathError> {
    ProjectDirs::from("", "", "oxid-anime").ok_or(PathError::Unavailable)
}

fn ensure_dir(path: &PathBuf) -> Result<(), PathError> {
    fs::create_dir_all(path)?;
    Ok(())
}

/// Persistent data directory (e.g. ~/.local/share/oxid-anime on Linux).
pub fn data_dir() -> Result<PathBuf, PathError> {
    let dir = project_dirs()?.data_dir().to_path_buf();
    ensure_dir(&dir)?;
    Ok(dir)
}

/// Cache directory (e.g. ~/.cache/oxid-anime on Linux).
pub fn cache_dir() -> Result<PathBuf, PathError> {
    let dir = project_dirs()?.cache_dir().to_path_buf();
    ensure_dir(&dir)?;
    Ok(dir)
}

pub fn db_path() -> Result<PathBuf, PathError> {
    let dir = data_dir()?.join("database");
    ensure_dir(&dir)?;
    Ok(dir.join("oxid.db"))
}

pub fn jsonl_path() -> Result<PathBuf, PathError> {
    let dir = data_dir()?.join("database");
    ensure_dir(&dir)?;
    Ok(dir.join("anime-offline-database.jsonl"))
}

pub fn version_path() -> Result<PathBuf, PathError> {
    let dir = data_dir()?.join("database");
    ensure_dir(&dir)?;
    Ok(dir.join("db_version.txt"))
}

pub fn compressed_path() -> Result<PathBuf, PathError> {
    let dir = cache_dir()?.join("compressed");
    ensure_dir(&dir)?;
    Ok(dir.join("anime-offline-database.jsonl.zst"))
}

/// Reserved for future disk-based poster cache. Currently unused.
#[allow(dead_code)]
pub fn posters_cache_dir() -> Result<PathBuf, PathError> {
    let dir = cache_dir()?.join("posters");
    ensure_dir(&dir)?;
    Ok(dir)
}
