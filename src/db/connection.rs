use std::env;
use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::error::{ErrorCode, TaskaiError};

use super::migrations;

/// Find the .git root by walking up from current directory.
pub fn find_git_root() -> Result<PathBuf, TaskaiError> {
    let mut dir = env::current_dir().map_err(|e| TaskaiError::database(e.to_string()))?;
    loop {
        if dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(TaskaiError::new(
                ErrorCode::NotInitialized,
                "Not inside a git repository. taskai requires a git repository.",
            ));
        }
    }
}

/// Get the path to the taskai database.
pub fn db_path() -> Result<PathBuf, TaskaiError> {
    let root = find_git_root()?;
    Ok(root.join(".worktoolai").join("taskai").join("taskai.db"))
}

/// Get the config file path.
pub fn config_path() -> Result<PathBuf, TaskaiError> {
    let root = find_git_root()?;
    Ok(root.join(".worktoolai").join("taskai").join("config.json"))
}

/// Open a connection to the database. Returns error if not initialized.
pub fn open_db() -> Result<Connection, TaskaiError> {
    let path = db_path()?;
    if !path.exists() {
        return Err(TaskaiError::not_initialized());
    }
    let conn = Connection::open(&path)?;
    configure_connection(&conn)?;
    Ok(conn)
}

/// Initialize the database: create directories, database, and run migrations.
pub fn init_db() -> Result<PathBuf, TaskaiError> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| TaskaiError::database(e.to_string()))?;
    }
    let conn = Connection::open(&path)?;
    configure_connection(&conn)?;
    migrations::run_migrations(&conn)?;
    Ok(path)
}

fn configure_connection(conn: &Connection) -> Result<(), TaskaiError> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA busy_timeout=5000;
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(())
}
