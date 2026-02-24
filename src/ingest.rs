//! Ingest command implementation
//!
//! This module handles ingesting files from an incoming directory,
//! archiving them into year-month folders, and indexing them in the database.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::database::init_db;
use crate::file_operations::{mtime_unix, safe_move, sha256_file, year_month_from_mtime};

/// Ingest files from the incoming directory into organized archives.
///
/// This function:
/// 1. Walks through all files in the incoming directory
/// 2. Computes metadata (size, modification time, SHA-256 hash)
/// 3. Organizes files into archives/YYYY-MM/ folders based on modification time
/// 4. Handles filename conflicts by adding numbered suffixes (e.g., "file (1).txt")
/// 5. Indexes all file metadata in the SQLite database
///
/// # Arguments
/// * `incoming` - Path to the directory containing files to ingest
/// * `archives` - Path to the root archives directory (will create YYYY-MM subdirs)
/// * `db` - Path to the SQLite database file
///
/// # Returns
/// * `Result<()>` - Ok if successful, error otherwise
pub fn ingest(incoming: &Path, archives: &Path, db: &Path) -> Result<()> {
    println!("Starting ingestion from: {}", incoming.display());
    println!("Archiving to: {}", archives.display());
    println!("Database: {}\n", db.display());

    // Create the database directory if it doesn't exist
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create db directory {}", parent.display()))?;
    }

    // Open the database connection
    let conn = Connection::open(db).with_context(|| format!("open db {}", db.display()))?;

    // Initialize the database schema if it doesn't exist
    init_db(&conn)?;

    // Counter for processed files
    let mut processed = 0u64;

    // Track directories that contained files (for cleanup later)
    let mut directories_with_files = HashSet::new();

    // Walk through the incoming directory, processing only files (not directories)
    for entry in WalkDir::new(incoming)
        .min_depth(1) // Skip the root incoming directory itself
        .into_iter()
        .filter_map(|e| e.ok())
    // Skip entries that produce errors
    {
        // Only process regular files, not directories or symlinks
        if !entry.file_type().is_file() {
            continue;
        }

        let src = entry.path().to_path_buf();
        let filename = entry.file_name().to_string_lossy().to_string();

        // Track the parent directory for later cleanup
        if let Some(parent) = src.parent() {
            if parent != incoming {
                directories_with_files.insert(parent.to_path_buf());
            }
        }

        println!("[{}] Processing: {}", processed + 1, filename);

        // Extract file metadata
        let meta = fs::metadata(&src)?;
        let size_bytes = meta.len() as i64;
        let mtime = mtime_unix(&src)?;

        // Determine the year-month folder for organization (e.g., "2024-03")
        let ym = year_month_from_mtime(mtime);
        println!("  → Archive folder: {}", ym);

        // Compute SHA-256 hash for deduplication and integrity verification
        print!("  → Computing SHA-256... ");
        let sha256 = sha256_file(&src)?;
        println!("{}", &sha256[..16]);

        // Construct the destination path
        let dst = archives.join(&ym).join(&filename);

        // Handle filename conflicts by adding numbered suffixes
        let final_dst = if dst.exists() {
            println!("  → File exists, finding unique name...");
            // Extract file stem and extension
            let stem = Path::new(&filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let ext = Path::new(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            // Find the first available suffix number
            let mut i = 1;
            loop {
                let candidate = if ext.is_empty() {
                    // No extension: "filename (1)"
                    archives.join(&ym).join(format!("{stem} ({i})"))
                } else {
                    // With extension: "filename (1).ext"
                    archives.join(&ym).join(format!("{stem} ({i}).{ext}"))
                };

                if !candidate.exists() {
                    break candidate;
                }
                i += 1;
            }
        } else {
            dst
        };

        // Move the file to its final destination
        println!("  → Moving to: {}", final_dst.display());
        safe_move(&src, &final_dst)?;

        // Convert path to string for database storage
        let path_str = final_dst.to_string_lossy().to_string();

        // Insert file metadata into the database
        // INSERT OR IGNORE handles potential duplicates gracefully
        println!("  → Indexing in database");
        conn.execute(
            "INSERT OR IGNORE INTO files(path, filename, size_bytes, mtime_unix, sha256) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path_str, filename, size_bytes, mtime, sha256],
        )?;

        processed += 1;
        println!("  ✓ Complete\n");
    }

    // Clean up empty directories from deepest to shallowest
    if !directories_with_files.is_empty() {
        println!("Cleaning up empty directories...");

        // Sort directories by depth (deepest first) so we can remove child dirs before parents
        let mut dirs: Vec<PathBuf> = directories_with_files.into_iter().collect();
        dirs.sort_by(|a, b| {
            let depth_a = a.components().count();
            let depth_b = b.components().count();
            depth_b.cmp(&depth_a) // Reverse order (deepest first)
        });

        for dir in dirs {
            // Check if directory is now empty
            if let Ok(mut entries) = fs::read_dir(&dir) {
                if entries.next().is_none() {
                    // Directory is empty, remove it
                    match fs::remove_dir(&dir) {
                        Ok(_) => println!("  → Removed empty directory: {}", dir.display()),
                        Err(e) => println!("  → Could not remove {}: {}", dir.display(), e),
                    }
                }
            }
        }
        println!();
    }

    // Report summary of ingestion
    println!("═══════════════════════════════════════");
    println!("Ingest complete!");
    println!("Total files processed: {}", processed);
    println!("═══════════════════════════════════════");
    Ok(())
}
