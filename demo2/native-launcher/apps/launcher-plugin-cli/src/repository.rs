//! Repository & Marketplace search (P2.8-A/§11-§13/§29): a repository is a
//! LOCAL JSON index file (§2: no central-server dependency — a repository
//! source is any path/URL the user configures; v1 ships the local-file
//! source). Search is deterministic and surfaces the TRUST BADGE computed
//! from signature+source so the UI never has to guess.

use crate::foundation::sha256_hex;
use crate::resolver::PackageMeta;
use crate::trust::{evaluate_trust, PackageSource, SignatureStatus, TrustDecision};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One repository index entry: package metadata + integrity + trust inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryEntry {
    #[serde(flatten)]
    pub package: PackageMeta,
    /// Human-facing summary for the marketplace list.
    pub summary: String,
    /// SHA-256 of the packaged payload (§10 integrity).
    pub payload_sha256: String,
    pub signature: SignatureBadge,
}

/// §9 badge persisted in the index (verified at publish time by the repo
/// tooling; the client re-verifies the checksum on download).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureBadge {
    Signed,
    ChecksumOnly,
    Unsigned,
}

/// §11/§12 Repository: a local index file. `save`/`load` are atomic-ish
/// (write-tmp-rename on save).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryIndex {
    pub entries: Vec<RepositoryEntry>,
}

impl RepositoryIndex {
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path_buf_with_suffix(path, "tmp");
        let tmp = Path::new(&tmp);
        std::fs::write(tmp, &body).map_err(|e| e.to_string())?;
        std::fs::rename(tmp, path).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<RepositoryIndex, String> {
        let raw = std::fs::read(path).map_err(|e| e.to_string())?;
        serde_json::from_slice(&raw).map_err(|e| e.to_string())
    }

    /// §13: add/replace an entry by package id, computing the payload digest.
    pub fn publish(&mut self, mut entry: RepositoryEntry, payload: &[u8]) {
        entry.payload_sha256 = sha256_hex(payload);
        self.entries.retain(|e| e.package.id != entry.package.id);
        self.entries.push(entry);
        self.entries.sort_by(|a, b| a.package.id.cmp(&b.package.id));
    }

    pub fn checksum_of(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.package.id == id)
            .map(|e| e.payload_sha256.as_str())
    }
}

fn path_buf_with_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{suffix}"));
    std::path::PathBuf::from(s)
}

/// §29 Marketplace search: deterministic substring relevance over id/name —
/// exact id match first, then name contains; every result carries its trust
/// badge computed from signature + the SOURCE the index was fetched from.
/// Search never installs anything.
pub fn search_marketplace(
    index: &RepositoryIndex,
    query: &str,
    source: PackageSource,
    limit: usize,
) -> Vec<(RepositoryEntry, TrustDecision)> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut exact = Vec::new();
    let mut contains = Vec::new();
    for e in &index.entries {
        let id = e.package.id.to_lowercase();
        let name = e.summary.to_lowercase();
        if id == q {
            exact.insert(0, e.clone());
        } else if id.starts_with(&q) {
            exact.push(e.clone());
        } else if name.contains(&q) {
            contains.push(e.clone());
        }
    }
    exact.extend(contains);
    exact.truncate(limit);
    exact
        .into_iter()
        .map(|e| {
            let badge = match e.signature {
                SignatureBadge::Signed => SignatureStatus::Signed,
                SignatureBadge::ChecksumOnly => SignatureStatus::ChecksumVerified,
                SignatureBadge::Unsigned => SignatureStatus::Unsigned,
            };
            let trust = evaluate_trust(badge, source);
            (e, trust)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::PackageMeta;

    fn entry(id: &str, summary: &str, badge: SignatureBadge) -> RepositoryEntry {
        RepositoryEntry {
            package: PackageMeta {
                id: id.into(),
                version: "1.0.0".into(),
                depends: vec![],
            },
            summary: summary.into(),
            payload_sha256: String::new(),
            signature: badge,
        }
    }

    /// §13: publish replaces by id and keeps the index sorted.
    #[test]
    fn publish_replaces_and_sorts() {
        let mut idx = RepositoryIndex::default();
        idx.publish(entry("zeta", "Zeta tools", SignatureBadge::Unsigned), b"z");
        idx.publish(entry("alpha", "Alpha tools", SignatureBadge::Signed), b"a");
        idx.publish(entry("alpha", "Alpha tools v2", SignatureBadge::Signed), b"a2");
        let ids: Vec<&str> = idx.entries.iter().map(|e| e.package.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "zeta"], "sorted, replaced by id");
        assert!(idx.checksum_of("alpha").is_some());
    }

    /// §29: deterministic relevance (exact id > id prefix > name contains),
    /// trust badge computed from signature + source.
    #[test]
    fn search_is_relevant_and_trust_badged() {
        let mut idx = RepositoryIndex::default();
        idx.publish(entry("chrome-tools", "Chrome helpers", SignatureBadge::Signed), b"c");
        idx.publish(entry("chromium-notes", "Chromium docs", SignatureBadge::Unsigned), b"n");
        idx.publish(entry("unrelated", "Nothing", SignatureBadge::ChecksumOnly), b"u");

        let results = search_marketplace(&idx, "chrom", PackageSource::OfficialRepository, 10);
        assert_eq!(results.len(), 2);
        // exact-prefix first
        assert_eq!(results[0].0.package.id, "chrome-tools");
        assert_eq!(results[0].1, TrustDecision::Trusted);
        assert_eq!(results[1].1, TrustDecision::Untrusted);

        // empty query = nothing (never the whole catalog)
        assert!(search_marketplace(&idx, "", PackageSource::OfficialRepository, 10).is_empty());
        // limit respected
        assert_eq!(
            search_marketplace(&idx, "chrom", PackageSource::SideLoad, 1).len(),
            1
        );
    }

    /// §11: save/load roundtrip preserves entries and checksums.
    #[test]
    fn index_save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("nl_repo_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.json");
        let mut idx = RepositoryIndex::default();
        idx.publish(entry("app", "App", SignatureBadge::ChecksumOnly), b"payload");
        idx.save(&path).unwrap();
        let back = RepositoryIndex::load(&path).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(
            back.checksum_of("app").unwrap(),
            sha256_hex(b"payload")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
