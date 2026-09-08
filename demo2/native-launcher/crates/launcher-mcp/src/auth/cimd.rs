//! CIMD (Client ID Metadata Documents, review 54 §18/§19) + legacy DCR
//! compatibility policy (§20).
//!
//! Registration preference: CIMD preferred; DCR only as an explicit
//! fallback when the server advertises a registration endpoint and no CIMD
//! metadata is available. CIMD URLs must be HTTPS with a non-root path.
//! Client secrets are NEVER stored in config (§8).

use serde::Deserialize;

use super::types::AuthError;

/// A fetched Client ID Metadata Document (untrusted input until the
/// `client_id` inside is validated against the fetch URL's issuer).
#[derive(Debug, Clone, Deserialize)]
pub struct ClientMetadataDocument {
    pub client_id: String,
    #[serde(default, rename = "application_type")]
    pub application_type: Option<String>,
    #[serde(default, rename = "redirect_uris")]
    pub redirect_uris: Vec<String>,
}

/// CIMD URL policy (review 54 §19): HTTPS only, non-root path only.
/// `https://launcher.example/.well-known/oauth-client` is valid;
/// `http://...` and `https://example.com/` are not.
pub fn validate_cimd_url(url: &str) -> Result<(), AuthError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or(AuthError::MetadataInvalid)?;
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(AuthError::MetadataInvalid);
    }
    let after_authority = rest.split_once('/').map(|(_, p)| p).unwrap_or("");
    let path = after_authority
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    if path.is_empty() {
        // root URL: https://example.com/ — rejected (must identify the
        // specific client metadata document)
        return Err(AuthError::MetadataInvalid);
    }
    Ok(())
}

/// Parse a fetched CIMD document (untrusted JSON → semantic validation).
pub fn parse_client_metadata(v: &serde_json::Value) -> Result<ClientMetadataDocument, AuthError> {
    let m: ClientMetadataDocument =
        serde_json::from_value(v.clone()).map_err(|_| AuthError::MetadataInvalid)?;
    if m.client_id.is_empty() {
        return Err(AuthError::MetadataInvalid);
    }
    Ok(m)
}

/// DCR compatibility decision (§20): DCR is the fallback ONLY when the
/// server metadata advertises a registration endpoint (the server opted
/// into dynamic registration). CIMD unavailability alone is never enough.
pub fn should_fallback_to_dcr(
    registration_endpoint: Option<&str>,
    cimd_attempted_and_failed: bool,
) -> bool {
    cimd_attempted_and_failed
        && registration_endpoint.map(|e| !e.is_empty()).unwrap_or(false)
}

/// DCR request body (§20): native application, loopback-friendly redirect.
pub fn dcr_request_body(redirect_uri: &str) -> serde_json::Value {
    serde_json::json!({
        "client_name": "native-launcher",
        "application_type": "native",
        "redirect_uris": [redirect_uri],
        "token_endpoint_auth_method": "none"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CIMD-001/002/003: URL policy — HTTPS + non-root path.
    #[test]
    fn cimd_url_policy() {
        assert!(validate_cimd_url("https://launcher.example/.well-known/oauth-client").is_ok());
        assert!(validate_cimd_url("https://launcher.example/a/b/client.json").is_ok());
        assert!(validate_cimd_url("http://launcher.example/.well-known/oauth-client").is_err());
        assert!(validate_cimd_url("https://launcher.example/").is_err());
        assert!(validate_cimd_url("https://launcher.example").is_err());
    }

    /// CIMD-004: malformed metadata rejected.
    #[test]
    fn cimd_malformed_rejected() {
        assert!(matches!(
            parse_client_metadata(&serde_json::json!({"application_type": "native"})),
            Err(AuthError::MetadataInvalid)
        ));
        assert!(parse_client_metadata(&serde_json::json!({"client_id": "c"})).is_ok());
    }

    /// CIMD-007/008: CIMD preferred; DCR only as explicit fallback with a
    /// server-advertised registration endpoint.
    #[test]
    fn cimd_preferred_dcr_fallback() {
        assert!(!should_fallback_to_dcr(Some("https://as/registration"), false));
        assert!(should_fallback_to_dcr(Some("https://as/registration"), true));
        assert!(!should_fallback_to_dcr(None, true), "no registration endpoint: fail closed");
    }

    /// CIMD-009: DCR requests declare native application type.
    #[test]
    fn dcr_body_native() {
        let body = dcr_request_body("http://127.0.0.1:0/callback");
        assert_eq!(body["application_type"], "native");
        assert!(body["redirect_uris"].as_array().unwrap().len() == 1);
    }
}
