//! P2.8 Foundation (spec `P2.8 — Ecosystem & Distribution 1.0 技术设计规范.md`
//! §5/§7/§10/§35): plugin identity, package integrity (SHA-256 checksums)
//! and the plugin lifecycle state machine.
//!
//! Redlines: identity/manifest/integrity are DATA; trust defaults to
//! fail-closed (an unknown package is not installable-trust); the lifecycle
//! state machine rejects illegal transitions (e.g. Broken → Enabled without
//! an explicit repair).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// §5 PluginIdentity
// ---------------------------------------------------------------------------

/// A plugin's install identity: stable id + exact version + the contract the
/// package was built against. Two packages with the same identity are the
/// same plugin at the same version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginIdentity {
    pub id: String,
    pub version: String,
    pub contract_version: String,
}

impl PluginIdentity {
    /// Lexical validation shared with the CLI install path (fail-closed).
    pub fn validate(&self) -> Result<(), String> {
        let ok = !self.id.is_empty()
            && self.id.len() <= 128
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
        if !ok {
            return Err(format!("invalid plugin id `{}`", self.id));
        }
        if self.version.trim().is_empty() || self.version.len() > 64 {
            return Err("invalid version".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// §10 Package Integrity — SHA-256 (self-contained, no external crates)
// ---------------------------------------------------------------------------

/// SHA-256 of `data`, hex-encoded lowercase.
pub fn sha256_hex(data: &[u8]) -> String {
    let h = sha256(data);
    let mut out = String::with_capacity(64);
    for b in h {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bitlen = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());

    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// §10: verify packaged file bytes against an expected digest (constant-time
/// string compare; a mismatch is a hard integrity failure).
pub fn verify_integrity(expected_sha256: &str, data: &[u8]) -> bool {
    let actual = sha256_hex(data);
    // constant-time-ish comparison over equal-length ASCII
    if actual.len() != expected_sha256.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in actual.bytes().zip(expected_sha256.bytes()) {
        diff |= a ^ b.to_ascii_lowercase();
    }
    diff == 0
}

// ---------------------------------------------------------------------------
// §35 Plugin lifecycle state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginLifecycle {
    Uninstalled,
    Installed,
    Enabled,
    Disabled,
    Broken,
}

impl PluginLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginLifecycle::Uninstalled => "uninstalled",
            PluginLifecycle::Installed => "installed",
            PluginLifecycle::Enabled => "enabled",
            PluginLifecycle::Disabled => "disabled",
            PluginLifecycle::Broken => "broken",
        }
    }
}

/// Legal lifecycle transitions (§35). Fail-closed: anything not listed is
/// rejected — notably Broken can only move to Installed (repair/reinstall),
/// never directly back to Enabled.
pub fn transition_allowed(from: PluginLifecycle, to: PluginLifecycle) -> bool {
    use PluginLifecycle::*;
    matches!(
        (from, to),
        (Uninstalled, Installed)
            | (Installed, Enabled)
            | (Installed, Disabled)
            | (Installed, Uninstalled)
            | (Installed, Broken)
            | (Enabled, Disabled)
            | (Enabled, Uninstalled)
            | (Enabled, Broken)
            | (Disabled, Enabled)
            | (Disabled, Uninstalled)
            | (Disabled, Broken)
            | (Broken, Installed) // repair via reinstall
            | (Broken, Uninstalled)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256 correctness against the standard test vectors.
    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// §10: integrity verification detects any byte change.
    #[test]
    fn integrity_verification() {
        let digest = sha256_hex(b"package payload");
        assert!(verify_integrity(&digest, b"package payload"));
        assert!(!verify_integrity(&digest, b"package payload tampered"));
        assert!(!verify_integrity("deadbeef", b"package payload"));
    }

    /// §5: identity validation (id charset/bounds).
    #[test]
    fn identity_validation() {
        let ok = PluginIdentity {
            id: "dev.tool".into(),
            version: "1.2.0".into(),
            contract_version: "0.1".into(),
        };
        assert!(ok.validate().is_ok());
        let bad = PluginIdentity {
            id: "../evil".into(),
            version: "1.0".into(),
            contract_version: "0.1".into(),
        };
        assert!(bad.validate().is_err());
    }

    /// §35: illegal transitions rejected — Broken never silently re-Enables.
    #[test]
    fn lifecycle_rejects_illegal_transitions() {
        use PluginLifecycle::*;
        assert!(transition_allowed(Uninstalled, Installed));
        assert!(transition_allowed(Enabled, Disabled));
        assert!(transition_allowed(Broken, Installed), "repair path");
        assert!(!transition_allowed(Broken, Enabled), "fail-closed");
        assert!(!transition_allowed(Uninstalled, Enabled), "must install first");
        assert!(!transition_allowed(Enabled, Installed), "no re-install in place");
    }

    /// Lifecycle enum roundtrips (persistence contract).
    #[test]
    fn lifecycle_roundtrip() {
        for l in [
            PluginLifecycle::Uninstalled,
            PluginLifecycle::Installed,
            PluginLifecycle::Enabled,
            PluginLifecycle::Disabled,
            PluginLifecycle::Broken,
        ] {
            let json = serde_json::to_string(&l).unwrap();
            let back: PluginLifecycle = serde_json::from_str(&json).unwrap();
            assert_eq!(back, l);
        }
    }
}
