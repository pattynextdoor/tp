pub mod schema;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

/// Returns the path to the tp database file.
///
/// Uses `$TP_DATA_DIR` if set, otherwise falls back to
/// `$XDG_DATA_HOME/tp` or `~/.local/share/tp`.
pub fn db_path() -> Result<PathBuf> {
    let dir = if let Ok(custom) = std::env::var("TP_DATA_DIR") {
        PathBuf::from(custom)
    } else if let Some(data) = dirs::data_dir() {
        data.join("tp")
    } else {
        let home = dirs::home_dir().context("could not determine home directory")?;
        home.join(".local").join("share").join("tp")
    };

    std::fs::create_dir_all(&dir).context("could not create data directory")?;
    Ok(dir.join("tp.db"))
}

/// Opens (or creates) the SQLite database and runs migrations.
pub fn open() -> Result<Connection> {
    let path = db_path()?;
    open_at(path)
}

/// Opens a database at a specific path — useful for testing.
pub fn open_at(path: impl AsRef<std::path::Path>) -> Result<Connection> {
    let conn = Connection::open(path.as_ref())?;
    configure(&conn)?;
    schema::migrate(&conn)?;
    Ok(conn)
}

/// Opens an in-memory database — used for unit tests.
#[allow(dead_code)]
pub fn open_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    schema::migrate(&conn)?;
    Ok(conn)
}

/// Applies WAL mode, synchronous=NORMAL, and foreign keys ON.
fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_memory_succeeds() {
        let conn = open_memory().expect("should open in-memory db");
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        // In-memory databases report "memory" for journal_mode
        assert!(mode == "wal" || mode == "memory");
    }
}
