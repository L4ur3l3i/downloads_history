use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "downloads_history")]
#[command(about = "Archive and index downloaded files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest files from incoming directory into archives/YYYY-MM and index them in SQLite.
    Ingest {
        #[arg(long)]
        incoming: PathBuf,

        #[arg(long)]
        archives: PathBuf,

        /// Path to SQLite DB (should be on NVMe)
        #[arg(long)]
        db: PathBuf,
    },

    /// Search by filename (FTS comes later; for now LIKE query)
    Search {
        #[arg(long)]
        db: PathBuf,

        query: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest {
            incoming,
            archives,
            db,
        } => ingest(&incoming, &archives, &db),
        Commands::Search { db, query } => search(&db, &query),
    }
}

fn init_db(conn: &Connection) -> Result<()> {
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

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn mtime_unix(path: &Path) -> Result<i64> {
    let meta = fs::metadata(path)?;
    let mtime = meta.modified().unwrap_or(SystemTime::now());
    let secs = mtime
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Ok(secs)
}

fn year_month_from_mtime(secs: i64) -> String {
    // Minimal: derive YYYY-MM from unix timestamp using chrono.
    let dt = chrono::DateTime::<chrono::Utc>::from(
        UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
    );
    format!("{:04}-{:02}", dt.year(), dt.month())
}

fn safe_move(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // Try rename; if cross-device, fallback to copy+remove.
    match fs::rename(src, dst) {
        Ok(_) => Ok(()),
        Err(_) => {
            fs::copy(src, dst)?;
            fs::remove_file(src)?;
            Ok(())
        }
    }
}

fn ingest(incoming: &Path, archives: &Path, db: &Path) -> Result<()> {
    let conn = Connection::open(db).with_context(|| format!("open db {}", db.display()))?;
    init_db(&conn)?;

    // Walk only files (not dirs)
    let mut processed = 0u64;

    for entry in WalkDir::new(incoming)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let src = entry.path().to_path_buf();
        let filename = entry.file_name().to_string_lossy().to_string();

        let meta = fs::metadata(&src)?;
        let size_bytes = meta.len() as i64;
        let mtime = mtime_unix(&src)?;
        let ym = year_month_from_mtime(mtime);

        let sha256 = sha256_file(&src)?;

        let dst = archives.join(&ym).join(&filename);

        // If destination exists, disambiguate by adding suffix
        let final_dst = if dst.exists() {
            let stem = Path::new(&filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let ext = Path::new(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let mut i = 1;
            loop {
                let candidate = if ext.is_empty() {
                    archives.join(&ym).join(format!("{stem} ({i})"))
                } else {
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

        safe_move(&src, &final_dst)?;

        let path_str = final_dst.to_string_lossy().to_string();

        conn.execute(
            "INSERT OR IGNORE INTO files(path, filename, size_bytes, mtime_unix, sha256) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path_str, filename, size_bytes, mtime, sha256],
        )?;

        processed += 1;
    }

    println!("Ingest done. Files processed: {processed}");
    Ok(())
}

fn search(db: &Path, query: &str) -> Result<()> {
    let conn = Connection::open(db)?;
    init_db(&conn)?;

    let like = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT path, size_bytes, mtime_unix, sha256
         FROM files
         WHERE filename LIKE ?1
         ORDER BY mtime_unix DESC
         LIMIT 50",
    )?;

    let rows = stmt.query_map([like], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    for r in rows {
        let (path, size, mtime, sha) = r?;
        println!(
            "{path}\n  size={size} mtime={mtime} sha256={}\n",
            &sha[..16]
        );
    }

    Ok(())
}

// chrono traits used in year_month_from_mtime
use chrono::Datelike;
