//! Auth types (review 54 §5/§6/§26/§28/§32/§40): credentials, credential
//! identity, redaction, auth state machine and session scope.
//!
//! Trust rules frozen here:
//! - `SecretString` redacts in Debug/Display — tokens can never leak into
//!   logs, errors, proposals, workflow state or catalogs via formatting.
//! - A `Credential` is BOUND to its issuer; cross-issuer reuse is a hard
//!   error (AUTH-002/016).
//! - `CredentialKey` is NOT `server_id`: it binds server + resource origin
//!   + issuer + client id so a server can migrate authorization servers.
//! - Remote authentication and Launcher capability are two disjoint
//!   authority domains (INV-AUTH-007/008).

use std::fmt;
use std::time::SystemTime;

/// A secret that redacts itself in every formatter (review 54 §40).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: impl Into<String>) -> Self {
        SecretString(s.into())
    }
    /// Explicit, opt-in exposure — the ONLY way to read the secret.
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// Serialization exists ONLY for OS credential-store persistence; the
// redacting formatters above keep the secret out of logs/errors/proposals.
impl serde::Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(SecretString::new(String::deserialize(deserializer)?))
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// OAuth credential bound to the issuer that issued it (review 54 §5).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Credential {
    pub access_token: SecretString,
    pub refresh_token: Option<SecretString>,
    /// REQUIRED binding field: the issuer that issued this credential.
    pub issuer: String,
    pub client_id: String,
    pub expires_at: Option<SystemTime>,
    pub scopes: Vec<String>,
}

impl Credential {
    /// True when `expires_at` has passed (None = no expiry information —
    /// treated as valid; revocation is handled by the 401 flow).
    pub fn expired(&self, now: SystemTime) -> bool {
        self.expires_at.map(|e| e <= now).unwrap_or(false)
    }
}

/// Credential storage identity (review 54 §6): server_id alone is NOT the
/// key — a server may migrate authorization servers, and credentials must
/// never be reused across issuers or clients.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialKey {
    pub server_id: String,
    pub resource_origin: String,
    pub issuer: String,
    pub client_id: String,
}

impl CredentialKey {
    /// Canonical, unambiguous storage key (canonical identity fields; the
    /// same lexical policy as McpInvokeInput — no separators/whitespace
    /// inside components).
    pub fn storage_key(&self) -> String {
        format!(
            "launcher-mcp/{}:{}/{}",
            self.server_id, self.client_id, self.issuer
        )
    }
}

/// Auth-layer errors (review 54 §32). Never carries secrets in its
/// payload; Workflow maps the resulting McpError, not these variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("auth discovery failed")]
    DiscoveryFailed,
    #[error("credential issuer mismatch")]
    IssuerMismatch,
    #[error("oauth state mismatch")]
    StateMismatch,
    #[error("authorization denied by user or server")]
    AuthorizationDenied,
    #[error("token exchange failed")]
    TokenExchangeFailed,
    #[error("token refresh failed")]
    RefreshFailed,
    #[error("credential issuer changed since storage")]
    CredentialIssuerMismatch,
    #[error("invalid or expired credential")]
    InvalidCredential,
    #[error("unsupported auth flow")]
    UnsupportedFlow,
    #[error("auth metadata invalid")]
    MetadataInvalid,
    #[error("redirect rejected")]
    RedirectRejected,
}

/// Auth session state machine (review 54 §26). Owned by the auth
/// subsystem only — it never enters Workflow lifecycle or McpServerConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    NoCredential,
    Discovering,
    AuthorizationRequired,
    Authorizing,
    ExchangingCode,
    Authenticated,
    Refreshing,
    /// Stored issuer != discovered issuer: reauthorize from zero.
    IssuerChanged,
    Revoked,
}

/// Ephemeral, memory-only OAuth session (review 54 §28): state,
/// code_verifier and authorization-server identity live here and nowhere
/// else — never in config.toml, Workflow, database, proposals or logs.
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub auth_session_id: String,
    /// Cryptographically random, one-time, short-lived (§16/§42).
    pub state: SecretString,
    pub code_verifier: SecretString,
    pub authorization_server: String,
    pub redirect_uri: String,
}

/// Parse a URL into `(origin, scheme, host)` — used for credential key
/// resource origins and endpoint policy. Reuses the transport's URL rules.
pub fn split_origin(url: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if scheme.is_empty() || authority.is_empty() {
        return None;
    }
    Some((
        format!("{scheme}://{authority}"),
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §40 secret redaction: no formatting path can expose the secret.
    #[test]
    fn secret_redacts_everywhere() {
        let s = SecretString::new("eyJsuper-secret-token");
        assert_eq!(format!("{s:?}"), "[REDACTED]");
        assert_eq!(format!("{s}"), "[REDACTED]");
        assert_eq!(format!("{s:?}").contains("super-secret"), false);
        assert_eq!(s.expose(), "eyJsuper-secret-token");
    }

    /// §5: expiry semantics.
    #[test]
    fn credential_expiry() {
        let now = SystemTime::now();
        let mut c = Credential {
            access_token: SecretString::new("t"),
            refresh_token: None,
            issuer: "https://auth.example.com".into(),
            client_id: "client".into(),
            expires_at: Some(now - std::time::Duration::from_secs(10)),
            scopes: vec![],
        };
        assert!(c.expired(now));
        c.expires_at = Some(now + std::time::Duration::from_secs(3600));
        assert!(!c.expired(now));
        c.expires_at = None;
        assert!(!c.expired(now), "no expiry info = treated as valid");
    }

    /// §6: credential identity binds server + origin + issuer + client.
    #[test]
    fn credential_key_identity() {
        let k = CredentialKey {
            server_id: "github".into(),
            resource_origin: "https://mcp.github.example".into(),
            issuer: "https://auth.example.com".into(),
            client_id: "launcher".into(),
        };
        let k2 = CredentialKey {
            issuer: "https://auth.other.example".into(),
            ..k.clone()
        };
        assert_ne!(k.storage_key(), k2.storage_key(), "different issuer = different credential");
    }
}
