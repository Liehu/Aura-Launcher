//! Update handoff (P2.2-E §25/§26): the only state an outgoing process may
//! leave for the next boot during an upgrade — pure upgrade metadata.
//!
//! Invariants (spec §26): handoff never carries clipboard, query content,
//! plugin payload, credentials or tokens. The file is temporary, written by
//! atomic replace, and consumed exactly once (§50 WaitingForExit → recover
//! handoff: consumption IS the recovery decision, not a heuristic — the
//! running binary's health check at the end of this boot decides whether the
//! upgrade is marked healthy).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Upgrade metadata handed from the outgoing process to the next boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateHandoff {
    /// Correlation id for the upgrade transaction (log/evidence use only).
    pub transaction_id: String,
    /// Version the upgrade was moving to (informational; the running binary
    /// is already authoritative for what it is).
    pub target_version: String,
    /// Who requested the upgrade (e.g. "user_update", "auto_recovery").
    #[serde(default)]
    pub reason: String,
    /// Unix epoch millis when the upgrade started.
    pub started_at_ms: i64,
    /// §25: `resume=true` means the next boot should treat the upgrade as
    /// interrupted and complete the recovery path.
    #[serde(default)]
    pub resume: bool,
}

pub fn handoff_path(data_dir: &Path) -> PathBuf {
    data_dir.join("update_handoff.json")
}

/// Atomically write the handoff file (tmp + rename, spec §25).
pub fn write_handoff(data_dir: &Path, handoff: &UpdateHandoff) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let path = handoff_path(data_dir);
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    let body = serde_json::to_string_pretty(handoff)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path).or_else(|e| {
        let _ = std::fs::remove_file(&tmp);
        Err(e)
    })
}

/// Read a pending handoff, if any. A corrupt or undecodable file is treated
/// as absent (logged, then removed — an unreadable handoff is not a reason
/// to block boot).
pub fn read_handoff(data_dir: &Path) -> Option<UpdateHandoff> {
    let path = handoff_path(data_dir);
    let raw = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<UpdateHandoff>(&raw) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "update handoff undecodable — discarding");
            let _ = std::fs::remove_file(&path);
            None
        }
    }
}

/// Consume-once: read the pending handoff and remove the file. §50
/// (WaitingForExit → recover handoff): the next boot either recovers it or
/// it must not linger — a stale handoff would make every later boot think
/// an upgrade is still in flight.
pub fn consume_handoff(data_dir: &Path) -> Option<UpdateHandoff> {
    let handoff = read_handoff(data_dir)?;
    let path = handoff_path(data_dir);
    if let Err(e) = std::fs::remove_file(&path) {
        // only warn: the file was already read; failing to delete (e.g.
        // AV lock) must not crash startup — the next boot will just
        // re-consume the same metadata, which is idempotent.
        tracing::warn!(path = %path.display(), error = %e, "update handoff removal failed");
    }
    Some(handoff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "nl_handoff_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample() -> UpdateHandoff {
        UpdateHandoff {
            transaction_id: "U-123".into(),
            target_version: "1.1.0".into(),
            reason: "user_update".into(),
            started_at_ms: 1_790_000_000_000,
            resume: true,
        }
    }

    /// §25: write → read roundtrip, atomic replace leaves no tmp behind.
    #[test]
    fn write_then_read_roundtrips() {
        let dir = scratch("roundtrip");
        write_handoff(&dir, &sample()).unwrap();
        assert!(!dir.join("update_handoff.json.tmp").exists());
        assert_eq!(read_handoff(&dir), Some(sample()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// §50: consume-once — the second boot sees nothing.
    #[test]
    fn consume_is_once() {
        let dir = scratch("consume");
        write_handoff(&dir, &sample()).unwrap();
        assert_eq!(consume_handoff(&dir), Some(sample()));
        assert_eq!(consume_handoff(&dir), None);
        assert!(!handoff_path(&dir).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A corrupt handoff is discarded, never blocks boot.
    #[test]
    fn corrupt_handoff_discarded() {
        let dir = scratch("corrupt");
        std::fs::write(handoff_path(&dir), "{not json").unwrap();
        assert_eq!(read_handoff(&dir), None);
        assert!(!handoff_path(&dir).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// §26: no sensitive fields — the struct serializes only upgrade
    /// metadata (guard against someone adding payload fields later without
    /// revisiting the spec).
    #[test]
    fn serialization_is_metadata_only() {
        let json = serde_json::to_string(&sample()).unwrap();
        for forbidden in ["clipboard", "query", "payload", "credential", "token"] {
            assert!(!json.to_lowercase().contains(forbidden));
        }
        assert_eq!(
            json,
            r#"{"transaction_id":"U-123","target_version":"1.1.0","reason":"user_update","started_at_ms":1790000000000,"resume":true}"#
        );
    }
}
