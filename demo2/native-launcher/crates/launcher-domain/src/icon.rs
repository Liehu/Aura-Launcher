//! Icon contract (P2.1-E1, review 79 §3-5/§51): lightweight cross-layer
//! types. NO bitmaps, NO HICON, NO filesystem — see INV-ICON-001/007.

use serde::{Deserialize, Serialize};

/// Where an application's icon comes from (review 79 §4): the Catalog
/// records only the SOURCE KIND; the icon subsystem decides how to fetch it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IconSource {
    PackageResource,
    Executable,
    Shortcut,
    Shell,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IconVariant {
    Small,
    Normal,
    Large,
}

/// The semantic icon identity (review 79 §51). Deliberately excludes the
/// display name (INV-ICON-006) and includes extraction provenance so cache
/// entries invalidate naturally when identity/algorithm/source change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IconKey {
    /// Semantic application identity, e.g. `win32:<normalized exe>` or
    /// `packaged:<family>:<appid>` (host-resolved only).
    pub application: String,
    pub variant: IconVariant,
    /// Bumped when the underlying source changes (e.g. package update).
    pub source_revision: u64,
    /// Bumped when extraction/decoding logic changes — invalidates old disk
    /// cache entries without a startup purge (review 79 §18).
    pub extractor_version: u32,
}

impl IconKey {
    /// Stable disk-cache key material (review 79 §17).
    pub fn cache_key(&self) -> String {
        format!(
            "{}|{:?}|v{}|e{}",
            self.application, self.variant, self.source_revision, self.extractor_version
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// INV-ICON-006: cache identity is semantic, not display-name based.
    #[test]
    fn cache_key_ignores_display_name_and_binds_provenance() {
        let base = IconKey {
            application: "win32:c:/chrome/chrome.exe".into(),
            variant: IconVariant::Normal,
            source_revision: 1,
            extractor_version: 1,
        };
        let mut renamed = base.clone();
        renamed.application = "win32:c:/chrome/chrome2.exe".into();
        assert_ne!(base.cache_key(), renamed.cache_key());

        let mut new_extractor = base.clone();
        new_extractor.extractor_version = 2;
        assert_ne!(base.cache_key(), new_extractor.cache_key());

        let mut repainted = base.clone();
        repainted.source_revision = 7;
        assert_ne!(base.cache_key(), repainted.cache_key());
    }
}
