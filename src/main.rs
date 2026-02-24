//! Downloads History - Archive and Index Downloaded Files
//!
//! This application provides a command-line interface for:
//! - Ingesting files from a downloads/incoming directory
//! - Organizing them into year-month based archives
//! - Indexing file metadata in SQLite for fast searching
//! - Searching archived files by filename

// Module declarations - each module is in a separate file
mod database;
mod file_operations;
mod ingest;
mod search;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Command-line interface structure for the application
#[derive(Parser)]
#[command(name = "downloads_history")]
#[command(about = "Archive and index downloaded files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for the application
#[derive(Subcommand)]
enum Commands {
    /// Ingest files from incoming directory into archives/YYYY-MM and index them in SQLite.
    /// `cargo run -- ingest --incoming path/to/incoming --archives path/to/archives --db path/to/db`
    /// Example:
    /// cargo run -- ingest --incoming /shared/downloads_history/incoming --archives /shared/downloads_history/archives --db /home/l4ur3l3i/downloads_history/index.sqlite
    ///
    /// This command:
    /// - Walks through the incoming directory
    /// - Computes file metadata (hash, size, mtime)
    /// - Moves files to archives organized by year-month
    /// - Stores metadata in SQLite database
    Ingest {
        /// Path to the incoming directory containing files to ingest
        #[arg(long)]
        incoming: PathBuf,

        /// Path to the root archives directory (will create YYYY-MM subdirs)
        #[arg(long)]
        archives: PathBuf,

        /// Path to SQLite database file (recommended: on NVMe for performance)
        #[arg(long)]
        db: PathBuf,
    },

    /// Search for files by filename using substring matching.
    /// `cargo run -- search --db path/to/db "search query"`
    /// cargo run -- search --db /shared/downloads_history/index.sqlite "laure"
    /// Currently uses SQL LIKE for searching.
    /// Future enhancement: Full-text search (FTS) for more advanced queries.
    Search {
        /// Path to SQLite database file
        #[arg(long)]
        db: PathBuf,

        /// Search query string (matches anywhere in filename)
        query: String,
    },
}

/// Main entry point - parses CLI arguments and dispatches to the appropriate command
fn main() -> Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Dispatch to the appropriate command handler
    match cli.command {
        Commands::Ingest {
            incoming,
            archives,
            db,
        } => ingest::ingest(&incoming, &archives, &db),
        Commands::Search { db, query } => search::search(&db, &query),
    }
}
