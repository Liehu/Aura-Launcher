//! Compatibility Profile model (MVP4.3 Phase 11, review 47 §2/§3): wire
//! protocol differences are isolated to `launcher-mcp`; domain, workflow,
//! action and the AI planner never see a protocol version.
//!
//! Profiles are APPENDED, never reinterpreted: a new revision adds a
//! variant, it does not change the meaning of existing ones.

/// One MCP wire protocol revision. v0.1 ships the legacy compatibility
/// profile; the current spec revision is the stateless profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum McpProtocolProfile {
    /// Legacy compatibility profile: stateful
    /// `initialize → notifications/initialized → tools/*` session
    /// (the implementation Phase 1–10 was security-hardened against).
    V2025_06_18,
    /// Current spec revision: stateless, self-describing requests — no
    /// initialize/session handshake; client info and protocol version
    /// travel in each request's `_meta`; `server/discover` available.
    V2026_07_28,
}

impl Default for McpProtocolProfile {
    fn default() -> Self {
        Self::V2025_06_18
    }
}

impl McpProtocolProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V2025_06_18 => "2025-06-18",
            Self::V2026_07_28 => "2026-07-28",
        }
    }

    /// Config-side parse; unknown revisions fail closed to an error rather
    /// than silently degrading to another profile (compat contract rule 9).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "2025-06-18" | "" | "legacy" => Some(Self::V2025_06_18),
            "2026-07-28" | "2026" => Some(Self::V2026_07_28),
            _ => None,
        }
    }

    /// Legacy profile keeps the stateful initialize handshake; the current
    /// profile is stateless (each request is self-describing).
    pub fn needs_handshake(&self) -> bool {
        matches!(self, Self::V2025_06_18)
    }
}

/// Discovery cache metadata (review 47 §10). Purely discovery-side
/// bookkeeping: CacheMetadata MUST NOT enter ActionProposal, WorkflowStep
/// or Effect — it never leaves launcher-mcp/launcher-core discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetadata {
    /// Server-advertised cache lifetime for the list result, if any
    /// (2026 `tools/list` `ttlMs`).
    pub ttl_ms: Option<u64>,
    /// Cache scope (2026 `cacheScope`): this server only, or shareable.
    pub scope: CacheScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheScope {
    /// Cache is valid only for this server + launcher instance (default:
    /// fail closed — never share caches across servers).
    #[default]
    Server,
    /// Server explicitly marked the list as shareable.
    Shareable,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// COMPAT-PROFILE: parse is exact; unknown revisions fail closed.
    #[test]
    fn profile_parse_fails_closed() {
        assert_eq!(McpProtocolProfile::parse(""), Some(McpProtocolProfile::V2025_06_18));
        assert_eq!(
            McpProtocolProfile::parse("2025-06-18"),
            Some(McpProtocolProfile::V2025_06_18)
        );
        assert_eq!(
            McpProtocolProfile::parse("2026-07-28"),
            Some(McpProtocolProfile::V2026_07_28)
        );
        assert_eq!(McpProtocolProfile::parse("2019-01-01"), None);
        assert_eq!(McpProtocolProfile::parse("2026-07-28 "), Some(McpProtocolProfile::V2026_07_28));
    }

    /// COMPAT-PROFILE: handshake split matches the matrix (§4).
    #[test]
    fn profile_handshake_matrix() {
        assert!(McpProtocolProfile::V2025_06_18.needs_handshake());
        assert!(!McpProtocolProfile::V2026_07_28.needs_handshake());
        assert_eq!(McpProtocolProfile::default(), McpProtocolProfile::V2025_06_18);
    }

    /// COMPAT-PROFILE: cache metadata defaults fail closed (server-scoped).
    #[test]
    fn cache_metadata_defaults() {
        let c = CacheMetadata::default();
        assert_eq!(c.ttl_ms, None);
        assert_eq!(c.scope, CacheScope::Server);
    }
}
