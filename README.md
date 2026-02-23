# downloads_history

Personal homelab tool to archive and index Windows Downloads history.

## Goals (MVP)
- Pull files from a Windows machine (SSH)
- Ingest into `archives/YYYY-MM/`
- Build a lightweight index (SQLite FTS5) on NVMe
- Detect duplicates via SHA-256

## Stack (planned)
- Rust (CLI + later API)
- SQLite (FTS5)
- Optional UI (Vue) later
