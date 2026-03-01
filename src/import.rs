use anyhow::Result;
use rusqlite::Connection;
use std::io::BufRead;

use crate::project;

/// Import directory entries from zoxide's `query --list --score` output.
///
/// Each line is expected in the format: `  <score> <path>`
/// where score is a float (possibly left-padded with spaces) followed by a space and the path.
///
/// Entries are upserted into the `directories` table:
/// - New paths are inserted with the zoxide score as frecency.
/// - Existing paths get their frecency updated to whichever is higher (old or new).
///
/// Returns the count of successfully imported entries.
pub fn import_zoxide(conn: &Connection, reader: impl BufRead) -> Result<u64> {
    let tx = conn.unchecked_transaction()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let mut count: u64 = 0;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        // Skip blank lines
        if trimmed.is_empty() {
            continue;
        }

        // Parse: "<score> <path>" — score is a float, path is everything after the first space.
        // After trimming, the first token is the score, the rest is the path.
        let (score_str, path) = match trimmed.split_once(' ') {
            Some((s, p)) => (s, p.trim()),
            None => continue, // No space found — unparseable, skip
        };

        let score: f64 = match score_str.parse() {
            Ok(s) => s,
            Err(_) => continue, // Not a valid float — skip
        };

        if path.is_empty() {
            continue;
        }

        // Detect project root for the imported path
        let project_root = project::detect_project_root(path);

        // Upsert: insert new entries, or update existing ones to the higher frecency
        tx.execute(
            "INSERT INTO directories (path, frecency, last_access, access_count, project_root)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT(path) DO UPDATE SET
               frecency = MAX(frecency, ?2),
               last_access = MAX(last_access, ?3),
               project_root = COALESCE(?4, project_root)",
            rusqlite::params![path, score, now, project_root],
        )?;

        count += 1;
    }

    tx.commit()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::io::Cursor;

    /// Helper: count rows in the directories table.
    fn count_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn test_import_zoxide_basic() {
        // Two valid zoxide lines should produce count=2 and 2 rows in the DB.
        let conn = db::open_memory().unwrap();
        let input = "  12.4 /home/user/projects\n   3.0 /tmp/scratch\n";
        let reader = Cursor::new(input);

        let count = import_zoxide(&conn, reader).unwrap();

        assert_eq!(count, 2, "should report 2 imported entries");
        assert_eq!(count_rows(&conn), 2, "should have 2 rows in directories");
    }

    #[test]
    fn test_import_zoxide_skips_blank_lines() {
        // Blank and whitespace-only lines should be silently skipped.
        let conn = db::open_memory().unwrap();
        let input = "\n   \n  10.0 /valid/path\n\n";
        let reader = Cursor::new(input);

        let count = import_zoxide(&conn, reader).unwrap();

        assert_eq!(count, 1);
        assert_eq!(count_rows(&conn), 1);
    }

    #[test]
    fn test_import_zoxide_empty_input() {
        // Empty input should import zero entries.
        let conn = db::open_memory().unwrap();
        let input = "";
        let reader = Cursor::new(input);

        let count = import_zoxide(&conn, reader).unwrap();

        assert_eq!(count, 0);
        assert_eq!(count_rows(&conn), 0);
    }

    #[test]
    fn test_import_zoxide_deduplicates() {
        // Importing the same path twice should upsert (not create a duplicate row).
        // The second import should update the frecency to the higher value.
        let conn = db::open_memory().unwrap();

        let input1 = "  5.0 /home/user/projects\n";
        let count1 = import_zoxide(&conn, Cursor::new(input1)).unwrap();
        assert_eq!(count1, 1);

        let input2 = "  20.0 /home/user/projects\n";
        let count2 = import_zoxide(&conn, Cursor::new(input2)).unwrap();
        assert_eq!(count2, 1);

        // Should still be just 1 row, not 2.
        assert_eq!(count_rows(&conn), 1, "should have 1 row after dedup upsert");

        // Frecency should reflect the higher (newer) value.
        let frecency: f64 = conn
            .query_row(
                "SELECT frecency FROM directories WHERE path = ?1",
                ["/home/user/projects"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            frecency >= 20.0,
            "frecency should be updated to at least 20.0, got {}",
            frecency
        );
    }
}
