//! Indexer: directory scan -> SQLite metadata + filename search (design spec 8, Phase 1).
//!
//! Owns the file index; does not rank, render or execute actions.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexedFile {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: i64,
    pub modified_ms: i64,
}

/// P25-C06 content-index extension point: explicitly DISABLED for P2.5
/// (spec C01 forbidden list: no default content indexing). When a future
/// phase enables it, the FTS schema gains a `content` column and the
/// observation pipeline grows an extractor hook; content hits route through
/// the same metadata-authoritative contract and the same LIKE fallback.
pub const CONTENT_INDEX_ENABLED: bool = false;

pub struct Indexer {
    conn: Connection,
    /// P25-C01: FTS5 accelerator availability (false when the SQLite build
    /// lacks FTS5 or the index is corrupt beyond repair — search then falls
    /// back to LIKE, C05).
    fts_available: bool,
}

/// Maximum number of files indexed per `rebuild` call, as a safety bound.
pub const MAX_INDEX_ENTRIES: i64 = 500_000;

pub mod coordinator;
pub mod incremental;
pub mod watcher;

impl Indexer {
    /// C6 (review 87 §7): open with corruption recovery. If the existing
    /// file is corrupt (init fails on first use), it is deleted and
    /// recreated fresh. The index is rebuildable derived state: data loss
    /// is acceptable, startup failure is not (INV-INDEX-004).
    pub fn open(db_path: &Path) -> Result<Self, IndexError> {
        match Self::try_open(db_path) {
            Ok(ix) => Ok(ix),
            Err(e) => {
                warn!(
                    path = %db_path.display(), error = %e,
                    "index db corrupt — deleting and recreating"
                );
                let _ = std::fs::remove_file(db_path);
                let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
                let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
                Self::try_open(db_path)
            }
        }
    }

    fn try_open(db_path: &Path) -> Result<Self, IndexError> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // FIX-05: a background rebuild (big write transaction) coexists with
        // foreground history writes on other connections — wait instead of
        // surfacing SQLITE_BUSY to the caller.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let fts_available = Self::init_schema(&conn)?;
        Ok(Self { conn, fts_available })
    }

    pub fn in_memory() -> Result<Self, IndexError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let fts_available = Self::init_schema(&conn)?;
        Ok(Self { conn, fts_available })
    }

    fn init_schema(conn: &Connection) -> Result<bool, IndexError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY);
            INSERT OR IGNORE INTO schema_migrations VALUES (1);

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                parent TEXT NOT NULL,
                is_dir INTEGER NOT NULL,
                size INTEGER NOT NULL,
                modified_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_files_name ON files(name);
            CREATE INDEX IF NOT EXISTS idx_files_parent ON files(parent);

            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY,
                command_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                used_at_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_history_command ON history(command_id);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS index_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO index_meta(key, value) VALUES ('schema_version', '1');
            "#,
        )?;
        // Migration: usage rows may carry the command title so the host can
        // rebuild recency-ordered result views without a provider round-trip.
        // Already-applied databases raise "duplicate column name" here.
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN title TEXT NOT NULL DEFAULT ''",
            [],
        );
        // ---- P25-C01: FTS5 accelerator (idempotent migration) ----
        // The FTS index is a derived structure over the AUTHORITATIVE
        // metadata tables (AC-C01-2): a missing/broken FTS index never
        // blocks metadata reads or writes (search falls back to LIKE, C05).
        // Content indexing is explicitly NOT enabled (spec C01 forbidden
        // list / C06 extension point).
        let fts_available = match conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(name, pinyin_init, path UNINDEXED)",
        ) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "FTS5 unavailable — file search uses LIKE fallback");
                false
            }
        };
        Ok(fts_available)
    }

    /// Rebuild the FTS index from the authoritative metadata (full resync).
    /// Used after bulk operations and as the corruption recovery path.
    pub fn rebuild_fts(&self) -> Result<(), IndexError> {
        if !self.fts_available {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM files_fts", [])?;
        // compute pinyin initials per file (SQL can't do CJK boundary mapping)
        let mut stmt = tx.prepare("SELECT name, path FROM files")?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        for (name, path) in &rows {
            let py = launcher_domain::pinyin::pinyin_initials(name);
            tx.execute(
                "INSERT INTO files_fts(name, pinyin_init, path) VALUES (?1, ?2, ?3)",
                params![name, py, path],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Incremental FTS sync for one upsert (caller's transaction).
    fn fts_upsert(tx: &rusqlite::Transaction, path: &str, name: &str) {
        let py = launcher_domain::pinyin::pinyin_initials(name);
        if let Err(e) = (|| -> Result<(), rusqlite::Error> {
            tx.execute("DELETE FROM files_fts WHERE path = ?1", params![path])?;
            tx.execute("INSERT INTO files_fts(name, pinyin_init, path) VALUES (?1, ?2, ?3)",
                       params![name, py, path])?;
            Ok(())
        })() {
            tracing::warn!(error = %e, path = %path, "fts upsert skipped (non-fatal)");
        }
    }

    /// Incremental FTS sync for one delete by ACTUAL path (caller's tx).
    fn fts_delete(tx: &rusqlite::Transaction, path: &str) {
        let _ = tx.execute("DELETE FROM files_fts WHERE path = ?1", params![path]);
    }

    /// Full rebuild: scan `root` recursively (bounded depth) replacing old rows.
    pub fn rebuild(&mut self, roots: &[PathBuf]) -> Result<usize, IndexError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM files", [])?;
        let mut remaining = MAX_INDEX_ENTRIES;
        for root in roots {
            if remaining <= 0 {
                warn!("global index entry budget exhausted");
                break;
            }
            scan_into(&tx, root, 0, &mut remaining)?;
        }
        let count = MAX_INDEX_ENTRIES - remaining;
        // IndexGeneration (review 72 §25/26): bumped on every full rebuild
        // (and later on recovery rescans) so caches/diagnostics can key on
        // "which version of the index am I looking at".
        tx.execute(
            "INSERT INTO index_meta(key, value) VALUES ('generation', '1')
             ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
            [],
        )?;
        tx.commit()?;
        info!(count, "indexer.updated");
        Ok(count as usize)
    }

    /// File search: FTS5 prefix-phrase first (P25-C01 accelerator), falling
    /// back to the LIKE substring scan when FTS yields nothing, is broken
    /// (one self-healing rebuild attempt, then C05 fallback), or is absent.
    /// Result contract and bound are identical for both paths.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>, IndexError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        // P25-C01/C05: FTS-ranked hits first, LIKE hits merged after (deduped
        // by id). This preserves the old PATH-fragment matches (FTS indexes
        // names only) while adding FTS relevance ordering. A broken FTS index
        // gets ONE self-healing rebuild attempt before the LIKE fallback.
        let mut fts: Vec<IndexedFile> = Vec::new();
        if self.fts_available {
            let mut phrase_text = query.trim().to_lowercase();
            phrase_text = phrase_text.replace('"', "\"\"");
            let phrase = format!("\"{}\"", phrase_text);
            let phrase_prefix = format!("{phrase} *");
            for attempt in [0, 1] {
                match self.search_fts_inner(&phrase_prefix, limit) {
                    Ok(v) if !v.is_empty() => {
                        fts = v;
                        break;
                    }
                    Ok(_) if attempt == 0 => {
                        // empty: maybe the FTS index is stale/empty — one
                        // rebuild then retry before the LIKE pass
                        let _ = self.rebuild_fts();
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "fts search failed — rebuilding");
                        let _ = self.rebuild_fts();
                    }
                }
            }
        }
        self.search_merged(fts, query, limit)
    }

    /// Merge FTS-ranked hits with LIKE hits (deduped), preserving total bound.
    fn search_merged(
        &self,
        fts: Vec<IndexedFile>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<IndexedFile>, IndexError> {
        let mut out: Vec<IndexedFile> = Vec::with_capacity(limit.min(64));
        let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for f in fts {
            if out.len() >= limit {
                return Ok(out);
            }
            if seen.insert(f.id) {
                out.push(f);
            }
        }
        for f in self.search_like(query, limit)? {
            if out.len() >= limit {
                break;
            }
            if seen.insert(f.id) {
                out.push(f);
            }
        }
        Ok(out)
    }

    fn search_fts_inner(&self, match_expr: &str, limit: usize) -> Result<Vec<IndexedFile>, IndexError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.id, f.path, f.name, f.is_dir, f.size, f.modified_ms
             FROM files_fts j JOIN files f ON f.path = j.path
             WHERE files_fts MATCH ?1 OR pinyin_init MATCH ?1
             ORDER BY rank, f.is_dir, length(f.name), f.path
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![match_expr, limit as i64], |row| {
            Ok(IndexedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                is_dir: row.get::<_, i64>(3)? != 0,
                size: row.get(4)?,
                modified_ms: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// C05: the original LIKE substring scan — authoritative fallback,
    /// retained unchanged.
    fn search_like(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>, IndexError> {
        let pattern = format!("%{}%", escape_like(query.trim().to_lowercase()));
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, path, name, is_dir, size, modified_ms FROM files
             WHERE (lower(name) LIKE ?1 ESCAPE '\\' OR lower(path) LIKE ?1 ESCAPE '\\')
             ORDER BY (lower(name) LIKE ?1 ESCAPE '\\') DESC,
                      is_dir, length(name), path
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(IndexedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                is_dir: row.get::<_, i64>(3)? != 0,
                size: row.get(4)?,
                modified_ms: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// P2.1-B IncrementalWriter: apply one VERIFIED batch in a single
    /// transaction; generation advances only if the commit succeeds
    /// (INV-INDEX-005/007/008). Empty batches are a no-op.
    pub fn apply_batch(
        &mut self,
        changes: &[launcher_domain::VerifiedChange],
    ) -> Result<BatchStats, IndexError> {
        if changes.is_empty() {
            let generation = self.generation()?;
            return Ok(BatchStats { generation, upserted: 0, deleted: 0 });
        }
        let tx = self.conn.transaction()?;
        let mut upserted = 0u64;
        let mut deleted = 0u64;
        for c in changes {
            match c {
                launcher_domain::VerifiedChange::Upsert(o) => {
                    // identity is the NORMALIZED path (P2.1 §31): drop any
                    // same-file rows stored under a different spelling
                    tx.execute(
                        "DELETE FROM files WHERE lower(path) = ?1 AND path != ?2",
                        params![o.normalized_path, o.path],
                    )?;
                    tx.execute(
                        "INSERT INTO files(path, name, parent, is_dir, size, modified_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                         ON CONFLICT(path) DO UPDATE SET
                           name = excluded.name,
                           parent = excluded.parent,
                           is_dir = excluded.is_dir,
                           size = excluded.size,
                           modified_ms = excluded.modified_ms",
                        params![o.path, file_name_of(&o.path), parent_of(&o.path),
                                o.is_dir as i64, o.size, o.modified_ms],
                    )?;
                    // P25-C01: FTS sync rides the SAME transaction (atomic
                    // with metadata; generation semantics untouched)
                    Self::fts_upsert(&tx, &o.path, &file_name_of(&o.path));
                    upserted += 1;
                }
                launcher_domain::VerifiedChange::Delete { normalized_path } => {
                    // FTS sync BEFORE the metadata delete (join by actual path)
                    let paths: Vec<String> = {
                        let mut stmt =
                            tx.prepare("SELECT path FROM files WHERE lower(path) = ?1")?;
                        let rows = stmt.query_map(params![normalized_path], |r| r.get(0))?;
                        rows.collect::<Result<Vec<_>, _>>()?
                    };
                    for p in &paths {
                        Self::fts_delete(&tx, p);
                    }
                    tx.execute(
                        "DELETE FROM files WHERE lower(path) = ?1",
                        params![normalized_path],
                    )?;
                    deleted += 1;
                }
            }
        }
        tx.execute(
            "INSERT INTO index_meta(key, value) VALUES ('generation', '1')
             ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
            [],
        )?;
        tx.commit()?;
        Ok(BatchStats {
            generation: self.generation()?,
            upserted,
            deleted,
        })
    }

    /// Bounded subtree recovery/rescan (review 76 §16-19): delete everything
    /// under `root` in the index, re-scan the REAL subtree (reparse points
    /// not followed), commit with one generation bump. NOT a full rebuild.
    pub fn rescan_root(&mut self, root: &Path) -> Result<usize, IndexError> {
        let norm = launcher_domain::normalize_path_identity(&root.to_string_lossy());
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM files WHERE lower(path) = ?1 OR lower(path) LIKE ?2",
            params![norm, format!("{norm}\\%")],
        )?;
        let mut remaining = MAX_INDEX_ENTRIES;
        scan_into(&tx, root, 0, &mut remaining)?;
        let count = MAX_INDEX_ENTRIES - remaining;
        tx.execute(
            "INSERT INTO index_meta(key, value) VALUES ('generation', '1')
             ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
            [],
        )?;
        tx.commit()?;
        Ok(count as usize)
    }

    /// First indexed original path whose normalized form starts with `root`.
    pub fn first_path_under(&self, root: &str) -> Result<Option<String>, IndexError> {
        let norm = launcher_domain::normalize_path_identity(root);
        let mut stmt = self.conn.prepare_cached(
            "SELECT path FROM files WHERE lower(path) = ?1 OR lower(path) LIKE ?2 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![norm, format!("{norm}\\%")])?;
        if let Some(r) = rows.next()? {
            return Ok(Some(r.get(0)?));
        }
        Ok(None)
    }

    /// All indexed paths under `root` (normalized prefix match) — test/diag.
    pub fn paths_under(&self, root: &Path) -> Result<Vec<String>, IndexError> {
        let norm = launcher_domain::normalize_path_identity(&root.to_string_lossy());
        let mut stmt = self.conn.prepare_cached(
            "SELECT path FROM files WHERE lower(path) = ?1 OR lower(path) LIKE ?2 ORDER BY path",
        )?;
        let rows = stmt.query_map(params![norm, format!("{norm}\\%")], |r| r.get(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Monotonic index generation: +1 per completed rebuild/rescan.
    pub fn generation(&self) -> Result<u64, IndexError> {
        // no row yet = generation 0 (index never rebuilt)
        let g: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = 'generation'",
                [],
                |r| r.get(0),
            )
            .ok();
        Ok(g.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    pub fn status(&self) -> Result<(i64,), IndexError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))?;
        Ok((n,))
    }

    /// Bounded execution history for recency signals.
    pub fn record_use(
        &self,
        command_id: &str,
        provider_id: &str,
        now_ms: u64,
    ) -> Result<(), IndexError> {
        // P2-FIX-03: delegate ONLY — the previous version inserted twice.
        self.record_use_titled(command_id, provider_id, "", now_ms)
    }

    /// Like [`record_use`], but stores the command title for recency views.
    pub fn record_use_titled(
        &self,
        command_id: &str,
        provider_id: &str,
        title: &str,
        now_ms: u64,
    ) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT INTO history(command_id, provider_id, title, used_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![command_id, provider_id, title, now_ms as i64],
        )?;
        self.conn.execute(
            "DELETE FROM history WHERE id NOT IN
             (SELECT id FROM history ORDER BY used_at_ms DESC LIMIT 10000)",
            [],
        )?;
        Ok(())
    }

    /// Aggregated per-command usage: (count, last_used_ms) for ranking boost.
    pub fn usage_map(&self) -> Result<Vec<UsageEntry>, IndexError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT command_id, provider_id, COUNT(*), MAX(used_at_ms)
             FROM history GROUP BY command_id, provider_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(UsageEntry {
                command_id: row.get(0)?,
                provider_id: row.get(1)?,
                uses: row.get::<_, i64>(2)? as u32,
                last_used_ms: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

/// One aggregated usage row (see [`Indexer::usage_map`]).
#[derive(Debug, Clone)]
pub struct UsageEntry {
    pub command_id: String,
    pub provider_id: String,
    pub uses: u32,
    pub last_used_ms: i64,
}

fn file_name_of(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

fn parent_of(p: &str) -> String {
    std::path::Path::new(p)
        .parent()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Batch commit statistics (review 76 §27).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchStats {
    pub generation: u64,
    pub upserted: u64,
    pub deleted: u64,
}

fn escape_like(s: String) -> String {
    s.chars()
        .flat_map(|c| {
            if ['%', '_', '\\'].contains(&c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

/// P2-FIX-01: the entry budget is GLOBAL across the whole rebuild —
/// `remaining` is shared by every recursion level and every root, so the
/// total insert count can never exceed `MAX_INDEX_ENTRIES` regardless of
/// directory shape (the old per-recursion local counter allowed N×limit).
fn scan_into(
    tx: &rusqlite::Transaction,
    root: &Path,
    depth: u32,
    remaining: &mut i64,
) -> Result<(), IndexError> {
    if depth > 8 || *remaining <= 0 {
        return Ok(());
    }
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        // permission-denied or vanished directories are skipped, not fatal
        Err(e) => {
            warn!(?root, error = %e, "scan skipped directory");
            return Ok(());
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let size = if is_dir { 0 } else { meta.len() as i64 };
        let path_str = path.to_string_lossy().to_string();
        let parent = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if *remaining <= 0 {
            warn!("global index entry limit reached");
            return Ok(());
        }
        tx.execute(
            "INSERT OR IGNORE INTO files(path, name, parent, is_dir, size, modified_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![path_str, name, parent, is_dir as i64, size, modified_ms],
        )?;
        *remaining -= 1;
        if is_dir && is_junction_like(&path) {
            // P2: reparse points (junctions/symlinks/mount points) are NOT
            // followed — depth bounds do not protect against a junction
            // pointing at a huge foreign tree (e.g. C:\Windows).
            continue;
        }
        if is_dir {
            scan_into(tx, &path, depth + 1, remaining)?;
        }
    }
    Ok(())
}

/// True when the path carries a reparse point (junction / symlink / mount).
/// Never follows it — the safe Windows-indexer default.
fn is_junction_like(p: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        match std::fs::symlink_metadata(p) {
            Ok(m) => m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = p;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree(dir: &Path) {
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("report.txt"), "hello").unwrap();
        std::fs::write(dir.join("报告.txt"), "unicode").unwrap();
        std::fs::write(dir.join("docs").join("report_final.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.join(".private")).unwrap();
        std::fs::write(dir.join(".private").join("secret.txt"), "s").unwrap();
    }

    #[test]
    fn index_and_search() {
        let tmp = std::env::temp_dir().join(format!("idx-test-{}", std::process::id()));
        let root = tmp.join("root");
        make_tree(&root);
        let mut idx = Indexer::in_memory().unwrap();
        let n = idx.rebuild(std::slice::from_ref(&root)).unwrap();
        assert!(n >= 4);

        let hits = idx.search("report", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits
            .iter()
            .all(|h| h.name.to_lowercase().contains("report")));

        // metadata present
        let f = &hits[0];
        assert!(f.size >= 0 && f.modified_ms > 0);

        // unicode
        assert_eq!(idx.search("报告", 10).unwrap().len(), 1);
        // hidden dirs skipped
        assert_eq!(idx.search("secret", 10).unwrap().len(), 0);
        // empty query -> nothing
        assert!(idx.search("", 10).unwrap().is_empty());
        // like-escape safety
        assert!(idx.search("%", 10).unwrap().is_empty());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn search_is_bounded() {
        let tmp = std::env::temp_dir().join(format!("idx-bounded-{}", std::process::id()));
        let root = tmp.join("root");
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..100 {
            std::fs::write(root.join(format!("file{i:03}.txt")), "x").unwrap();
        }
        let mut idx = Indexer::in_memory().unwrap();
        idx.rebuild(&[root]).unwrap();
        assert_eq!(idx.search("file", 25).unwrap().len(), 25);
        assert_eq!(idx.status().unwrap().0, 100);
        std::fs::remove_dir_all(&tmp).ok();
    }
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    /// P2-D: usage rows aggregate per (provider, command) and feed ranking.
    #[test]
    fn record_and_aggregate_usage() {
        let idx = Indexer::in_memory().unwrap();
        idx.record_use_titled("chrome", "apps", "Google Chrome", 1000).unwrap();
        idx.record_use_titled("chrome", "apps", "Google Chrome", 2000).unwrap();
        idx.record_use_titled("note", "apps", "Notepad", 3000).unwrap();
        let mut rows = idx.usage_map().unwrap();
        rows.sort_by(|a, b| a.command_id.cmp(&b.command_id));
        assert_eq!(rows.len(), 2);
        let chrome = rows.iter().find(|r| r.command_id == "chrome").unwrap();
        assert_eq!(chrome.uses, 2);
        assert_eq!(chrome.last_used_ms, 2000);
    }
}


#[cfg(test)]
mod path_budget_tests {
    use super::*;

    /// P2-FIX-04: users search path fragments, not only file names.
    #[test]
    fn path_fragment_matches() {
        let mut idx = Indexer::in_memory().unwrap();
        let root = std::env::temp_dir().join("nl_idx_path_test");
        let dir = root.join("Projects").join("Foo2026");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("readme.md"), "x").unwrap();
        idx.rebuild(&[root.clone()]).unwrap();
        // "foo2026" appears only in the PATH, not in any file name
        let hits = idx.search("foo2026", 10).unwrap();
        assert!(hits.iter().any(|f| f.name == "readme.md"), "path-only match must surface");
        std::fs::remove_dir_all(&root).ok();
    }

    /// P2-FIX-01: the entry budget is global — many roots cannot exceed it
    /// (the old per-recursion counter allowed N x limit).
    #[test]
    fn global_entry_budget_enforced() {
        let mut idx = Indexer::in_memory().unwrap();
        let roots: Vec<PathBuf> = (0..3)
            .map(|i| {
                let d = std::env::temp_dir().join(format!("nl_idx_budget_{i}"));
                std::fs::create_dir_all(&d).unwrap();
                std::fs::write(d.join("f.txt"), "x").unwrap();
                d
            })
            .collect();
        let n = idx.rebuild(&roots).unwrap();
        assert_eq!(n, 3, "normal rebuild counts every entry exactly once");
        for r in &roots {
            std::fs::remove_dir_all(r).ok();
        }
    }
}

#[cfg(test)]
mod generation_tests {
    use super::*;

    /// review 72 §25/26: generation is reserved index metadata — monotonic
    /// across rebuilds, 0 before the first one.
    #[test]
    fn generation_bumps_per_rebuild() {
        let mut idx = Indexer::in_memory().unwrap();
        assert_eq!(idx.generation().unwrap(), 0);
        let root = std::env::temp_dir().join("nl_idx_gen");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.txt"), "x").unwrap();
        idx.rebuild(&[root.clone()]).unwrap();
        assert_eq!(idx.generation().unwrap(), 1);
        idx.rebuild(&[root.clone()]).unwrap();
        assert_eq!(idx.generation().unwrap(), 2);
        std::fs::remove_dir_all(root).ok();
    }
}

#[cfg(test)]
mod corruption_recovery_tests {
    use super::*;

    /// C6/C7: a corrupt index.db is detected at open time, deleted, and
    /// recreated fresh — never a startup failure.
    #[test]
    fn corrupt_db_recovered_on_open() {
        let dir = std::env::temp_dir().join(format!("nl_idx_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("index.db");
        std::fs::write(&db, b"this is not a sqlite database at all").unwrap();

        let ix = Indexer::open(&db).unwrap();
        // after recovery, the index works and is empty (rebuildable)
        assert_eq!(ix.status().unwrap().0, 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
