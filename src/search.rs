//! Search command implementation
//!
//! This module handles searching the database for files by filename.
//! Currently uses SQL LIKE for substring matching.
//! Future enhancement: Full-text search (FTS) for more advanced queries.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use crate::database::init_db;

/// Search for files in the database by filename.
///
/// Performs a case-insensitive substring search using SQL LIKE.
/// Results are ordered by modification time (newest first) and limited to 50 entries.
///
/// # Arguments
/// * `db` - Path to the SQLite database file
/// * `query` - Search string to match against filenames
///
/// # Display Format
/// For each matching file, prints:
/// - Full path to the archived file
/// - Size in bytes
/// - Modification time (Unix timestamp)
/// - First 16 characters of SHA-256 hash
///
/// # Returns
/// * `Result<()>` - Ok if successful, error otherwise
pub fn search(db: &Path, query: &str) -> Result<()> {
    // Open the database connection
    let conn = Connection::open(db)?;

    // Ensure the database schema exists
    init_db(&conn)?;

    // Build a LIKE pattern: %query% matches query anywhere in the filename
    let like = format!("%{}%", query);

    // Prepare the search query
    // Note: LIKE is case-insensitive by default in SQLite
    let mut stmt = conn.prepare(
        "SELECT path, size_bytes, mtime_unix, sha256
         FROM files
         WHERE filename LIKE ?1
         ORDER BY mtime_unix DESC
         LIMIT 50",
    )?;

    // Execute the query and map each row to a tuple
    let rows = stmt.query_map([like], |row| {
        Ok((
            row.get::<_, String>(0)?, // path
            row.get::<_, i64>(1)?,    // size_bytes
            row.get::<_, i64>(2)?,    // mtime_unix
            row.get::<_, String>(3)?, // sha256
        ))
    })?;

    // Display results
    for r in rows {
        let (path, size, mtime, sha) = r?;

        // Print file information
        // SHA-256 is truncated to first 16 chars for readability
        println!(
            "{path}\n  size={size} mtime={mtime} sha256={}\n",
            &sha[..16]
        );
    }

    Ok(())
}
