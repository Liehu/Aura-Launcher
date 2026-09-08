//! AuthProvider (review 54 §3/§9/§24/§33/§34): the ONLY component that
//! turns remote authentication into an `Authorization: Bearer ...` header.
//!
//! Frozen rules:
//! - 401 → refresh ONCE → retry the original request ONCE; a second 401
//!   surfaces as `AuthenticationRequired` (no refresh loops, §33).
//! - 403 → `PermissionDenied`, never a blind refresh/retry (§34).
//! - A valid remote credential does NOT grant Launcher authority, and
//!   Launcher authority does NOT grant credentials (INV-AUTH-007/008).
//! - Refresh can never change issuer or client_id (§24 / AUTH-016).
//! - Secrets are `SecretString` — they redact in every formatter.

pub mod cimd;
pub mod discovery;
pub mod oauth;
pub mod pkce;
pub mod storage;
pub mod types;

use std::time::SystemTime;

use self::discovery::{validate_issuer, AuthorizationServerMetadata};
use self::oauth::{credential_from_token_response, TokenResponse, OAuthHttp};
use self::types::{AuthError, Credential, CredentialKey};
use self::storage::CredentialStore;

/// What the transport needs from auth: a current Authorization header
/// value, refreshed at most once per request.
pub trait AuthProviderT: Send {
    /// Current header, refreshing an expired credential if possible.
    fn current_header(&mut self) -> Result<Option<String>, AuthError>;
    /// Called exactly once per 401: refresh-once; Ok(None) = give up.
    fn on_unauthorized(&mut self) -> Result<Option<String>, AuthError>;
}

/// Store-backed provider for one endpoint (one CredentialKey).
pub struct StoreAuthProvider<S: CredentialStore, H: OAuthHttp> {
    pub key: CredentialKey,
    pub token_endpoint: String,
    pub store: S,
    pub http: H,
}

impl<S: CredentialStore, H: OAuthHttp> StoreAuthProvider<S, H> {
    pub fn new(key: CredentialKey, token_endpoint: String, store: S, http: H) -> Self {
        Self { key, token_endpoint, store, http }
    }

    /// §24: issuer and client_id are fixed at credential creation; refresh
    /// reuses the stored refresh token against the SAME token endpoint.
    fn refresh_credential(&mut self, credential: &Credential) -> Result<Credential, AuthError> {
        // issuer binding check BEFORE the network call (AUTH-016)
        if credential.issuer != self.key.issuer {
            return Err(AuthError::CredentialIssuerMismatch);
        }
        let Some(refresh_token) = credential.refresh_token.as_ref() else {
            return Err(AuthError::RefreshFailed);
        };
        self::oauth::validate_token_endpoint(&self.token_endpoint)?;
        let form = self::oauth::refresh_form(refresh_token.expose(), &self.key.client_id);
        let v = self.http.post_form(&self.token_endpoint, &form)?;
        let resp: TokenResponse =
            serde_json::from_value(v).map_err(|_| AuthError::RefreshFailed)?;
        let refreshed =
            credential_from_token_response(&self.key.issuer, &self.key.client_id, &resp)?;
        // carry the refresh token forward when the response omits it
        let mut refreshed = refreshed;
        if refreshed.refresh_token.is_none() {
            refreshed.refresh_token = credential.refresh_token.clone();
        }
        self.store.save(&self.key, &refreshed).map_err(|_| AuthError::RefreshFailed)?;
        Ok(refreshed)
    }

    fn header_for(credential: &Credential) -> String {
        format!("Bearer {}", credential.access_token.expose())
    }
}

impl<S: CredentialStore, H: OAuthHttp> AuthProviderT for StoreAuthProvider<S, H> {
    fn current_header(&mut self) -> Result<Option<String>, AuthError> {
        let credential = self.store.load(&self.key).map_err(|_| AuthError::InvalidCredential)?;
        let Some(mut credential) = credential else {
            return Ok(None);
        };
        // issuer binding is checked on EVERY load (review 54 §14)
        if credential.issuer != self.key.issuer {
            return Err(AuthError::CredentialIssuerMismatch);
        }
        if credential.expired(SystemTime::now()) {
            credential = self.refresh_credential(&credential)?;
        }
        Ok(Some(Self::header_for(&credential)))
    }

    /// §33: called once per 401. Refreshes (expired or server-revoked
    /// session) and returns the new header; a credential that cannot
    /// refresh yields Ok(None) — the transport then reports
    /// AuthenticationRequired. Never loops.
    fn on_unauthorized(&mut self) -> Result<Option<String>, AuthError> {
        let credential = self.store.load(&self.key).map_err(|_| AuthError::InvalidCredential)?;
        let Some(credential) = credential else {
            return Ok(None);
        };
        if credential.issuer != self.key.issuer {
            return Err(AuthError::CredentialIssuerMismatch);
        }
        match self.refresh_credential(&credential) {
            Ok(refreshed) => Ok(Some(Self::header_for(&refreshed))),
            // refresh refused/expired: single attempt, then stop (§33)
            Err(AuthError::RefreshFailed) | Err(AuthError::InvalidCredential) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Convenience: validate a discovered authorization server against the
/// expected issuer (§12) before it may be used for anything.
pub fn accept_authorization_server(
    discovered_from: &str,
    metadata: &AuthorizationServerMetadata,
) -> Result<(), AuthError> {
    validate_issuer(discovered_from, &metadata.issuer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    struct FakeHttp {
        fail_refresh: bool,
    }
    impl OAuthHttp for FakeHttp {
        fn post_form(&self, _url: &str, _form: &[(String, String)]) -> Result<serde_json::Value, AuthError> {
            if self.fail_refresh {
                return Err(AuthError::RefreshFailed);
            }
            Ok(serde_json::json!({
                "access_token": "at-2", "refresh_token": "rt-2", "expires_in": 3600
            }))
        }
        fn get_json(&self, _url: &str) -> Result<serde_json::Value, AuthError> {
            Ok(serde_json::json!({}))
        }
    }

    fn key() -> CredentialKey {
        CredentialKey {
            server_id: "calc".into(),
            resource_origin: "https://mcp.example.com".into(),
            issuer: "https://auth.example.com".into(),
            client_id: "client".into(),
        }
    }

    fn credential(issuer: &str, expired: bool, refresh: bool) -> Credential {
        Credential {
            access_token: types::SecretString::new("at-1"),
            refresh_token: refresh.then(|| types::SecretString::new("rt-1")),
            issuer: issuer.into(),
            client_id: "client".into(),
            expires_at: expired.then(|| SystemTime::now() - Duration::from_secs(10)),
            scopes: vec![],
        }
    }

    type MemStore = self::storage::InMemoryCredentialStore;

    /// AUTH-017: expired credential → transparent refresh → valid header.
    #[test]
    fn auth017_expired_credential_refreshes() {
        let mut store = MemStore::new();
        store.save(&key(), &credential("https://auth.example.com", true, true)).unwrap();
        let mut provider =
            StoreAuthProvider::new(key(), "https://auth.example.com/token".into(), store, FakeHttp { fail_refresh: false });
        let header = provider.current_header().unwrap().unwrap();
        assert!(header.starts_with("Bearer at-2"), "refreshed token used");
    }

    /// AUTH-018: revoked (unrefreshable) credential → Ok(None), caller
    /// reports AuthenticationRequired; no infinite loop.
    #[test]
    fn auth018_revoked_stops() {
        let mut store = MemStore::new();
        store.save(&key(), &credential("https://auth.example.com", false, true)).unwrap();
        let mut provider =
            StoreAuthProvider::new(key(), "https://auth.example.com/token".into(), store, FakeHttp { fail_refresh: true });
        // valid-but-revoked-on-server: current_header returns the stored one
        assert!(provider.current_header().unwrap().is_some());
        // 401 handler refreshes exactly once; the fixture refuses → None
        // (the caller then reports AuthenticationRequired; never loops)
        assert!(provider.on_unauthorized().unwrap().is_none());
        // and repeated 401s stay bounded: still None, still no panic
        assert!(provider.on_unauthorized().unwrap().is_none());
    }

    /// AUTH-002/016: credential issuer mismatch — hard error, no network.
    #[test]
    fn auth002_credential_issuer_mismatch() {
        let mut store = MemStore::new();
        store.save(&key(), &credential("https://authOTHER.example", false, true)).unwrap();
        let mut provider =
            StoreAuthProvider::new(key(), "https://auth.example.com/token".into(), store, FakeHttp { fail_refresh: false });
        assert_eq!(
            provider.current_header(),
            Err(AuthError::CredentialIssuerMismatch)
        );
    }

    /// AUTH-009/010/011 (§40): secret redaction holds across store round-
    /// trips and error formatting.
    #[test]
    fn auth_secret_never_leaks() {
        let mut store = MemStore::new();
        store.save(&key(), &credential("https://auth.example.com", true, true)).unwrap();
        let dumped = format!("{store:?}");
        assert!(!dumped.contains("at-1") && !dumped.contains("rt-1"), "store leaks secrets");
        let mut provider =
            StoreAuthProvider::new(key(), "https://auth.example.com/token".into(), store, FakeHttp { fail_refresh: true });
        assert!(provider.on_unauthorized().unwrap().is_none());
        // error formatting (RefreshFailed) carries no secrets
        let err = format!("{:?}", AuthError::RefreshFailed.to_string());
        assert!(!err.contains("at-1") && !err.contains("rt-1"));
        // the store Debug was checked above; credential Debug too
        let c = credential("https://auth.example.com", false, true);
        assert!(!format!("{c:?}").contains("at-1"));
        let _ = Mutex::new(0);
    }

    /// §12: authorization servers are accepted only after issuer
    /// validation.
    #[test]
    fn accept_authorization_server_validates() {
        use super::{accept_authorization_server, AuthorizationServerMetadata};
        let m = AuthorizationServerMetadata {
            issuer: "https://authB.example".into(),
            authorization_endpoint: "a".into(),
            token_endpoint: "t".into(),
            registration_endpoint: None,
            scopes_supported: vec![],
            code_challenge_methods_supported: vec!["S256".into()],
        };
        assert_eq!(
            accept_authorization_server("https://authA.example", &m),
            Err(AuthError::IssuerMismatch)
        );
        assert!(accept_authorization_server("https://authB.example", &m).is_ok());
    }
}
