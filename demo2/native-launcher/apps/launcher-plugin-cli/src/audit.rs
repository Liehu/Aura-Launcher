//! Audit log (P2.8/§32): append-only JSONL event trail for ecosystem
//! operations (install/upgrade/uninstall/decisions). Append-only means the
//! reader never rewrites; each line is one self-contained JSON event with a
//! monotonic sequence number.

use std::io::Write;
use std::path::Path;

/// Append one audit event. Creates the file if missing. Never throws on
/// content — the caller decides whether a failed audit write is fatal
/// (security-relevant events SHOULD be).
pub fn append(event_type: &str, detail: &serde_json::Value, path: &Path) -> Result<u64, String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let seq = next_seq(path)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let line = serde_json::json!({
        "seq": seq,
        "event": event_type,
        "detail": detail,
    });
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok(seq)
}

/// Read the full trail (seq order = file order).
pub fn read_all(path: &Path) -> Result<Vec<(u64, String, serde_json::Value)>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut out = Vec::new();
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        out.push((
            v["seq"].as_u64().unwrap_or(0),
            v["event"].as_str().unwrap_or("").to_string(),
            v["detail"].clone(),
        ));
    }
    Ok(out)
}

fn next_seq(path: &Path) -> Result<u64, String> {
    Ok(read_all(path)?.last().map(|(seq, _, _)| *seq).unwrap_or(0) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_trail_appends_in_order() {
        let dir = std::env::temp_dir().join(format!("nl_audit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("audit.jsonl");
        append("install", &serde_json::json!({"id": "app", "by": "user"}), &path).unwrap();
        append("approve", &serde_json::json!({"id": "app"}), &path).unwrap();
        append("uninstall", &serde_json::json!({"id": "app"}), &path).unwrap();
        let trail = read_all(&path).unwrap();
        assert_eq!(trail.len(), 3);
        assert_eq!(trail[0].0, 1);
        assert_eq!(trail[2].1, "uninstall");
        assert_eq!(trail[2].2["id"], "app");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Missing file = empty trail (never an error).
    #[test]
    fn missing_trail_is_empty() {
        let dir = std::env::temp_dir().join(format!("nl_audit_e_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_all(&dir.join("none.jsonl")).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
