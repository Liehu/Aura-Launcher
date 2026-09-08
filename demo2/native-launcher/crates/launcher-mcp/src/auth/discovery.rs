//! Auth metadata discovery (review 54 §10/§11/§12): Protected Resource
//! Metadata → Authorization Server Metadata, plus the S0/S1 issuer
//! validation rule.
//!
//! Trust rule (§10): metadata is server-declared DATA, not authority —
//! `authorization_servers` is only a discovery INPUT; every issuer that
//! comes back must re-validate against what we discovered.

use serde::Deserialize;

use super::types::AuthError;

/// `/.well-known/oauth-protected-resource` (review 54 §10).
#[derive(Debug, Clone, Deserialize)]
pub struct ProtectedResourceMetadata {
    pub resource: String,
    #[serde(default, rename = "authorization_servers")]
    pub authorization_servers: Vec<String>,
    #[serde(default, rename = "scopes_supported")]
    pub scopes_supported: Vec<String>,
}

/// `/.well-known/oauth-authorization-server` (review 54 §11).
#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizationServerMetadata {
    /// The issuer this document belongs to. MUST equal the URL the
    /// document was discovered from (§12 issuer validation).
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default, rename = "registration_endpoint")]
    pub registration_endpoint: Option<String>,
    #[serde(default, rename = "scopes_supported")]
    pub scopes_supported: Vec<String>,
    #[serde(default, rename = "code_challenge_methods_supported")]
    pub code_challenge_methods_supported: Vec<String>,
}

/// Well-known path constants.
pub const PROTECTED_RESOURCE_WELL_KNOWN: &str = "/.well-known/oauth-protected-resource";
pub const AUTHORIZATION_SERVER_WELL_KNOWN: &str = "/.well-known/oauth-authorization-server";
pub const OPENID_WELL_KNOWN: &str = "/.well-known/openid-configuration";

/// Origin of a URL (`scheme://authority`) for well-known construction.
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    Some(format!("{scheme}://{authority}"))
}

/// Protected-resource well-known URL for an MCP endpoint (review 54 §10).
pub fn protected_resource_metadata_url(mcp_url: &str) -> Option<String> {
    Some(format!(
        "{}{PROTECTED_RESOURCE_WELL_KNOWN}",
        origin_of(mcp_url)?
    ))
}

/// Authorization-server well-known URL candidates, in discovery fallback
/// order (§11): OAuth metadata first, then OIDC configuration.
pub fn authorization_server_metadata_urls(issuer: &str) -> Vec<String> {
    let origin = origin_of(issuer).unwrap_or_default();
    vec![
        format!("{origin}{AUTHORIZATION_SERVER_WELL_KNOWN}"),
        format!("{origin}{OPENID_WELL_KNOWN}"),
    ]
}

/// §12 — THE S0/S1 rule: the `iss` returned by a discovered document (or
/// an authorization callback) must equal the issuer we discovered it from.
/// Comparison is exact after normalizing ONE trailing slash only; anything
/// else (host swap, scheme downgrade, path injection) is a mix-up.
pub fn validate_issuer(discovered: &str, returned: &str) -> Result<(), AuthError> {
    fn norm(s: &str) -> &str {
        s.trim_end_matches('/')
    }
    if norm(discovered) == norm(returned) {
        Ok(())
    } else {
        Err(AuthError::IssuerMismatch)
    }
}

/// PKCE method policy (§15): the launcher speaks S256 only. A server that
/// does not advertise S256 is unsupported (fail closed), unless we're
/// forced into a compatibility profile later (not in P0-C).
pub fn supports_pkce_s256(metadata: &AuthorizationServerMetadata) -> bool {
    metadata
        .code_challenge_methods_supported
        .iter()
        .any(|m| m.eq_ignore_ascii_case("S256"))
}

/// Parse protected-resource metadata from JSON, rejecting anything without
/// the required fields (metadata is untrusted input).
pub fn parse_protected_resource(v: &serde_json::Value) -> Result<ProtectedResourceMetadata, AuthError> {
    let m: ProtectedResourceMetadata = serde_json::from_value(v.clone())
        .map_err(|_| AuthError::MetadataInvalid)?;
    if m.resource.is_empty() || m.authorization_servers.is_empty() {
        return Err(AuthError::MetadataInvalid);
    }
    Ok(m)
}

/// Parse authorization-server metadata; issuer field is REQUIRED.
pub fn parse_authorization_server(v: &serde_json::Value) -> Result<AuthorizationServerMetadata, AuthError> {
    let m: AuthorizationServerMetadata = serde_json::from_value(v.clone())
        .map_err(|_| AuthError::MetadataInvalid)?;
    if m.issuer.is_empty()
        || m.authorization_endpoint.is_empty()
        || m.token_endpoint.is_empty()
    {
        return Err(AuthError::MetadataInvalid);
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// AUTH-001 (§12/§13): authorization-server mix-up — a returned issuer
    /// that differs from the discovered one is rejected, no token exchange.
    #[test]
    fn auth001_issuer_mixup_rejected() {
        assert!(validate_issuer("https://auth.example.com", "https://auth.example.com").is_ok());
        assert!(validate_issuer("https://auth.example.com/", "https://auth.example.com").is_ok());
        assert_eq!(
            validate_issuer("https://auth.example.com", "https://evil.example.com"),
            Err(AuthError::IssuerMismatch)
        );
        // scheme downgrade is a mismatch
        assert_eq!(
            validate_issuer("https://auth.example.com", "http://auth.example.com"),
            Err(AuthError::IssuerMismatch)
        );
        // path injection is a mismatch
        assert_eq!(
            validate_issuer("https://auth.example.com", "https://auth.example.com.evil.io"),
            Err(AuthError::IssuerMismatch)
        );
    }

    /// §11: metadata parsing requires the semantic fields (E-003 principle:
    /// serde structural validity ≠ protocol validity).
    #[test]
    fn auth_metadata_semantic_validation() {
        let good = json!({
            "issuer": "https://auth.example.com",
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token",
            "code_challenge_methods_supported": ["S256"]
        });
        let m = parse_authorization_server(&good).unwrap();
        assert!(supports_pkce_s256(&m));
        // missing issuer → MetadataInvalid
        let bad = json!({
            "authorization_endpoint": "x", "token_endpoint": "y"
        });
        assert!(matches!(
            parse_authorization_server(&bad),
            Err(AuthError::MetadataInvalid)
        ));
        // unsupported PKCE methods → unsupported flow, fail closed
        let plain_only = json!({
            "issuer": "i", "authorization_endpoint": "a", "token_endpoint": "t",
            "code_challenge_methods_supported": ["plain"]
        });
        assert!(!supports_pkce_s256(&parse_authorization_server(&plain_only).unwrap()));
    }

    /// §10: protected resource metadata parsing.
    #[test]
    fn protected_resource_parsing() {
        let good = json!({
            "resource": "https://mcp.example.com",
            "authorization_servers": ["https://auth.example.com"]
        });
        let m = parse_protected_resource(&good).unwrap();
        assert_eq!(m.authorization_servers[0], "https://auth.example.com");
        let bad = json!({"resource": ""});
        assert!(matches!(
            parse_protected_resource(&bad),
            Err(AuthError::MetadataInvalid)
        ));
    }
}
