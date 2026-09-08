//! OAuth authorization-code + PKCE + refresh flow (review 54 §15/§16/§21/
//! §24/§31). All wire I/O is behind the `OAuthHttp` port so every rule
//! here is testable without a real network; the production port is bound
//! to ureq with HTTPS-only token endpoints (loopback exception, §21).

use std::time::{Duration, SystemTime};

use serde::Deserialize;

use super::pkce::{code_challenge_s256, generate_code_verifier, generate_state};
use super::types::{AuthSession, Credential, SecretString};
pub use super::types::AuthError;

/// Port for the token endpoint (and any OAuth metadata fetch the provider
/// needs at flow time). Production impl: ureq; tests: scripted.
pub trait OAuthHttp: Send {
    /// POST `application/x-www-form-urlencoded` to the token endpoint.
    fn post_form(&self, url: &str, form: &[(String, String)]) -> Result<serde_json::Value, AuthError>;
    /// GET a JSON document (metadata / CIMD fetch).
    fn get_json(&self, url: &str) -> Result<serde_json::Value, AuthError>;
}

/// Token endpoint scheme policy (§21): HTTPS required; loopback http
/// allowed for development.
pub fn validate_token_endpoint(url: &str) -> Result<(), AuthError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or(AuthError::MetadataInvalid)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let loopback = host.starts_with("127.0.0.1") || host.starts_with("localhost") || host.starts_with("[::1]");
    match scheme.to_ascii_lowercase().as_str() {
        "https" => Ok(()),
        "http" if loopback => Ok(()),
        _ => Err(AuthError::MetadataInvalid),
    }
}

/// Build the authorization redirect URL (§15/§16/§17): S256 challenge,
/// cryptographically random one-time state, native loopback redirect.
/// Returns (url, session).
pub fn begin_authorization(
    authorization_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
) -> Result<(String, AuthSession), AuthError> {
    if !redirect_uri.starts_with("http://127.0.0.1") && !redirect_uri.starts_with("http://localhost")
    {
        return Err(AuthError::RedirectRejected);
    }
    let state = generate_state();
    let code_verifier = generate_code_verifier();
    let challenge = code_challenge_s256(code_verifier.expose());
    let mut url = format!(
        "{authorization_endpoint}?response_type=code&client_id={client_id}\
         &redirect_uri={redirect_uri}&state={state}&code_challenge={challenge}\
         &code_challenge_method=S256"
    );
    if !scopes.is_empty() {
        url.push_str(&format!("&scope={}", scopes.join("%20")));
    }
    Ok((
        url,
        AuthSession {
            auth_session_id: format!("a-{}", std::process::id()),
            state,
            code_verifier,
            authorization_server: authorization_endpoint.to_string(),
            redirect_uri: redirect_uri.to_string(),
        },
    ))
}

/// A parsed OAuth callback (§31): `code`, `state`, and (2026) `iss`.
#[derive(Debug, Clone, PartialEq)]
pub struct Callback {
    pub code: String,
    pub state: String,
    pub iss: Option<String>,
}

/// Parse a callback query string. Missing code/state is a hard error.
pub fn parse_callback(query: &str) -> Result<Callback, AuthError> {
    fn urldecode(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'%' if i + 2 < bytes.len() + 1 && i + 2 <= bytes.len() - 1 + 1 => {
                    let hex = std::str::from_utf8(&bytes[i + 1..(i + 3).min(bytes.len())])
                        .unwrap_or_default();
                    match u8::from_str_radix(hex, 16) {
                        Ok(b) => {
                            out.push(b);
                            i += 3;
                        }
                        Err(_) => {
                            out.push(bytes[i]);
                            i += 1;
                        }
                    }
                }
                b'+' => {
                    out.push(b' ');
                    i += 1;
                }
                b => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    let mut code = None;
    let mut state = None;
    let mut iss = None;
    for pair in query.trim_start_matches('?').split('&') {
        let (k, v) = pair.split_once('=').ok_or(AuthError::StateMismatch)?;
        let v = urldecode(v);
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "iss" => iss = Some(v),
            _ => {}
        }
    }
    Ok(Callback {
        code: code.ok_or(AuthError::StateMismatch)?,
        state: state.ok_or(AuthError::StateMismatch)?,
        iss,
    })
}

/// §31 callback validation: state equality (constant-time-ish: compare
/// byte equality after length gate), issuer match, one-time use enforced
/// by the caller consuming the session. Any failure = NO token exchange.
pub fn validate_callback(
    session: &AuthSession,
    callback: &Callback,
    expected_issuer: &str,
) -> Result<(), AuthError> {
    if callback.state != session.state.expose() {
        return Err(AuthError::StateMismatch);
    }
    // state is single-use: validate MUST be followed by session destruction
    if let Some(iss) = &callback.iss {
        super::discovery::validate_issuer(expected_issuer, iss)?;
    }
    if callback.code.is_empty() {
        return Err(AuthError::AuthorizationDenied);
    }
    Ok(())
}

/// Token exchange / refresh request body builder.
pub fn token_exchange_form(
    code: &str,
    session: &AuthSession,
    client_id: &str,
) -> Vec<(String, String)> {
    vec![
        ("grant_type".into(), "authorization_code".into()),
        ("code".into(), code.into()),
        ("redirect_uri".into(), session.redirect_uri.clone()),
        ("client_id".into(), client_id.into()),
        ("code_verifier".into(), session.code_verifier.expose().into()),
    ]
}

pub fn refresh_form(
    refresh_token: &str,
    client_id: &str,
) -> Vec<(String, String)> {
    vec![
        ("grant_type".into(), "refresh_token".into()),
        ("refresh_token".into(), refresh_token.into()),
        ("client_id".into(), client_id.into()),
    ]
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// §24: a token response can never change the credential's issuer — the
/// issuer is taken from the EXPECTED value, and the token endpoint host
/// was already validated against it by the caller.
pub fn credential_from_token_response(
    expected_issuer: &str,
    client_id: &str,
    response: &TokenResponse,
) -> Result<Credential, AuthError> {
    if response.access_token.is_empty() {
        return Err(AuthError::TokenExchangeFailed);
    }
    Ok(Credential {
        access_token: SecretString::new(response.access_token.clone()),
        refresh_token: response.refresh_token.as_ref().map(|r| SecretString::new(r.clone())),
        issuer: expected_issuer.to_string(),
        client_id: client_id.to_string(),
        expires_at: response.expires_in.map(|s| SystemTime::now() + Duration::from_secs(s)),
        scopes: response
            .scope
            .as_ref()
            .map(|s| s.split(' ').map(str::to_string).collect())
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AUTH-003 (§31): state mismatch → reject BEFORE token exchange.
    #[test]
    fn auth003_state_mismatch() {
        let (url, session) = begin_authorization(
            "https://auth.example.com/authorize",
            "client",
            "http://127.0.0.1:0/callback",
            &[],
        )
        .unwrap();
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state="));
        // correct state parses + validates
        let state = session.state.expose().to_string();
        let cb = parse_callback(&format!("code=xyz&state={state}&iss=https%3A%2F%2Fauth.example.com")).unwrap();
        assert!(validate_callback(&session, &cb, "https://auth.example.com").is_ok());
        // flipped state → StateMismatch
        let cb_bad = parse_callback("code=xyz&state=evil&iss=https%3A%2F%2Fauth.example.com").unwrap();
        assert_eq!(
            validate_callback(&session, &cb_bad, "https://auth.example.com"),
            Err(AuthError::StateMismatch)
        );
    }

    /// AUTH-001 (§13): issuer mix-up in the callback iss — rejected before
    /// any redemption.
    #[test]
    fn auth001_callback_issuer_mixup() {
        let (_, session) = begin_authorization(
            "https://authA.example/authorize",
            "client",
            "http://127.0.0.1:0/callback",
            &[],
        )
        .unwrap();
        let state = session.state.expose().to_string();
        let cb = parse_callback(&format!("code=xyz&state={state}&iss=https%3A%2F%2FauthB.example")).unwrap();
        assert_eq!(
            validate_callback(&session, &cb, "https://authA.example"),
            Err(AuthError::IssuerMismatch)
        );
    }

    /// AUTH-005: PKCE verifier travels with the exchange (form content).
    #[test]
    fn auth005_pkce_verifier_in_exchange() {
        let (_, session) = begin_authorization(
            "https://auth.example.com/authorize",
            "client",
            "http://127.0.0.1:0/callback",
            &[],
        )
        .unwrap();
        let form = token_exchange_form("the-code", &session, "client");
        assert!(form.contains(&("code_verifier".into(), session.code_verifier.expose().into())));
        assert!(form.contains(&("code".into(), "the-code".into())));
    }

    /// §21: token endpoints must be HTTPS (loopback http = dev exception).
    #[test]
    fn auth008_token_endpoint_scheme() {
        assert!(validate_token_endpoint("https://auth.example.com/token").is_ok());
        assert!(validate_token_endpoint("http://127.0.0.1:9000/token").is_ok());
        assert!(validate_token_endpoint("http://auth.example.com/token").is_err());
    }

    /// §24: a token response never changes the stored issuer.
    #[test]
    fn auth016_refresh_issuer_binding() {
        let resp = TokenResponse {
            access_token: "at".into(),
            refresh_token: Some("rt".into()),
            expires_in: Some(3600),
            scope: Some("repo".into()),
        };
        let c = credential_from_token_response("https://authA.example", "client", &resp).unwrap();
        assert_eq!(c.issuer, "https://authA.example");
        assert!(!c.expired(SystemTime::now()));
        assert!(format!("{:?}", c.access_token).contains("[REDACTED]"));
    }
}
