//! Persistent Application Catalog 2.0 (P2.4-A02/A03/A04,
//! spec `01-P2.4-DESIGN-SPEC.md` §4; supersedes the P2.1-D.1 v1 store).
//!
//! Responsibility split (unchanged, frozen by user directive):
//! - Discovery aggregation (AppRegistryProvider + packaged + portable) stays
//!   EXACTLY as-is — observations are produced by it, never rewritten here.
//! - This module is the PERSISTENT CATALOG layer: it normalizes
//!   [`ApplicationObservation`]s into canonical identities (via
//!   `app_identity::canonical_identity`, pure), merges same-identity rows,
//!   persists lifecycle metadata and a commit-gated generation.
//!
//! Invariants (spec §4.4/§4.5):
//! - INV-APP-008 single writer: only this store writes catalog tables.
//! - One reconcile = ONE transaction; generation +1 only after commit;
//!   a failed reconcile leaves the previous committed catalog queryable.
//! - Lifecycle (`ready`/`stale`/`broken`) is METADATA — it never grants or
//!   retains execution authority by itself (A04: "metadata != authority").
//! - Re-observation resets an entry to `ready` (fresh evidence); stale/broken
//!   survive only while the entry is not re-observed.

use crate::app_identity::{canonical_identity, ApplicationObservation};
use rusqlite::{params, Connection};
pub const CURRENT_SCHEMA_VERSION: i64 = 2;

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

/// A03: intermediate merge row — one canonical identity with all observing
/// sources, built deterministically before the transaction opens.
#[derive(Clone)]
struct Merged {
    identity: String,
    display_name: String,
    source: String,
    launch_path: String,
    sources: Vec<String>,
    entry_path: String,
}

/// Catalog lifecycle (spec §4.4). Metadata states, never authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CatalogLifecycle {
    #[default]
    Ready,
    Stale,
    Broken,
}

impl CatalogLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            CatalogLifecycle::Ready => "ready",
            CatalogLifecycle::Stale => "stale",
            CatalogLifecycle::Broken => "broken",
        }
    }

    fn parse(s: &str) -> CatalogLifecycle {
        match s {
            "stale" => CatalogLifecycle::Stale,
            "broken" => CatalogLifecycle::Broken,
            _ => CatalogLifecycle::Ready,
        }
    }
}

/// One persisted catalog row: a merged, canonical application entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRecord {
    pub identity_key: String,
    pub display_name: String,
    pub source: String,
    pub launch_path: String,
    pub lifecycle: CatalogLifecycle,
    /// All source_ids that observed this identity (deterministic order).
    pub sources: Vec<String>,
    /// Original discovery entry path (e.g. the .lnk). This is what keeps
    /// command_ids stable across catalog rebuilds (A05 stable-id rule).
    pub entry_path: String,
}

impl CatalogStore {
    /// C6: open with corruption recovery + v1→v2 migration. A corrupt
    /// catalog.db is deleted and recreated fresh (the catalog is rebuildable
    /// from discovery).
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
                updated_at    INTEGER NOT NULL,
                lifecycle     TEXT NOT NULL DEFAULT 'ready',
                sources       TEXT NOT NULL DEFAULT '[]',
                aumid         TEXT,
                package_identity TEXT,
                entry_path    TEXT NOT NULL DEFAULT ''
            );
            "#,
        )?;
        Self::migrate(&conn)?;
        Ok(conn)
    }

    /// Schema migration (A02): detects a v1 database by the ABSENCE of the
    /// `lifecycle` column (PRAGMA, not a version meta row — v1 predates it)
    /// and adds the v2 columns additively. Idempotent; a v1 database keeps
    /// every row, its generation and its committed content.
    fn migrate(conn: &Connection) -> Result<(), CatalogError> {
        let has_lifecycle = conn
            .prepare("PRAGMA table_info(application)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .any(|c| c.as_deref() == Ok("lifecycle"));
        if !has_lifecycle {
            tracing::info!(from = 1, to = 2, "catalog schema migration");
            conn.execute_batch(
                r#"
                ALTER TABLE application ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'ready';
                ALTER TABLE application ADD COLUMN sources TEXT NOT NULL DEFAULT '[]';
                ALTER TABLE application ADD COLUMN aumid TEXT;
                ALTER TABLE application ADD COLUMN package_identity TEXT;
                ALTER TABLE application ADD COLUMN entry_path TEXT NOT NULL DEFAULT '';
                UPDATE application SET sources = JSON_ARRAY(source) WHERE sources = '[]';
                UPDATE application SET entry_path = launch_path WHERE entry_path = '';
                "#,
            )?;
        }
        conn.execute(
            "INSERT INTO catalog_meta(key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![CURRENT_SCHEMA_VERSION.to_string()],
        )?;
        Ok(())
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
    /// Convenience wrapper over [`Self::reconcile_observations`] for v1-style
    /// pre-identity callers (recovery tests, diagnostics).
    pub fn reconcile(
        &self,
        entries: &[(String, String, String, String)],
    ) -> Result<(u64, usize), CatalogError> {
        let mut merged: Vec<Merged> = entries
            .iter()
            .map(|(id, name, source, launch)| Merged {
                identity: id.clone(),
                display_name: name.clone(),
                source: source.clone(),
                launch_path: launch.clone(),
                sources: vec![source.clone()],
                entry_path: launch.clone(),
            })
            .collect();
        merged.sort_by(|a, b| a.identity.cmp(&b.identity));
        self.commit_merged(&merged)
    }

    /// A03 — reconcile from raw observations: canonical identities are
    /// computed purely, rows are merged deterministically (sort by identity
    /// then source_id), same-identity observations merge their source lists,
    /// absent observations are deleted, and re-observed entries return to
    /// `ready`. Single transaction; generation +1 only on commit.
    pub fn reconcile_observations(
        &self,
        obs: &[ApplicationObservation],
    ) -> Result<(u64, usize), CatalogError> {
        let mut merged: Vec<Merged> = Vec::new();
        let mut with_identity: Vec<(String, &ApplicationObservation)> =
            obs.iter().map(|o| (canonical_identity(o), o)).collect();
        with_identity.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.source_id.cmp(&b.1.source_id)));
        for (identity, o) in with_identity {
            let launch_path = o
                .launch_target
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| o.identity_hint.clone());
            if let Some(last) = merged.last_mut().filter(|m| m.identity == identity) {
                // same identity from another source: merge provenance, keep
                // the deterministic-first display/source (sorted order)
                if !last.sources.contains(&o.source_id) {
                    last.sources.push(o.source_id.clone());
                }
            } else {
                merged.push(Merged {
                    identity,
                    display_name: o.display_name.clone(),
                    source: o.source_id.clone(),
                    launch_path,
                    sources: vec![o.source_id.clone()],
                    // the source-scoped hint IS the original entry path
                    entry_path: o.identity_hint.clone(),
                });
            }
        }
        self.commit_merged(&merged)
    }

    fn commit_merged(&self, merged: &[Merged]) -> Result<(u64, usize), CatalogError> {
        {
            let mut conn = self.conn.lock().expect("catalog lock");
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM application", [])?;
            for m in merged {
                let sources =
                    serde_json::to_string(&m.sources).unwrap_or_else(|_| "[]".into());
                tx.execute(
                    "INSERT OR REPLACE INTO application
                     (identity_key, display_name, source, launch_path, updated_at, lifecycle, sources, entry_path)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'ready', ?6, ?7)",
                    params![
                        m.identity,
                        m.display_name,
                        m.source,
                        m.launch_path,
                        now_ms(),
                        sources,
                        m.entry_path
                    ],
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
        Ok((self.generation()?, merged.len()))
    }

    /// A04 — lifecycle is a metadata transition (ready/stale/broken). It
    /// changes presentation/diagnostics only; it can never grant authority.
    pub fn set_lifecycle(
        &self,
        identity_key: &str,
        lifecycle: CatalogLifecycle,
    ) -> Result<(), CatalogError> {
        let conn = self.conn.lock().expect("catalog lock");
        conn.execute(
            "UPDATE application SET lifecycle = ?2, updated_at = ?3 WHERE identity_key = ?1",
            params![identity_key, lifecycle.as_str(), now_ms()],
        )?;
        Ok(())
    }

    /// All catalog rows, deterministic order.
    pub fn list(&self) -> Result<Vec<CatalogRecord>, CatalogError> {
        let conn = self.conn.lock().expect("catalog lock");
        let mut stmt = conn
            .prepare_cached("SELECT identity_key, display_name, source, launch_path, lifecycle, sources, entry_path
                            FROM application ORDER BY identity_key")?;
        let rows = stmt.query_map([], |r| {
            let lifecycle: String = r.get(4)?;
            let sources_json: String = r.get(5)?;
            Ok(CatalogRecord {
                identity_key: r.get(0)?,
                display_name: r.get(1)?,
                source: r.get(2)?,
                launch_path: r.get(3)?,
                lifecycle: CatalogLifecycle::parse(&lifecycle),
                sources: serde_json::from_str(&sources_json).unwrap_or_default(),
                entry_path: r.get(6)?,
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
    use crate::app_identity::ApplicationObservation;

    fn store() -> CatalogStore {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_cat_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        CatalogStore::open(&dir.join("catalog.db")).unwrap()
    }

    fn obs(source: &str, target: &str, name: &str) -> ApplicationObservation {
        ApplicationObservation {
            source_id: source.into(),
            source_kind: source.into(),
            identity_hint: format!("{source}:hint"),
            display_name: name.into(),
            launch_target: Some(target.into()),
            observed_at: 1,
            ..Default::default()
        }
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

    /// A03: same identity observed by two sources merges into ONE row with
    /// both sources listed, deterministically.
    #[test]
    fn same_identity_from_multiple_sources_merges() {
        let s = store();
        let (gen, n) = s
            .reconcile_observations(&[
                obs("start-menu", r"C:\Tools\App.exe", "App"),
                obs("uninstall", r"C:\Tools\App.exe", "App"),
            ])
            .unwrap();
        assert_eq!(n, 1, "same exe from two sources collapses to one entry");
        assert_eq!(gen, 1);
        let all = s.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].sources.len(), 2, "both sources preserved as provenance");
        // deterministic: reconciling again in swapped order gives same row
        s.reconcile_observations(&[
            obs("uninstall", r"C:\Tools\App.exe", "App"),
            obs("start-menu", r"C:\Tools\App.exe", "App"),
        ])
        .unwrap();
        assert_eq!(s.list().unwrap(), all);
    }

    /// A03: a source entry that disappears is removed; changed metadata is
    /// updated; generation advances once per committed reconcile.
    #[test]
    fn missing_entry_removed_and_metadata_updated() {
        let s = store();
        s.reconcile_observations(&[
            obs("start-menu", r"C:\A\App.exe", "App"),
            obs("start-menu", r"C:\B\Old.exe", "Old Name"),
        ])
        .unwrap();
        let (gen, n) = s
            .reconcile_observations(&[
                obs("start-menu", r"C:\A\App.exe", "App Renamed"),
                obs("start-menu", r"C:\C\New.exe", "New"),
            ])
            .unwrap();
        assert_eq!(gen, 2);
        assert_eq!(n, 2);
        let all = s.list().unwrap();
        let renamed = all.iter().find(|r| r.identity_key.contains(r"a\app.exe")).unwrap();
        assert_eq!(renamed.display_name, "App Renamed", "changed metadata updated");
        assert!(all.iter().all(|r| !r.launch_path.contains("Old.exe")), "missing source entry removed");
    }

    /// A04: lifecycle transitions are metadata; set_lifecycle only touches
    /// presentation state and re-observation resets to ready.
    #[test]
    fn lifecycle_is_metadata_and_resets_on_reobservation() {
        let s = store();
        s.reconcile_observations(&[obs("start-menu", r"C:\A\App.exe", "App")])
            .unwrap();
        s.set_lifecycle(
            &s.list().unwrap()[0].identity_key,
            CatalogLifecycle::Stale,
        )
        .unwrap();
        assert_eq!(s.list().unwrap()[0].lifecycle, CatalogLifecycle::Stale);
        // re-observation = fresh evidence → back to ready
        s.reconcile_observations(&[obs("start-menu", r"C:\A\App.exe", "App")])
            .unwrap();
        assert_eq!(s.list().unwrap()[0].lifecycle, CatalogLifecycle::Ready);
    }

    /// A02: migration from a v1 database (v1 schema rows present, no
    /// schema_version meta) keeps every row and its generation.
    #[test]
    fn migration_from_v1_preserves_rows_and_generation() {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_cat_mig_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("catalog.db");

        // craft a v1 database by hand (v1 schema + meta, no lifecycle column)
        {
            let conn = Connection::open(&db).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE catalog_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                CREATE TABLE application (
                    identity_key  TEXT PRIMARY KEY,
                    display_name  TEXT NOT NULL,
                    source        TEXT NOT NULL,
                    launch_path   TEXT NOT NULL,
                    updated_at    INTEGER NOT NULL
                );
                INSERT INTO application VALUES ('win32:c:/a.exe', 'A', 'start-menu', 'c:/a.exe', 1);
                INSERT INTO catalog_meta VALUES ('generation', '7');
                "#,
            )
            .unwrap();
        }

        let store = CatalogStore::open(&db).unwrap();
        assert_eq!(store.generation().unwrap(), 7, "generation survives migration");
        let all = store.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].lifecycle, CatalogLifecycle::Ready, "v1 rows default ready");
        assert_eq!(all[0].sources, vec!["start-menu".to_string()], "v1 source backfilled");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A06 conformance: corruption resets lifecycle metadata to safe defaults
    /// (ready + fresh rebuild). Stale/broken are rebuildable metadata — they
    /// must never SURVIVE as authority-bearing residue after a rebuild.
    #[test]
    fn corrupt_rebuild_resets_lifecycle_to_ready() {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_cat_a06_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("catalog.db");
        {
            let s = CatalogStore::open(&db).unwrap();
            s.reconcile_observations(&[obs("start-menu", r"C:\A\App.exe", "App")])
                .unwrap();
            s.set_lifecycle(&s.list().unwrap()[0].identity_key, CatalogLifecycle::Stale)
                .unwrap();
        }
        std::fs::write(&db, b"NOT SQLITE AT ALL").unwrap();
        let s = CatalogStore::open(&db).unwrap();
        assert_eq!(s.generation().unwrap(), 0, "rebuild restarts generation");
        assert!(s.list().unwrap().is_empty(), "corrupt catalog rebuilds empty");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// §4.5: a failed reconcile (db locked by another writer) leaves the
    /// previous committed generation and catalog visible.
    #[test]
    fn failed_reconcile_preserves_previous_generation() {
        let dir = {
            static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let d = std::env::temp_dir().join(format!("nl_cat_fail_{}_{}", std::process::id(), n));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        let db = dir.join("catalog.db");
        let s = store_at(&db);
        s.reconcile_observations(&[obs("start-menu", r"C:\A\App.exe", "App")])
            .unwrap();
        let gen_before = s.generation().unwrap();

        // hold an exclusive write lock from a second connection, then
        // attempt a reconcile — the transaction must fail without commit
        let blocker = Connection::open(&db).unwrap();
        blocker.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let result = s.reconcile_observations(&[obs("start-menu", r"C:\B\Other.exe", "Other")]);
        blocker.execute_batch("ROLLBACK").unwrap();
        drop(blocker);
        assert!(result.is_err(), "reconcile under exclusive lock must fail");
        assert_eq!(s.generation().unwrap(), gen_before, "failed reconcile does not advance generation");
        assert_eq!(s.list().unwrap().len(), 1, "previous catalog remains queryable");
        std::fs::remove_dir_all(&dir).ok();
    }

    fn store_at(db: &Path) -> CatalogStore {
        CatalogStore::open(db).unwrap()
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
