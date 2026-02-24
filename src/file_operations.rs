//! File system operations and metadata extraction
//!
//! This module provides utilities for:
//! - Computing SHA-256 hashes of files
//! - Extracting file modification times
//! - Deriving year-month folders from timestamps
//! - Safely moving files across filesystems

use anyhow::{Context, Result};
use chrono::Datelike;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Compute the SHA-256 hash of a file.
///
/// Reads the file in 1MB chunks to handle large files efficiently
/// without loading the entire file into memory.
///
/// # Arguments
/// * `path` - Path to the file to hash
///
/// # Returns
/// * `Result<String>` - Hex-encoded SHA-256 hash string, or error if file cannot be read
pub fn sha256_file(path: &Path) -> Result<String> {
    // Open the file with context for better error messages
    let mut f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;

    // Initialize the SHA-256 hasher
    let mut hasher = Sha256::new();

    // Read in 1MB chunks for memory efficiency
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break; // End of file reached
        }
        hasher.update(&buf[..n]);
    }

    // Return the hash as a hex string
    Ok(hex::encode(hasher.finalize()))
}

/// Get the modification time of a file as a Unix timestamp.
///
/// Falls back to current system time if modification time cannot be retrieved.
///
/// # Arguments
/// * `path` - Path to the file
///
/// # Returns
/// * `Result<i64>` - Unix timestamp (seconds since epoch)
pub fn mtime_unix(path: &Path) -> Result<i64> {
    let meta = fs::metadata(path)?;

    // Get modified time, falling back to now if unavailable
    let mtime = meta.modified().unwrap_or(SystemTime::now());

    // Convert to Unix timestamp (seconds since UNIX_EPOCH)
    let secs = mtime
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    Ok(secs)
}

/// Convert a Unix timestamp to a YYYY-MM string for folder organization.
///
/// This is used to organize archived files into year-month folders
/// based on their modification time (e.g., "2024-03", "2025-12").
///
/// # Arguments
/// * `secs` - Unix timestamp in seconds
///
/// # Returns
/// * `String` - Year-month string in YYYY-MM format
pub fn year_month_from_mtime(secs: i64) -> String {
    // Convert Unix timestamp to UTC DateTime
    let dt = chrono::DateTime::<chrono::Utc>::from(
        UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
    );

    // Format as YYYY-MM
    format!("{:04}-{:02}", dt.year(), dt.month())
}

/// Safely move a file from source to destination, creating parent directories if needed.
///
/// First attempts a simple rename (fast for same filesystem).
/// If that fails (e.g., cross-device move), falls back to copy + delete.
///
/// # Arguments
/// * `src` - Source file path
/// * `dst` - Destination file path
///
/// # Returns
/// * `Result<()>` - Ok if successful, error otherwise
pub fn safe_move(src: &Path, dst: &Path) -> Result<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    // Try rename first (fast, atomic on same filesystem)
    match fs::rename(src, dst) {
        Ok(_) => Ok(()),
        Err(_) => {
            // If rename fails (e.g., cross-device), fall back to copy + remove
            fs::copy(src, dst)?;
            fs::remove_file(src)?;
            Ok(())
        }
    }
}
