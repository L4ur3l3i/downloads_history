//! Database initialization and schema management
//!
//! This module handles all SQLite database operations including:
//! - Creating and initializing the database schema
//! - Setting up indexes for efficient queries
//! - Managing the files table structure

use anyhow::Result;
use rusqlite::Connection;

/// Initialize the database schema if it doesn't already exist.
///
/// Creates the `files` table to store metadata about archived files:
/// - `id`: Primary key (auto-increment)
/// - `path`: Full path to the archived file (unique)
/// - `filename`: Original filename for search purposes
/// - `size_bytes`: File size in bytes
/// - `mtime_unix`: Last modified time as Unix timestamp
/// - `sha256`: SHA-256 hash for deduplication and integrity
///
/// Also creates indexes on `filename` and `sha256` for faster queries.
///
/// # Arguments
/// * `conn` - SQLite connection reference
///
/// # Returns
/// * `Result<()>` - Ok if successful, error otherwise
pub fn init_db(conn: &Connection) -> Result<()> {
    // Execute the schema creation queries as a batch for efficiency
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS files (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  filename TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  mtime_unix INTEGER NOT NULL,
  sha256 TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename);
CREATE INDEX IF NOT EXISTS idx_files_sha256 ON files(sha256);
"#,
    )?;
    Ok(())
}
