//! Persistent Application Catalog (P2.1-D.1, review 82 §四/§五).
//!
//! Responsibility split (frozen by user directive):
//! - Discovery aggregation (AppRegistryProvider + packaged + portable) stays
//!   EXACTLY as-is — it is stable and must not be rewritten.
//! - This module is the PERSISTENT CATALOG layer: it materializes the merged
//!   entries into SQLite (`catalog.db`) with a `ApplicationCatalogGeneration`
//!   so host/health/diagnostics can observe catalog versions, and so future
//!   SearchProvider v2 can read the catalog instead of re-discovering.
//!
//! INV-APP-002 (identity merge) stays in the aggregation layer;
//! INV-APP-008 (single writer) applies here: only this store writes catalog
//! tables. This is NOT a search-time dependency — discovery failing must not
//! break the app provider (review 78 §33/79 §33).

use rusqlite::{params, Connection};
pub const CURRENT_SCHEMA_VERSION: i64 = 1;

pub struct CatalogStore {
    conn: Mutex<Connection>,
}

use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
}

/// One persisted catalog row (flattened view of an AppEntry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRecord {
    pub identity_key: String,
    pub display_name: String,
    pub source: String,
    pub launch_path: String,
}

impl CatalogStore {
    /// C6: open with corruption recovery — a corrupt catalog.db is deleted
    /// and recreated fresh (the catalog is rebuildable from discovery).
    pub fn open(db_path: &Path) -> Result<Self, CatalogError> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // C6: pre-open corruption check — delete files that can never be
        // valid SQLite (wrong magic bytes or not a file at all).
        if db_path.exists() {
            let header = std::fs::read(db_path)
                .map(|b| b[..16.min(b.len())].to_vec())
                .unwrap_or_default();
            let is_sqlite = header.starts_with(b"SQLite format 3\0");
            let is_empty = header.is_empty();
            if !is_sqlite && !is_empty {
                tracing::warn!(path = %db_path.display(), "catalog db corrupt — deleting and recreating");
                let _ = std::fs::remove_file(db_path);
            }
        }
        let conn = Self::open_conn(db_path).or_else(|first| {
            // C6: truncated/structurally-corrupt files can pass the 16-byte
            // magic check yet fail at open/schema time — treat any failure
            // on an EXISTING file as corruption: delete (incl. WAL/SHM) and
            // retry once. Only the retry result is surfaced.
            tracing::warn!(path = %db_path.display(), error = %first, "catalog db failed to open — deleting and recreating");
            for suffix in ["", "-wal", "-shm"] {
                let p = db_path.to_string_lossy() + suffix;
                let _ = std::fs::remove_file(p.as_ref());
            }
            Self::open_conn(db_path)
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn open_conn(db_path: &Path) -> Result<Connection, CatalogError> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS catalog_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS application (
                identity_key  TEXT PRIMARY KEY,
                display_name  TEXT NOT NULL,
                source        TEXT NOT NULL,
                launch_path   TEXT NOT NULL,
                updated_at    INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(conn)
    }

    /// ApplicationCatalogGeneration: +1 per successful reconcile commit
    /// (review 78 §34/§82 — same commit semantics as the file index).
    pub fn generation(&self) -> Result<u64, CatalogError> {
        let conn = self.conn.lock().expect("catalog lock");
        let g: Option<String> = conn
            .query_row(
                "SELECT value FROM catalog_meta WHERE key = 'generation'",
                [],
                |r| r.get(0),
            )
            .ok();
        Ok(g.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    /// Replace the whole catalog in ONE transaction and bump the generation
    /// only on commit. `entries` = (identity_key, display_name, source,
    /// launch_path). Returns (generation, rows_written).
    pub fn reconcile(&self, entries: &[(String, String, String, String)]) -> Result<(u64, usize), CatalogError> {
        {
            // scope the guard: generation() re-locks after commit
            let mut conn = self.conn.lock().expect("catalog lock");
            let tx = conn.transaction()?;
        tx.execute("DELETE FROM application", [])?;
        for (id, name, source, launch) in entries {
            tx.execute(
                "INSERT OR REPLACE INTO application
                 (identity_key, display_name, source, launch_path, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, name, source, launch, now_ms()],
            )?;
        }
        tx.execute(
            "INSERT INTO catalog_meta(key, value) VALUES ('generation', '1')
             ON CONFLICT(key) DO UPDATE SET
               value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
            [],
        )?;
        tx.commit()?;
        }
        Ok((self.generation()?, entries.len()))
    }

    /// All catalog rows, deterministic order (diagnostics/future provider).
    pub fn list(&self) -> Result<Vec<CatalogRecord>, CatalogError> {
        let conn = self.conn.lock().expect("catalog lock");
        let mut stmt = conn
            .prepare_cached("SELECT identity_key, display_name, source, launch_path
                            FROM application ORDER BY identity_key")?;
        let rows = stmt.query_map([], |r| {
            Ok(CatalogRecord {
                identity_key: r.get(0)?,
                display_name: r.get(1)?,
                source: r.get(2)?,
                launch_path: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> CatalogStore {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_cat_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        CatalogStore::open(&dir.join("catalog.db")).unwrap()
    }

    /// §34/§53: reconcile is replace-all + generation bump, committed
    /// atomically.
    #[test]
    fn reconcile_replaces_and_bumps_generation() {
        let s = store();
        assert_eq!(s.generation().unwrap(), 0);
        let rows = vec![
            ("win32:c:/chrome/chrome.exe".into(), "Chrome".into(), "start-menu".into(), "c:/chrome/chrome.exe".into()),
            ("packaged:calc_8wek:app".into(), "Calculator".into(), "packaged".into(), r"shell:AppsFolder\calc_8wek!app".into()),
        ];
        let (gen, n) = s.reconcile(&rows).unwrap();
        assert_eq!(gen, 1);
        assert_eq!(n, 2);
        // second reconcile replaces (no duplicates)
        let (gen2, n2) = s.reconcile(&rows[..1]).unwrap();
        assert_eq!(gen2, 2);
        assert_eq!(n2, 1);
        let all = s.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].identity_key, "win32:c:/chrome/chrome.exe");
    }
}

#[cfg(test)]
mod corruption_tests {
    use super::*;

    /// C6: a corrupt catalog.db is detected at open time and recreated
    /// fresh (generation=0, empty table) — the catalog is rebuildable.
    #[test]
    fn corrupt_catalog_db_recovered() {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_cat_corrupt_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("catalog.db");
        std::fs::write(&db, b"NOT SQLITE AT ALL").unwrap();

        let store = CatalogStore::open(&db).unwrap();
        assert_eq!(store.generation().unwrap(), 0);
        let (gen, cnt) = store
            .reconcile(&[("k".into(), "N".into(), "start-menu".into(), "p".into())])
            .unwrap();
        assert_eq!(gen, 1);
        assert_eq!(cnt, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}

