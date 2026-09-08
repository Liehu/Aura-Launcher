//! HTTP security policy for the Streamable HTTP transport (review P0-B
//! §6/§19/§20/§25/§26): URL classification, reserved headers, status
//! mapping and content-type rules. This is TRANSPORT policy — it lives in
//! launcher-mcp and is never consulted by executors, workflow or the AI.

use std::time::Duration;

use crate::error::McpError;

/// Network class of a configured MCP endpoint host (review P0-B §20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkClass {
    Loopback,
    Private,
    Public,
    /// Blocklisted outright (cloud metadata endpoints etc.).
    Metadata,
}

/// Explicit opt-ins from configuration. Defaults are fail-closed:
/// loopback HTTP allowed, everything plaintext beyond loopback requires
/// `allow_plain_http`, private networks always require
/// `allow_private_network` (even over HTTPS, to keep the SSRF surface
/// explicit), metadata endpoints are NEVER allowed.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlPolicy {
    pub allow_private_network: bool,
    pub allow_plain_http: bool,
}

/// Classify a host string (no DNS: P0-B classifies the URL; resolver-level
/// pinning is explicitly out of scope per review P0-B §21).
pub fn classify_host(host: &str) -> NetworkClass {
    let mut host = host.trim().to_ascii_lowercase();
    // bracketed IPv6 (possibly [::1]:8080): take the address inside brackets
    if let Some(rest) = host.strip_prefix('[') {
        host = rest.split(']').next().unwrap_or(rest).to_string();
    } else {
        host = host.trim_end_matches('.').to_string();
        // strip an explicit port on non-bracketed hosts
        if let Some((h, port)) = host.rsplit_once(':') {
            if port.parse::<u16>().is_ok() {
                host = h.to_string();
            }
        }
    }
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "0.0.0.0" | "::1") {
        return NetworkClass::Loopback;
    }
    // cloud metadata endpoints: deny outright
    if host == "169.254.169.254" || host.starts_with("169.254.169.") {
        return NetworkClass::Metadata;
    }
    // IPv6 private/link-local (fc00::/7 unique-local, fe80::/10 link-local)
    if host.starts_with("fe80:") || host.starts_with("fc") || host.starts_with("fd") {
        return NetworkClass::Private;
    }
    // dotted-quad IPv4 classification
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        let octets: Vec<u8> = parts.iter().map(|p| p.parse().unwrap()).collect();
        return match (octets[0], octets[1]) {
            (127, _) => NetworkClass::Loopback,
            (10, _) => NetworkClass::Private,
            (172, 16..=31) => NetworkClass::Private,
            (192, 168) => NetworkClass::Private,
            (169, 254) => NetworkClass::Metadata, // link-local incl. metadata range
            _ => NetworkClass::Public,
        };
    }
    // hostnames: loopback names already handled; everything else public
    NetworkClass::Public
}


/// Validate `url` against scheme + network policy (review P0-B §6/§20).
/// Returns the (scheme, host) pair on success.
pub fn validate_url(url: &str, policy: &UrlPolicy) -> Result<(String, String), McpError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| McpError::InvalidInput(format!("mcp url must be absolute: {url}")))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(McpError::InvalidInput(format!(
            "mcp url scheme must be http(s): {url}"
        )));
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_string();
    if host.is_empty() {
        return Err(McpError::InvalidInput(format!("mcp url has no host: {url}")));
    }
    let class = classify_host(&host);
    if class == NetworkClass::Metadata {
        return Err(McpError::InvalidInput(
            "mcp url points at a metadata endpoint (blocked)".into(),
        ));
    }
    if scheme == "http" {
        let loopback = class == NetworkClass::Loopback;
        if !loopback && !policy.allow_plain_http {
            return Err(McpError::InvalidInput(
                "plaintext http is only allowed for loopback endpoints unless \
                 allow_plain_http is explicitly configured"
                    .into(),
            ));
        }
    }
    if class == NetworkClass::Private && !policy.allow_private_network {
        return Err(McpError::InvalidInput(
            "private-network mcp endpoints require explicit allow_private_network"
                .into(),
        ));
    }
    Ok((scheme, host))
}

/// Headers the transport owns outright (review P0-B §25): custom headers
/// must never override these.
pub const RESERVED_HEADERS: &[&str] = &[
    "mcp-protocol-version",
    "mcp-method",
    "mcp-name",
    "content-type",
    "content-length",
    "host",
    "authorization",
    "cookie",
];

/// True when a user-configured header name collides with a reserved one.
pub fn is_reserved_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    RESERVED_HEADERS.contains(&lower.as_str())
        || lower.starts_with("mcp-")
        || lower.starts_with("mcp_")
}

/// Validate a response Content-Type (review P0-B §15/§16): JSON (with
/// optional parameters) is the only accepted result media type in P0-B;
/// anything else must never be interpreted as an MCP result.
pub fn validate_content_type(content_type: Option<&str>) -> Result<(), McpError> {
    let Some(ct) = content_type else {
        return Err(McpError::ProtocolViolation(
            "response missing Content-Type".into(),
        ));
    };
    let media = ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    match media.as_str() {
        "application/json" => Ok(()),
        "text/event-stream" => Err(McpError::ProtocolViolation(
            "text/event-stream responses are unsupported in P0-B (safe reject)".into(),
        )),
        other => Err(McpError::ProtocolViolation(format!(
            "unexpected response content-type: {other}"
        ))),
    }
}

/// HTTP status → McpError (review P0-B §14/§41): three layers stay
/// distinct — HTTP transport semantics, JSON-RPC semantics (handled by the
/// caller when a 400 carries a valid JSON-RPC error), and MCP method
/// semantics. Status mapping lives HERE, never in Workflow.
pub fn status_error(status: u16, endpoint: &str) -> Option<McpError> {
    let err = match status {
        200..=299 => return None,
        400 | 404 => McpError::ServerUnavailable(format!(
            "http {status} from {endpoint}"
        )),
        401 => McpError::AuthenticationRequired(format!("http 401 from {endpoint}")),
        403 => McpError::PermissionDenied(format!("http 403 from {endpoint}")),
        408 => McpError::Timeout(Duration::from_secs(0)),
        // 429 and 5xx: server-side condition, preserved as unavailable
        429 => McpError::ServerUnavailable(format!("http 429 (throttled) from {endpoint}")),
        s if s >= 500 => McpError::ServerUnavailable(format!("http {s} from {endpoint}")),
        s => McpError::ProtocolViolation(format!("unexpected http status {s} from {endpoint}")),
    };
    Some(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HTTP-SEC-003/004/005 (§43 SSRF matrix): URL classification policy.
    #[test]
    fn http_sec_url_policy_matrix() {
        let strict = UrlPolicy::default();
        let lax = UrlPolicy { allow_private_network: true, allow_plain_http: true };

        // loopback HTTP: allowed by default
        assert!(validate_url("http://127.0.0.1:9000/mcp", &strict).is_ok());
        assert!(validate_url("http://localhost:9000/mcp", &strict).is_ok());
        assert!(validate_url("http://[::1]:9000/mcp", &strict).is_ok());
        // metadata endpoints: NEVER allowed, even fully open policy
        assert!(validate_url("http://169.254.169.254/latest/meta-data", &lax).is_err());
        assert!(validate_url("http://169.254.170.2/x", &lax).is_err());
        // private networks: opt-in only
        for url in [
            "http://10.1.2.3/mcp",
            "http://172.16.0.9/mcp",
            "http://172.31.255.1/mcp",
            "http://192.168.1.1/mcp",
            "https://192.168.5.5/mcp",
        ] {
            assert!(validate_url(url, &strict).is_err(), "{url} private must opt in");
            assert!(validate_url(url, &lax).is_ok(), "{url} allowed with opt-in");
        }
        // public HTTPS: allowed; public HTTP: opt-in only
        assert!(validate_url("https://mcp.example.com/mcp", &strict).is_ok());
        assert!(validate_url("http://mcp.example.com/mcp", &strict).is_err());
        assert!(validate_url("http://mcp.example.com/mcp", &lax).is_ok());
        // non-http schemes rejected
        assert!(validate_url("ftp://mcp.example.com", &lax).is_err());
        assert!(validate_url("mcp.example.com", &lax).is_err());
    }

    /// HTTP-SEC-007 (§25/§26): reserved headers can never be overridden.
    #[test]
    fn http_sec_reserved_headers() {
        for name in [
            "MCP-Protocol-Version", "Mcp-Method", "Mcp-Name", "Content-Type",
            "Content-Length", "Host", "Authorization", "Cookie", "MCP-Anything",
        ] {
            assert!(is_reserved_header(name), "{name} must be reserved");
        }
        assert!(!is_reserved_header("X-Organization"));
    }

    /// §15/§16: content-type rules — JSON tolerated with parameters,
    /// SSE safe-rejected, HTML never an MCP result.
    #[test]
    fn http_content_type_matrix() {
        assert!(validate_content_type(Some("application/json")).is_ok());
        assert!(validate_content_type(Some("application/json; charset=utf-8")).is_ok());
        assert!(validate_content_type(None).is_err());
        assert!(matches!(
            validate_content_type(Some("text/event-stream")),
            Err(McpError::ProtocolViolation(_))
        ));
        assert!(matches!(
            validate_content_type(Some("text/html")),
            Err(McpError::ProtocolViolation(_))
        ));
    }

    /// §41: HTTP status → McpError matrix (single mapping point).
    #[test]
    fn http_status_matrix() {
        assert!(status_error(200, "e").is_none());
        assert!(status_error(201, "e").is_none());
        assert!(matches!(status_error(400, "e").unwrap(), McpError::ServerUnavailable(_)));
        assert!(matches!(
            status_error(401, "e").unwrap(),
            McpError::AuthenticationRequired(_)
        ));
        assert!(matches!(
            status_error(403, "e").unwrap(),
            McpError::PermissionDenied(_)
        ));
        assert!(matches!(status_error(404, "e").unwrap(), McpError::ServerUnavailable(_)));
        assert!(matches!(status_error(408, "e").unwrap(), McpError::Timeout(_)));
        assert!(matches!(status_error(429, "e").unwrap(), McpError::ServerUnavailable(_)));
        assert!(matches!(status_error(500, "e").unwrap(), McpError::ServerUnavailable(_)));
        assert!(matches!(
            status_error(302, "e").unwrap(),
            McpError::ProtocolViolation(_)
        ));
    }
}
