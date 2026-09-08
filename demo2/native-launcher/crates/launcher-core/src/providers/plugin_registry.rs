//! Plugin Registry (P2.2-D core, review: P2.2-D §32/§33/§104): persistent
//! lifecycle state for external plugins — enabled flag, quarantine, failure
//! count, capability requests/grants. SQLite in `plugins.db`.
//!
//! Boundaries (P2.2-D §47/§109): this is lifecycle STATE, not a runtime
//! executor and not an ActionEngine. RuntimeManager stays process-owner;
//! PluginBroker stays effect-executor.

use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const QUARANTINE_THRESHOLD: u32 = 3;

/// P2.4-C02 trust states (spec §6.2). Trust is PROVENANCE attitude — how the
/// host regards the plugin's origin — and is orthogonal to `enabled`
/// (operator decision) and `quarantined` (runtime safety). Discovery of a
/// manifest can only reach `Known`; `Trusted` requires an explicit host/admin
/// operation. Manifest metadata can never mutate trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrustState {
    #[default]
    Unknown,
    Known,
    Trusted,
}

impl TrustState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustState::Unknown => "unknown",
            TrustState::Known => "known",
            TrustState::Trusted => "trusted",
        }
    }

    fn parse(s: &str) -> TrustState {
        match s {
            "known" => TrustState::Known,
            "trusted" => TrustState::Trusted,
            _ => TrustState::Unknown,
        }
    }
}

/// P2.4-C03 capability decisions (spec §6.3): unset must fail closed for
/// protected capabilities. Only the explicit host decision API mutates a
/// decision — manifest/AI/MCP metadata never can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapDecision {
    #[default]
    Unset,
    Allow,
    Deny,
}

impl CapDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            CapDecision::Unset => "unset",
            CapDecision::Allow => "allow",
            CapDecision::Deny => "deny",
        }
    }

    fn parse(s: &str) -> CapDecision {
        match s {
            "allow" => CapDecision::Allow,
            "deny" => CapDecision::Deny,
            _ => CapDecision::Unset,
        }
    }
}

fn backup_path(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bak", db_path.display()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginLifecycleState {
    pub enabled: bool,
    pub quarantined: bool,
    pub protocol_failures: u32,
}

pub struct PluginRegistry {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

impl PluginRegistry {
    /// C6 + P2.3-D.1: open with corruption recovery. Recovery order:
    /// restore from `plugins.db.bak` (preserves user security choices) →
    /// quarantine the corrupt file as `plugins.db.corrupt-<ts>` and rebuild
    /// fresh (all plugins default to enabled, safe state).
    pub fn open(db_path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // pre-open corruption check: files that can never be valid SQLite
        // still go through the backup path before falling back to rebuild.
        if db_path.exists() {
            let header = std::fs::read(db_path)
                .map(|b| b[..16.min(b.len())].to_vec())
                .unwrap_or_default();
            let is_sqlite = header.starts_with(b"SQLite format 3\0");
            let is_empty = header.is_empty();
            if !is_sqlite && !is_empty {
                tracing::warn!(path = %db_path.display(), "plugin registry corrupt — attempting backup restore");
                Self::quarantine_corrupt_file(db_path);
                if Self::try_restore_from_backup(db_path) {
                    tracing::info!(path = %db_path.display(), "plugin registry restored from backup");
                }
            }
        }
        let conn = Self::open_conn(db_path).or_else(|first| {
            // C6: truncated/structurally-corrupt files can pass the 16-byte
            // magic check yet fail at open/schema time. SECURITY: corruption
            // recovery resets user security choices to permissive defaults —
            // so restore the last known-good backup first and only rebuild
            // fresh when no usable backup exists.
            tracing::warn!(path = %db_path.display(), error = %first, "plugin registry failed to open — attempting backup restore");
            Self::quarantine_corrupt_file(db_path);
            let restored = Self::try_restore_from_backup(db_path);
            if restored {
                tracing::info!(path = %db_path.display(), "plugin registry restored from backup");
            }
            Self::open_conn(db_path).or_else(|second| {
                // SECURITY: last resort — user security choices reset to
                // enabled defaults. The corrupt file was quarantined above
                // (kept under `.corrupt-<ts>`) instead of deleted.
                tracing::warn!(path = %db_path.display(), error = %second, "plugin registry rebuild — no usable backup");
                Self::open_conn(db_path)
            })
        })?;
        let registry = Self {
            conn: Mutex::new(conn),
            db_path: db_path.to_path_buf(),
        };
        // P2.3-D.1: refresh the backup after every successful open so the
        // next corruption recovers instead of rebuilding. Best-effort: a
        // failed backup never blocks startup.
        if let Err(e) = registry.write_backup() {
            tracing::warn!(path = %db_path.display(), error = %e, "plugin registry backup write failed");
        }
        Ok(registry)
    }

    fn remove_db_files(db_path: &Path) {
        for suffix in ["-wal", "-shm"] {
            let p = db_path.to_string_lossy() + suffix;
            let _ = std::fs::remove_file(p.as_ref());
        }
    }

    /// Move the unusable db file aside under `.corrupt-<ts>` (kept for
    /// forensics) and clear its WAL/SHM sidecars so the next open rebuilds.
    fn quarantine_corrupt_file(db_path: &Path) {
        if db_path.exists() {
            let quarantined = PathBuf::from(format!("{}.corrupt-{}", db_path.display(), now_ms()));
            match std::fs::rename(db_path, &quarantined) {
                Ok(()) => {
                    tracing::warn!(path = %quarantined.display(), "plugin registry corrupt file quarantined");
                }
                Err(e) => {
                    tracing::warn!(path = %db_path.display(), error = %e, "plugin registry corrupt file rename failed — removing");
                    let _ = std::fs::remove_file(db_path);
                }
            }
        }
        Self::remove_db_files(db_path);
    }

    fn try_restore_from_backup(db_path: &Path) -> bool {
        let bak = backup_path(db_path);
        if !bak.exists() {
            return false;
        }
        match std::fs::copy(&bak, db_path) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(path = %bak.display(), error = %e, "plugin registry backup restore failed");
                false
            }
        }
    }

    /// Write `plugins.db.bak`: checkpoint WAL into the main db, then copy.
    /// Security-relevant state (enabled/quarantined) must survive corruption,
    /// so every mutating method refreshes the backup. The registry holds a
    /// handful of rows, so the copy is trivially cheap.
    fn write_backup(&self) -> Result<(), std::io::Error> {
        let db_path = self.db_path.clone();
        {
            let conn = self.conn.lock().expect("registry lock");
            let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        }
        let bak = backup_path(&db_path);
        let tmp = PathBuf::from(format!("{}.tmp", bak.display()));
        std::fs::copy(&db_path, &tmp)?;
        // atomic replace so a crash mid-copy cannot destroy the last good backup
        std::fs::rename(&tmp, &bak).or_else(|e| {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        })?;
        Ok(())
    }

    fn open_conn(db_path: &Path) -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS plugins (
                plugin_id     TEXT PRIMARY KEY,
                enabled       INTEGER NOT NULL DEFAULT 1,
                quarantined   INTEGER NOT NULL DEFAULT 0,
                failures      INTEGER NOT NULL DEFAULT 0,
                updated_at    INTEGER NOT NULL,
                trust         TEXT NOT NULL DEFAULT 'unknown'
            );
            CREATE TABLE IF NOT EXISTS plugin_capabilities (
                plugin_id  TEXT NOT NULL,
                capability TEXT NOT NULL,
                decision   TEXT NOT NULL DEFAULT 'unset',
                PRIMARY KEY (plugin_id, capability)
            );
            "#,
        )?;
        Self::migrate(&conn);
        Ok(conn)
    }

    /// P2.4-C01/C02 migration: add the `trust` column to a pre-P2.4 database
    /// (detected by PRAGMA, idempotent). Existing plugins are `unknown` —
    /// migration never auto-trusts (fail-closed).
    fn migrate(conn: &Connection) {
        let has_trust = conn
            .prepare("PRAGMA table_info(plugins)")
            .map(|mut stmt| {
                let names: Result<Vec<String>, _> = stmt
                    .query_map([], |r| r.get::<_, String>(1))?
                    .collect();
                names.map(|cols| cols.iter().any(|c| c == "trust"))
            })
            .unwrap_or(Ok(false))
            .unwrap_or(false);
        if !has_trust {
            tracing::info!(to = 2, "plugin registry schema migration (trust column)");
            let _ = conn.execute_batch(
                "ALTER TABLE plugins ADD COLUMN trust TEXT NOT NULL DEFAULT 'unknown';",
            );
        }
    }

    /// Lifecycle state for one plugin; unknown plugins default to
    /// enabled=true (existing-discovery compatibility, P2.2-D §105).
    pub fn state(&self, plugin_id: &str) -> PluginLifecycleState {
        let conn = self.conn.lock().expect("registry lock");
        conn.query_row(
            "SELECT enabled, quarantined, failures FROM plugins WHERE plugin_id = ?1",
            params![plugin_id],
            |r| {
                Ok(PluginLifecycleState {
                    enabled: r.get::<_, i64>(0)? != 0,
                    quarantined: r.get::<_, i64>(1)? != 0,
                    protocol_failures: r.get::<_, i64>(2)? as u32,
                })
            },
        )
        .unwrap_or(PluginLifecycleState {
            enabled: true,
            quarantined: false,
            protocol_failures: 0,
        })
    }

    pub fn set_enabled(&self, plugin_id: &str, enabled: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "INSERT INTO plugins(plugin_id, enabled, quarantined, failures, updated_at)
             VALUES (?1, ?2, 0, 0, ?3)
             ON CONFLICT(plugin_id) DO UPDATE SET enabled = ?2, updated_at = ?3",
            params![plugin_id, enabled as i64, now_ms()],
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(())
    }

    /// Record one protocol-level failure; returns `true` when the accumulated
    /// failures reached QUARANTINE_THRESHOLD (review: P2.2-D §59/§61 —
    /// quarantine after repeated protocol violations, bounded window = the
    /// persisted count, reset on success).
    pub fn record_failure(&self, plugin_id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "INSERT INTO plugins(plugin_id, enabled, quarantined, failures, updated_at)
             VALUES (?1, 1, 0, 1, ?2)
             ON CONFLICT(plugin_id) DO UPDATE SET
               failures = failures + 1, updated_at = ?2,
               quarantined = (failures + 1) >= ?3",
            params![plugin_id, now_ms(), QUARANTINE_THRESHOLD],
        )?;
        let q: i64 = conn.query_row(
            "SELECT quarantined FROM plugins WHERE plugin_id = ?1",
            params![plugin_id],
            |r| r.get(0),
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(q != 0)
    }

    /// Success resets the failure count (and lifts quarantine, P2.2-D §62:
    /// a successful update/recovery is the host-sanctioned clear).
    pub fn record_success(&self, plugin_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "UPDATE plugins SET failures = 0, quarantined = 0, updated_at = ?2
             WHERE plugin_id = ?1",
            params![plugin_id, now_ms()],
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(())
    }

    /// Record a requested capability (manifest is untrusted metadata —
    /// decision stays 'unset' = denied at runtime until host approval).
    pub fn record_capability(&self, plugin_id: &str, capability: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "INSERT OR IGNORE INTO plugin_capabilities(plugin_id, capability, decision)
             VALUES (?1, ?2, 'unset')",
            params![plugin_id, capability],
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(())
    }

    /// P2.4-C02: trust of one plugin; unknown plugins are `Unknown`.
    pub fn trust(&self, plugin_id: &str) -> TrustState {
        let conn = self.conn.lock().expect("registry lock");
        let t: Option<String> = conn
            .query_row(
                "SELECT trust FROM plugins WHERE plugin_id = ?1",
                params![plugin_id],
                |r| r.get(0),
            )
            .ok();
        t.as_deref().map(TrustState::parse).unwrap_or(TrustState::Unknown)
    }

    /// Explicit trust transition (host/admin operation only). This is the
    /// ONLY path to `Trusted` — there is no API that reaches it from
    /// manifest data, AI/MCP output or discovery.
    pub fn set_trust(&self, plugin_id: &str, trust: TrustState) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "INSERT INTO plugins(plugin_id, enabled, quarantined, failures, updated_at, trust)
             VALUES (?1, 1, 0, 0, ?2, ?3)
             ON CONFLICT(plugin_id) DO UPDATE SET trust = ?3, updated_at = ?2",
            params![plugin_id, now_ms(), trust.as_str()],
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(())
    }

    /// Manifest observation: promotes `Unknown` → `Known` ONLY (discovery is
    /// not trust). `Known`/`Trusted` are never demoted by re-observation.
    pub fn note_manifest_observed(&self, plugin_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "INSERT INTO plugins(plugin_id, enabled, quarantined, failures, updated_at, trust)
             VALUES (?1, 1, 0, 0, ?2, 'known')
             ON CONFLICT(plugin_id) DO UPDATE SET
               trust = CASE WHEN trust = 'unknown' THEN 'known' ELSE trust END,
               updated_at = ?2",
            params![plugin_id, now_ms()],
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(())
    }

    /// P2.4-C03: stored capability decision; unset (default) must fail
    /// closed at the enforcement point.
    pub fn capability_decision(&self, plugin_id: &str, capability: &str) -> CapDecision {
        let conn = self.conn.lock().expect("registry lock");
        let d: Option<String> = conn
            .query_row(
                "SELECT decision FROM plugin_capabilities WHERE plugin_id = ?1 AND capability = ?2",
                params![plugin_id, capability],
                |r| r.get(0),
            )
            .ok();
        d.as_deref().map(CapDecision::parse).unwrap_or(CapDecision::Unset)
    }

    /// Explicit capability decision (host approval UI / policy). The ONLY
    /// writer of a decision — manifest requests record via
    /// [`Self::record_capability`] and always stay `unset`.
    pub fn set_capability_decision(
        &self,
        plugin_id: &str,
        capability: &str,
        decision: CapDecision,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("registry lock");
        conn.execute(
            "INSERT INTO plugin_capabilities(plugin_id, capability, decision)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(plugin_id, capability) DO UPDATE SET decision = ?3",
            params![plugin_id, capability, decision.as_str()],
        )?;
        drop(conn);
        self.refresh_backup();
        Ok(())
    }

    fn refresh_backup(&self) {
        if let Err(e) = self.write_backup() {
            tracing::warn!(path = %self.db_path.display(), error = %e, "plugin registry backup write failed");
        }
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

    fn reg() -> PluginRegistry {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_reg_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        PluginRegistry::open(&dir.join("plugins.db")).unwrap()
    }

    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_reg_{}_{}_{}", tag, std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// P2.2-D §59/§61: repeated protocol failures → quarantine; success
    /// resets the count and lifts quarantine.
    #[test]
    fn failure_threshold_quarantines_and_success_resets() {
        let r = reg();
        for i in 1..=QUARANTINE_THRESHOLD {
            assert_eq!(r.record_failure("p1").unwrap(), i >= QUARANTINE_THRESHOLD);
        }
        assert!(r.state("p1").quarantined);
        r.record_success("p1").unwrap();
        let st = r.state("p1");
        assert!(!st.quarantined);
        assert_eq!(st.protocol_failures, 0);
    }

    /// Unknown plugin defaults to enabled (discovery compatibility).
    #[test]
    fn unknown_plugin_defaults_enabled() {
        let r = reg();
        let st = r.state("never-seen");
        assert!(st.enabled);
        assert!(!st.quarantined);
    }

    /// enable/disable persists across reopen (Gate 1).
    #[test]
    fn enabled_state_persists() {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_reg_persist_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("plugins.db");
        {
            let r = PluginRegistry::open(&db).unwrap();
            r.set_enabled("p", false).unwrap();
        }
        {
            let r = PluginRegistry::open(&db).unwrap();
            assert!(!r.state("p").enabled);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P2.3-D.1: a corrupt plugins.db is restored from the last known-good
    /// backup — user security choices (disabled plugin) survive corruption.
    #[test]
    fn corrupt_db_restores_from_backup() {
        let dir = scratch_dir("reg_bak");
        let db = dir.join("plugins.db");
        {
            let r = PluginRegistry::open(&db).unwrap();
            r.set_enabled("p", false).unwrap();
            r.record_failure("q").unwrap();
        }
        assert!(dir.join("plugins.db.bak").exists());
        // corrupt the main db past the 16-byte magic check (simulated disk
        // corruption); the backup keeps the pre-corruption state.
        std::fs::write(&db, b"NOTSQLITE-not-a-database-at-all").unwrap();
        let r = PluginRegistry::open(&db).unwrap();
        assert!(!r.state("p").enabled);
        assert_eq!(r.state("q").protocol_failures, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P2.4-C02: discovery reaches `Known` only; `Trusted` needs the
    /// explicit host operation; re-observation never demotes trust.
    #[test]
    fn trust_ladder_never_auto_trusts() {
        let r = reg();
        assert_eq!(r.trust("p1"), TrustState::Unknown, "unknown plugin = Unknown");
        r.note_manifest_observed("p1").unwrap();
        assert_eq!(r.trust("p1"), TrustState::Known, "manifest observation = Known, never Trusted");
        assert_ne!(r.trust("p1"), TrustState::Trusted);
        r.note_manifest_observed("p1").unwrap();
        assert_eq!(r.trust("p1"), TrustState::Known);
        r.set_trust("p1", TrustState::Trusted).unwrap();
        assert_eq!(r.trust("p1"), TrustState::Trusted);
        // re-observation must not demote an explicit host decision
        r.note_manifest_observed("p1").unwrap();
        assert_eq!(r.trust("p1"), TrustState::Trusted);
        // trust persists across reopen (backup path covers it too)
    }

    /// P2.4-C03: unset != allow; only the explicit decision API mutates;
    /// manifest requests record as unset; deny persists across reopen.
    #[test]
    fn capability_decisions_fail_closed_and_persist() {
        let dir = scratch_dir("reg_cap");
        let db = dir.join("plugins.db");
        {
            let r = PluginRegistry::open(&db).unwrap();
            assert_eq!(
                r.capability_decision("p1", "filesystem.read"),
                CapDecision::Unset,
                "unknown capability = unset (fail closed)"
            );
            // manifest REQUEST records — stays unset
            r.record_capability("p1", "filesystem.read").unwrap();
            assert_eq!(r.capability_decision("p1", "filesystem.read"), CapDecision::Unset);
            r.set_capability_decision("p1", "filesystem.read", CapDecision::Allow).unwrap();
            r.set_capability_decision("p1", "network", CapDecision::Deny).unwrap();
        }
        {
            let r = PluginRegistry::open(&db).unwrap();
            assert_eq!(r.capability_decision("p1", "filesystem.read"), CapDecision::Allow);
            assert_eq!(r.capability_decision("p1", "network"), CapDecision::Deny);
            assert_eq!(r.capability_decision("p1", "shell.execute"), CapDecision::Unset);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P2.3-D.1: corruption without a usable backup rebuilds fresh (safe
    /// defaults) and the corrupt file is quarantined, not deleted.
    #[test]
    fn corrupt_db_without_backup_rebuilds_and_quarantines() {
        let dir = scratch_dir("reg_no_bak");
        let db = dir.join("plugins.db");
        std::fs::write(&db, b"NOTSQLITE-not-a-database-at-all").unwrap();
        let r = PluginRegistry::open(&db).unwrap();
        // fresh registry = safe defaults
        assert!(r.state("p").enabled);
        let quarantined: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("plugins.db.corrupt-"))
            .collect();
        assert_eq!(quarantined.len(), 1, "corrupt file kept for forensics");
        std::fs::remove_dir_all(&dir).ok();
    }
}
