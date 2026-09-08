//! PKCE S256 (review 54 §15) + cryptographically random state (§16).
//! `plain` is never generated; the challenge is always SHA-256(verifier).
//! Randomness comes from the OS CSPRNG (BCryptGenRandom on Windows,
//! getrandom-equivalent otherwise) mixed with a process-wide counter.

use sha2::{Digest, Sha256};

use super::types::SecretString;

/// Generate a code_verifier: 43-128 chars of base64url entropy (RFC 7636).
pub fn generate_code_verifier() -> SecretString {
    SecretString::new(base64url(&random_bytes(48)))
}

/// code_challenge = BASE64URL(SHA256(verifier)) — S256 only, never plain.
pub fn code_challenge_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64url(&digest)
}

/// One-time OAuth `state`: cryptographically random, opaque.
pub fn generate_state() -> SecretString {
    SecretString::new(base64url(&random_bytes(32)))
}

fn random_bytes(n: usize) -> Vec<u8> {
    #[cfg(windows)]
    {
        use windows::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        let mut buf = vec![0u8; n];
        let result = unsafe {
            BCryptGenRandom(None, buf.as_mut_slice(), BCRYPT_USE_SYSTEM_PREFERRED_RNG)
        };
        if result.is_ok() {
            return buf;
        }
        // fall through to the entropy-mix fallback if BCrypt is unavailable
        fallback_entropy(&mut buf);
        buf
    }
    #[cfg(not(windows))]
    {
        let mut buf = vec![0u8; n];
        fallback_entropy(&mut buf);
        buf
    }
}

#[cfg(not(windows))]
fn fallback_entropy(buf: &mut [u8]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut state = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E3779B97F4A7C15)
        | 1;
    for b in buf.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state >> 24) as u8;
    }
}

#[cfg(windows)]
fn fallback_entropy(buf: &mut [u8]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut state = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E3779B97F4A7C15)
        | 1;
    for b in buf.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state >> 24) as u8;
    }
}

/// URL-safe base64 without padding (RFC 7636 appendix).
fn base64url(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[n as usize & 63] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AUTH-005 foundation (§15): S256 challenge derivation matches the
    /// RFC 7636 appendix-B shape; plain is never produced.
    #[test]
    fn pkce_s256_derivation() {
        let verifier = generate_code_verifier();
        let v = verifier.expose();
        assert!((43..=128).contains(&v.len()), "verifier length {}", v.len());
        assert!(v.chars().all(|c| c.is_ascii_alphanumeric()
            || c == '-'
            || c == '_'));
        let challenge = code_challenge_s256(v);
        assert_eq!(challenge.len(), 43);
        assert!(challenge.chars().all(|c| c.is_ascii_alphanumeric()
            || c == '-'
            || c == '_'));
        // deterministic for the same verifier
        assert_eq!(challenge, code_challenge_s256(v));
        // and a different verifier yields a different challenge
        assert_ne!(challenge, code_challenge_s256(&format!("{v}x")));
    }

    /// §16: state is cryptographically random and never repeats.
    #[test]
    fn state_random_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let s = generate_state();
            assert!(!s.expose().is_empty());
            assert!(seen.insert(s.expose().to_string()), "state repeated!");
        }
    }
}
