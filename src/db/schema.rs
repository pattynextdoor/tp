use anyhow::Result;
use rusqlite::Connection;

/// Runs idempotent schema migrations (CREATE TABLE IF NOT EXISTS).
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS directories (
    id           INTEGER PRIMARY KEY,
    path         TEXT UNIQUE NOT NULL,
    frecency     REAL NOT NULL DEFAULT 1.0,
    last_access  INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    access_count INTEGER NOT NULL DEFAULT 1,
    project_root TEXT
);

CREATE TABLE IF NOT EXISTS projects (
    id          INTEGER PRIMARY KEY,
    path        TEXT UNIQUE NOT NULL,
    name        TEXT,
    kind        TEXT,
    description TEXT,
    indexed_at  INTEGER
);

CREATE TABLE IF NOT EXISTS waypoints (
    id         INTEGER PRIMARY KEY,
    name       TEXT UNIQUE NOT NULL,
    path       TEXT NOT NULL,
    created_at INTEGER DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS sessions (
    id         INTEGER PRIMARY KEY,
    timestamp  INTEGER DEFAULT (strftime('%s','now')),
    from_path  TEXT,
    to_path    TEXT,
    query      TEXT,
    match_type TEXT
);

CREATE INDEX IF NOT EXISTS idx_directories_frecency ON directories(frecency DESC);
CREATE INDEX IF NOT EXISTS idx_directories_path ON directories(path);
CREATE INDEX IF NOT EXISTS idx_directories_project_root ON directories(project_root);
CREATE TABLE IF NOT EXISTS project_dirs (
    id           INTEGER PRIMARY KEY,
    project_root TEXT NOT NULL,
    rel_path     TEXT NOT NULL,
    description  TEXT NOT NULL,
    indexed_at   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(project_root, rel_path)
);

CREATE INDEX IF NOT EXISTS idx_sessions_timestamp ON sessions(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_project_dirs_root ON project_dirs(project_root);
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn migrate_is_idempotent() {
        let conn = db::open_memory().unwrap();
        migrate(&conn).expect("second migration should succeed");
        migrate(&conn).expect("third migration should succeed");
    }

    #[test]
    fn tables_exist_after_migration() {
        let conn = db::open_memory().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"directories".to_string()));
        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"waypoints".to_string()));
        assert!(tables.contains(&"sessions".to_string()));
    }

    #[test]
    fn insert_directory() {
        let conn = db::open_memory().unwrap();
        conn.execute(
            "INSERT INTO directories (path) VALUES (?1)",
            ["/home/user/projects"],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn insert_waypoint() {
        let conn = db::open_memory().unwrap();
        conn.execute(
            "INSERT INTO waypoints (name, path) VALUES (?1, ?2)",
            ["home", "/home/user"],
        )
        .unwrap();

        let path: String = conn
            .query_row(
                "SELECT path FROM waypoints WHERE name = ?1",
                ["home"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(path, "/home/user");
    }

    #[test]
    fn project_dirs_table_exists() {
        let conn = db::open_memory().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"project_dirs".to_string()));
    }

    #[test]
    fn insert_project_dir() {
        let conn = db::open_memory().unwrap();
        conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            ["/home/user/tp", "src/ai", "AI integration module"],
        )
        .unwrap();

        let desc: String = conn
            .query_row(
                "SELECT description FROM project_dirs WHERE project_root = ?1 AND rel_path = ?2",
                ["/home/user/tp", "src/ai"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(desc, "AI integration module");
    }

    #[test]
    fn project_dir_unique_constraint() {
        let conn = db::open_memory().unwrap();
        conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            ["/home/user/tp", "src/ai", "first"],
        )
        .unwrap();
        let result = conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            ["/home/user/tp", "src/ai", "second"],
        );
        assert!(result.is_err());
    }

    #[test]
    fn directory_path_is_unique() {
        let conn = db::open_memory().unwrap();
        conn.execute("INSERT INTO directories (path) VALUES (?1)", ["/tmp"])
            .unwrap();
        let result = conn.execute("INSERT INTO directories (path) VALUES (?1)", ["/tmp"]);
        assert!(result.is_err());
    }
}
