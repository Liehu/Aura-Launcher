//! Core orchestration: Provider registry, query fan-out, ranking, history.

pub mod agent_host;
pub mod catalog;
pub mod favorites;
pub mod providers;
pub mod search_coordinator;
pub mod workflow_backend;

use std::path::PathBuf;

use std::sync::Mutex;
use std::time::Instant;

use launcher_domain::{
    Action, ActionKind, ActionPayload, Category, Command, ContextSnapshot, QueryContext,
};
use launcher_mcp::compat::McpProtocolProfile;
use launcher_indexer::{IndexedFile, Indexer};
use launcher_search::rank_with_boost;

pub use providers::app::AppProvider;
pub use providers::context::ContextProvider;
pub use providers::file::FileProvider;
pub use providers::plugin::PluginProvider;

/// A Provider discovers Commands (design spec 5.1). It never touches the UI.
pub trait Provider: Send {
    fn id(&self) -> &str;
    fn query(&mut self, q: &QueryContext) -> Vec<Command>;

    /// Manifest id when this provider fronts an external plugin (MVP4.0).
    fn plugin_identity(&self) -> Option<&str> {
        None
    }

    /// Surface a query failure (Main.Error state, UI-CONTRACT §3.1):
    /// providers that fail (plugin spawn/cooldown, indexer IO) report it here
    /// instead of silently returning empty results. Default: no error.
    fn take_last_error(&mut self) -> Option<String> {
        None
    }

    /// Execute a plugin-owned action through the PluginBroker (MVP4.0 /
    /// ADR-0014). Only plugin providers override this; it is always reached
    /// via the ActionEngine result path, never directly from producers.
    /// Errors carry the frozen WorkflowFailureClass (contract §5).
    fn execute_action(
        &mut self,
        _action_id: &str,
        _input: &serde_json::Value,
        _execution_id: &str,
        _context_generation: u64,
    ) -> Result<serde_json::Value, (launcher_domain::workflow::WorkflowFailureClass, String)> {
        Err((
            launcher_domain::workflow::WorkflowFailureClass::InvalidInput,
            "provider does not support execute_action".into(),
        ))
    }
}

/// Result of a full search round.
#[derive(Debug)]
pub struct SearchResult {
    pub commands: Vec<Command>,
    pub elapsed: std::time::Duration,
    /// Per-provider query failures (Main.Error state, UI-CONTRACT §3.1):
    /// empty results + non-empty errors means "search failed", not
    /// "no results".
    pub errors: Vec<String>,
}

/// Bounded default result limit.
pub const MAX_RESULTS: usize = 50;

/// Query supersession guard (ADR-0004).
///
/// Every keystroke starts a new generation; the UI only accepts results whose
/// generation equals the current one, so a slow older query can never
/// overwrite a newer one ("ch → chrome" race).
pub struct SearchSession {
    generation: std::sync::atomic::AtomicU64,
}

impl SearchSession {
    pub fn new() -> Self {
        Self {
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Start a new query and return its id.
    pub fn begin(&self) -> u64 {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1
    }

    /// True when `query_id` is still the newest query (stale results are
    /// dropped by the caller).
    pub fn is_current(&self, query_id: u64) -> bool {
        self.generation.load(std::sync::atomic::Ordering::SeqCst) == query_id
    }
}

impl Default for SearchSession {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Core {
    providers: Vec<Box<dyn Provider>>,
    history: Option<Indexer>,
    /// P2.2-A: user-explicit favorites (semantic identity keys).
    favorites: Option<favorites::FavoriteService>,
    favorite_snapshot: favorites::FavoriteSnapshot,
    /// P2.2-A §102: bumped on every successful favorite/pin mutation.
    user_state_generation: std::sync::atomic::AtomicU64,
    /// P2.2-B: generation-aware query cache over final ranked results.
    search_cache: Mutex<launcher_search::cache::SearchCache>,
    /// P2.2-C: context captured once per search (host-injected).
    search_context: Mutex<Option<launcher_search::ContextSnapshot>>,
    /// P2.2-F: advances only on SEMANTIC context change (review 82 §12).
    context_generation: std::sync::atomic::AtomicU64,
    /// P2.2-F: generation counters completing the cache key contract.
    /// application: plugin lifecycle / app-catalog-visible changes.
    application_generation: std::sync::atomic::AtomicU64,
    /// ranking: reserved for runtime-configurable ranking weights.
    ranking_generation: std::sync::atomic::AtomicU64,
    /// P2.2-F §10-12: last SEMANTIC context state — generation only
    /// advances when this changes (not on every capture).
    last_context_state: Mutex<Option<(Option<String>, Option<String>)>>,
    /// Configured MCP server endpoints (MVP4.3 Phase 7): the executor
    /// registry's `mcp:*` routing table, config-owned.
    mcp_servers: std::collections::HashMap<String, launcher_mcp::executor::ServerEndpoint>,
    /// Optional executor override (tests); the production registry builds
    /// a [`launcher_mcp::executor::StdMcpExecutor`] from `mcp_servers`.
    mcp_executor: Option<std::sync::Arc<dyn launcher_mcp::executor::McpExecutor>>,
    /// Monotonic execution correlation ids (`e-<n>`, review 41 §6.3).
    execution_counter: std::sync::atomic::AtomicU64,
}

impl Core {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            history: None,
            favorites: None,
            favorite_snapshot: Default::default(),
            user_state_generation: std::sync::atomic::AtomicU64::new(0),
            application_generation: std::sync::atomic::AtomicU64::new(0),
            ranking_generation: std::sync::atomic::AtomicU64::new(0),
            last_context_state: Mutex::new(None),
            search_cache: Mutex::new(launcher_search::cache::SearchCache::new(256, 16 * 1024 * 1024)),
            search_context: Mutex::new(None),
            context_generation: std::sync::atomic::AtomicU64::new(0),
            mcp_servers: std::collections::HashMap::new(),
            mcp_executor: None,
            execution_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Optional bounded execution-history sink (SQLite `history` table).
    pub fn set_history(&mut self, indexer: Indexer) {
        self.history = Some(indexer);
    }

    /// Record a command execution; bounded to 10k rows inside the indexer.
    pub fn record_use(&mut self, command_id: &str, provider_id: &str) {
        self.record_use_with_title(command_id, provider_id, "");
    }

    /// [`record_use`] with the command title (P2-D recency views).
    pub fn record_use_with_title(&mut self, command_id: &str, provider_id: &str, title: &str) {
        // P2.2-B §125: usage ranking state changes invalidate cached results
        self.user_state_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.history.as_ref() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let _ = h.record_use_titled(command_id, provider_id, title, now);
        }
    }

    /// Usage map from the history sink: ((provider_id, command_id) ->
    /// (count, last_used_ms)). Empty when no sink is configured or the
    /// read fails (ranking silently degrades to lexical-only).
    fn usage_snapshot(&self) -> std::collections::HashMap<(String, String), (u32, i64)> {
        let mut map = std::collections::HashMap::new();
        if let Some(h) = self.history.as_ref() {
            if let Ok(rows) = h.usage_map() {
                for e in rows {
                    map.insert((e.provider_id, e.command_id), (e.uses, e.last_used_ms));
                }
            }
        }
        map
    }

    /// Deterministic usage boost for one command (P2-D): frequency
    /// (capped) + recency. Max ≈ 42 — enough to reorder within a match
    /// class (contains vs prefix), never enough to beat an exact match.
    fn usage_boost(usage: &std::collections::HashMap<(String, String), (u32, i64)>) -> impl Fn(&Command) -> f32 + '_ {
        use launcher_search::RankingWeights;
        let w = RankingWeights::default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        move |c: &Command| {
            let Some(&(uses, last)) = usage.get(&(c.provider_id.clone(), c.id.clone())) else {
                return 0.0;
            };
            let freq = w.history_freq_per_use * uses.clamp(0, w.history_freq_cap) as f32;
            let recency = if now - last < 24 * 3600 * 1000 {
                w.history_recency_day
            } else if now - last < 7 * 24 * 3600 * 1000 {
                w.history_recency_week
            } else {
                0.0
            };
            freq + recency
        }
    }

    /// P2.2-A: attach the favorites store and load its ranking snapshot.
    pub fn set_favorites(&mut self, svc: favorites::FavoriteService) {
        self.refresh_favorite_snapshot();
        self.favorites = Some(svc);
    }

    /// Reload the favorite ranking snapshot after a mutation.
    pub fn refresh_favorite_snapshot(&mut self) {
        if let Some(fav) = self.favorites.as_ref() {
            if let Ok(snap) = fav.snapshot() {
                self.favorite_snapshot = snap;
            }
        }
    }

    /// Toggle favorite for one command's semantic identity. Returns the new
    /// favorite state (true = now a favorite).
    pub fn toggle_favorite(&mut self, c: &Command) -> Option<bool> {
        let key = launcher_search::search_identity_key(c);
        let svc = self.favorites.as_ref()?;
        let snap = self.favorite_snapshot.clone();
        let result = if snap.is_favorite(&key) {
            svc.remove(&key).map(|_| false)
        } else {
            svc.add(&key).map(|_| true)
        };
        if let Ok(now_fav) = result {
            self.user_state_generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.refresh_favorite_snapshot();
            return Some(now_fav);
        }
        None
    }

    pub fn favorite_snapshot(&self) -> favorites::FavoriteSnapshot {
        self.favorite_snapshot.clone()
    }

    /// P2.2-C (review 82 §10-12): host captures semantic context per popup
    /// show; the CONTEXT GENERATION only advances when the SEMANTIC state
    /// (foreground, folder) actually changes — repeated identical captures
    /// keep the generation stable so cache keys remain valid.
    pub fn set_search_context(&self, ctx: Option<launcher_search::ContextSnapshot>) {
        let semantic = ctx.as_ref().map(|c| {
            (
                c.foreground_app.clone(),
                c.current_folder
                    .as_ref()
                    .map(|f| launcher_domain::normalize_path_identity(f)),
            )
        });
        let changed = {
            let mut last = self.last_context_state.lock().expect("ctx state");
            let changed = *last != semantic;
            if changed {
                *last = semantic.clone();
            }
            changed
        };
        if changed {
            self.context_generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        *self.search_context.lock().expect("ctx") = ctx;
    }

    pub fn context_generation(&self) -> u64 {
        self.context_generation.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// P2.2-F: application-visible lifecycle changes (plugin enable/disable/
    /// quarantine, catalog refresh) bump this so cached results miss.
    pub fn bump_application_generation(&self) {
        self.application_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// P2.4-A05: adopt the committed CatalogGeneration as the cache-visible
    /// application generation, so a catalog reconcile invalidates cached
    /// application results (cache keys must include every invalidating
    /// generation).
    pub fn set_application_generation(&self, gen: u64) {
        self.application_generation
            .store(gen, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn register(&mut self, p: Box<dyn Provider>) {
        self.providers.push(p);
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Mutable provider access for the workflow backend (MVP4.1).
    pub fn providers_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Provider>> {
        self.providers.iter_mut()
    }

    /// Fan the query out to all providers sequentially, then rank.
    /// Sequential keeps results deterministic; providers are in-memory
    /// or local-SQLite so per-query cost stays in the microsecond range.
    pub fn search(&mut self, raw_query: &str, limit: usize) -> SearchResult {
        let started = Instant::now();
        // E11 (review 89 §12): cap query length — unbounded user input can
        // cause OOM in tokenisation and ranking. 512 chars covers all
        // legitimate use cases; longer input is truncated.
        let truncated: String = raw_query.chars().take(512).collect();
        let q = QueryContext::parse(&truncated);
        let limit = limit.min(MAX_RESULTS);
        // P2.2-B: generation-aware cache lookup BEFORE provider dispatch
        let env = launcher_search::cache::SearchCacheEnvironment {
            file_generation: self
                .history
                .as_ref()
                .and_then(|h| h.generation().ok())
                .unwrap_or(0),
            application_generation: self
                .application_generation
                .load(std::sync::atomic::Ordering::SeqCst),
            user_state_generation: self
                .user_state_generation
                .load(std::sync::atomic::Ordering::SeqCst),
            context_generation: self
                .context_generation
                .load(std::sync::atomic::Ordering::SeqCst),
            ranking_generation: self
                .ranking_generation
                .load(std::sync::atomic::Ordering::SeqCst),
        };
        let cache_key = launcher_search::cache::SearchCacheKey::new(&q.normalized, env);
        {
            let mut cache = self.search_cache.lock().expect("search cache");
            if let Some(cached) = cache.get(&cache_key) {
                tracing::debug!(query = raw_query, "search.cache_hit");
                return SearchResult {
                    commands: cached,
                    elapsed: started.elapsed(),
                    // provider errors are unknown/not applicable on a hit
                    errors: Vec::new(),
                };
            }
        }

        let mut all: Vec<Command> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for p in self.providers.iter_mut() {
            all.extend(p.query(&q));
            if let Some(e) = p.take_last_error() {
                errors.push(format!("{}: {}", p.id(), e));
            }
        }
        let usage = self.usage_snapshot();
        let favorite_snapshot = self.favorite_snapshot.clone();
        let context = self.search_context.lock().ok().and_then(|g| g.clone());
        let favorite_and_context_boost = move |c: &Command| -> f32 {
            let usage_boost = Self::usage_boost(&usage)(c);
            // P2.2-A §25/§26: favorites are a bounded ranking signal
            let mut fav = 0.0;
            let key = launcher_search::search_identity_key(c);
            if favorite_snapshot.is_pinned(&key) {
                fav += 20.0;
            } else if favorite_snapshot.is_favorite(&key) {
                fav += 10.0;
            }
            // P2.2-C §28: context is additive and never creates candidates
            let ctx_boost = context
                .as_ref()
                .map(|ctx| launcher_search::context_score(c, ctx, &Default::default()))
                .unwrap_or(0.0);
            usage_boost + fav + ctx_boost
        };
        let commands = rank_with_boost(all, &q, limit, favorite_and_context_boost);
        // P2.2-B §89: cache the FINAL ranked snapshot (negative = short TTL)
        {
            let mut cache = self.search_cache.lock().expect("search cache");
            let ttl = if commands.is_empty() {
                launcher_search::cache::CacheTtl::Negative
            } else {
                launcher_search::cache::CacheTtl::Complete
            };
            cache.put(cache_key, commands.clone(), ttl);
        }
        tracing::debug!(query = raw_query, elapsed = ?started.elapsed(), "query.completed");
        SearchResult {
            commands,
            elapsed: started.elapsed(),
            errors,
        }
    }
}

impl Core {
    /// The unified ActionCatalog for AI planning (MVP4.3 Phase 9, review 44
    /// §4): fresh empty-text discovery across EVERY provider — built-in,
    /// plugin and MCP — as raw Commands. This is a capability DESCRIPTION
    /// view: visibility here is not authorization; disabled actions are
    /// described, and the Resolver still decides everything at execution.
    /// Empty-query view (P2.1 §57/58): the most recently/frequently used
    /// commands, re-joined against the live catalog so every entry carries
    /// its full executable actions (history rows alone only have ids+title).
    /// Deterministic: usage score DESC, then provider/id. Empty until the
    /// user has executed something at least once.
    pub fn recent_commands(&mut self, limit: usize) -> Vec<Command> {
        let usage = self.usage_snapshot();
        if usage.is_empty() {
            return Vec::new();
        }
        let boost = Self::usage_boost(&usage);
        let mut scored: Vec<Command> = self
            .action_catalog()
            .into_iter()
            .filter_map(|mut c| {
                let b = boost(&c);
                (b > 0.0).then(|| {
                    c.score = 100.0 + b; // recency view has no lexical base
                    c
                })
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.provider_id.cmp(&b.provider_id))
                .then_with(|| a.id.cmp(&b.id))
        });
        scored.truncate(limit.min(MAX_RESULTS));
        scored
    }

    pub fn action_catalog(&mut self) -> Vec<Command> {
        let q = QueryContext::parse("");
        let mut out = Vec::new();
        for p in self.providers.iter_mut() {
            out.extend(p.query(&q));
        }
        out
    }

    /// Planner-facing catalog items (review 44 §7/§26): lossy, neutral
    /// projection — no presentation or authority state survives.
    pub fn action_catalog_items(&mut self) -> Vec<launcher_workflow::proposal::ActionCatalogItem> {
        self.action_catalog()
            .iter()
            .flat_map(launcher_workflow::proposal::ActionCatalogItem::items_from_command)
            .collect()
    }

    /// Route a validated `plugin.*` action to its broker (MVP4.0). The
    /// plugin is addressed by its host-assigned manifest id (INV-029):
    /// producers can never address or impersonate another provider.
    pub fn execute_plugin_action(
        &mut self,
        plugin_id: &str,
        action_id: &str,
        input: &serde_json::Value,
        execution_id: &str,
        context_generation: u64,
    ) -> Result<serde_json::Value, (launcher_domain::workflow::WorkflowFailureClass, String)> {
        for p in self.providers.iter_mut() {
            if p.plugin_identity() == Some(plugin_id) {
                return p.execute_action(action_id, input, execution_id, context_generation);
            }
        }
        Err((
            launcher_domain::workflow::WorkflowFailureClass::PluginUnavailable,
            format!("unknown plugin: {plugin_id}"),
        ))
    }
}

/// Effect routing (MVP4.3 Phase 7, review 41 §7.3): Core is the assembly
/// point of the EffectExecutorRegistry. The ActionEngine only ever emits an
/// `Effect::PluginInvoked` (routing classification); the registry maps the
/// effect's namespace (`mcp:*` / `plugin:*`) onto the executor that owns it.
/// ActionEngine and McpExecutor never meet directly (§7.4).
impl Core {
    /// Register one configured MCP server endpoint for the executor
    /// (legacy profile).
    pub fn register_mcp_server(&mut self, id: &str, program: &str, args: Vec<String>) {
        self.register_mcp_server_with_profile(id, program, args, McpProtocolProfile::default());
    }

    /// Register a Streamable HTTP MCP endpoint (P0-B): 2026 stateless
    /// profile only.
    pub fn register_mcp_http_server(
        &mut self,
        id: &str,
        url: &str,
        allow_private_network: bool,
        allow_plain_http: bool,
    ) {
        self.mcp_servers.insert(
            id.to_string(),
            launcher_mcp::executor::ServerEndpoint::http(url, allow_private_network, allow_plain_http),
        );
    }

    /// Register an endpoint with an explicit P1-A runtime mode.
    pub fn register_mcp_server_with_runtime(
        &mut self,
        id: &str,
        endpoint: launcher_mcp::executor::ServerEndpoint,
    ) {
        self.mcp_servers.insert(id.to_string(), endpoint);
    }

    /// Register one configured MCP server endpoint under an explicit wire
    /// profile (Phase 11): the profile stays inside launcher-mcp.
    pub fn register_mcp_server_with_profile(
        &mut self,
        id: &str,
        program: &str,
        args: Vec<String>,
        profile: McpProtocolProfile,
    ) {
        self.mcp_servers.insert(
            id.to_string(),
            launcher_mcp::executor::ServerEndpoint::stdio(program, args).with_profile(profile),
        );
    }

    /// Executor override (tests). Production routing builds a
    /// `StdMcpExecutor` from `mcp_servers` on first use.
    pub fn set_mcp_executor(&mut self, ex: std::sync::Arc<dyn launcher_mcp::executor::McpExecutor>) {
        self.mcp_executor = Some(ex);
    }

    pub fn mcp_server_count(&self) -> usize {
        self.mcp_servers.len()
    }

    /// Fresh execution correlation id (`e-<n>`), preserved end-to-end into
    /// the MCP transport (MCP-058, review 41 §6.3).
    pub fn next_execution_id(&self) -> String {
        let n = self
            .execution_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        format!("e-{n}")
    }

    fn mcp_executor(&self) -> Option<std::sync::Arc<dyn launcher_mcp::executor::McpExecutor>> {
        if let Some(ex) = &self.mcp_executor {
            return Some(ex.clone());
        }
        if self.mcp_servers.is_empty() {
            return None;
        }
        let servers: std::collections::HashMap<String, launcher_mcp::executor::ServerEndpoint> =
            self.mcp_servers
                .iter()
                .map(|(id, endpoint)| (id.clone(), endpoint.clone()))
                .collect();
        Some(std::sync::Arc::new(launcher_mcp::executor::StdMcpExecutor::new(
            servers,
            std::time::Duration::from_millis(5000),
        )))
    }

    /// Execute one approved MCP invocation through the registry's McpExecutor.
    /// Input shape is re-validated here (defense line 2, review 41 §7.5);
    /// authorization already happened in the resolver.
    pub fn execute_mcp_action(
        &mut self,
        input: &serde_json::Value,
        execution_id: &str,
    ) -> Result<serde_json::Value, (launcher_domain::workflow::WorkflowFailureClass, String)> {
        use launcher_domain::workflow::WorkflowFailureClass as Class;
        let parsed = launcher_mcp::executor::McpInvokeInput::from_json(input).map_err(|e| {
            (e.failure_class(), e.to_string())
        })?;
        if !self.mcp_servers.contains_key(parsed.server_id.as_str()) {
            return Err((
                Class::PluginUnavailable,
                format!("unknown mcp server: {}", parsed.server_id),
            ));
        }
        let ex = self.mcp_executor().ok_or_else(|| {
            (Class::PluginUnavailable, "no mcp executor configured".to_string())
        })?;
        let result = ex
            .execute(&parsed, execution_id)
            .map_err(|e| (e.failure_class(), e.to_string()))?;
        // tool business error (isError) = BusinessError, never a protocol
        // violation (review 41 §6.5)
        if let Some(msg) = result.business_error() {
            return Err((Class::BusinessError, msg));
        }
        let content: Vec<&launcher_mcp::executor::McpContent> = result.content.iter().collect();
        Ok(serde_json::json!({
            "server_id": parsed.server_id.as_str(),
            "tool_name": parsed.tool_name,
            "content": content,
            "structured_content": result.structured_content,
        }))
    }

    /// The single effect dispatch point (review 41 §7.3): route a
    /// validated `Effect::PluginInvoked` outcome by the host-assigned
    /// provider namespace onto the owning executor. `execution_id` is
    /// minted ONCE per attempt by the caller (review 42 §12: attempt ↔
    /// execution_id is 1:1; the registry never re-mints).
    pub fn execute_effect(
        &mut self,
        provider_id: &str,
        action_id: &str,
        input: &serde_json::Value,
        execution_id: &str,
        context_generation: u64,
    ) -> Result<serde_json::Value, (launcher_domain::workflow::WorkflowFailureClass, String)> {
        use launcher_domain::workflow::WorkflowFailureClass as Class;
        if let Some(server) = provider_id.strip_prefix("mcp:") {
            // defense-in-depth (review 41 §7.5): the host-assigned provider
            // namespace must agree with the input's config-owned server_id
            if let Some(sid) = input.get("server_id").and_then(|v| v.as_str()) {
                if sid != server {
                    // Phase 10 (review 45 A-002): cross-server confused
                    // deputy is a protocol-boundary violation, not a mere
                    // input shape error — Stop, no retry.
                    return Err((
                        Class::ProtocolViolation,
                        format!("provider/server mismatch: {provider_id} vs {sid}"),
                    ));
                }
            }
            return self.execute_mcp_action(input, execution_id);
        }
        if let Some(plugin) = provider_id.strip_prefix("plugin:") {
            // P1-FIX-01: the caller-minted execution_id is authoritative
            // end-to-end ("registry never re-mints") — no second mint here.
            return self.execute_plugin_action(
                plugin,
                action_id,
                input,
                execution_id,
                context_generation,
            );
        }
        Err((
            Class::ProtocolViolation,
            format!("effect has no host-assigned provider namespace: {provider_id}"),
        ))
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a Command from an indexed file entry (shared by file provider).
pub fn file_command(f: &IndexedFile) -> Command {
    let kind = ActionKind::Open;
    Command {
        id: f.path.clone(),
        title: f.name.clone(),
        subtitle: Some(f.path.clone()),
        icon: None,
        provider_id: "files".into(),
        score: 0.0,
        keywords: vec![f.name.clone()],
        category: if f.is_dir {
            Category::Folder
        } else {
            Category::File
        },
        actions: vec![
            Action {
                kind,
                payload: Some(ActionPayload::Path(f.path.clone())),
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            },
            Action {
                kind: ActionKind::Reveal,
                payload: Some(ActionPayload::Path(f.path.clone())),
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            },
            // P2-C: copy the full path as text for shell/clipboard use.
            Action {
                kind: ActionKind::Copy,
                payload: Some(ActionPayload::Path(f.path.clone())),
                id: Some("copypath".into()),
                title: Some("Copy path".into()),
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            },
        ],
        target: Some(f.path.clone()),
    }
}

/// Context-aware extra commands (design spec 6 / test plan 4 "Context"):
/// offered when Explorer context is present.
pub fn context_commands(snapshot: &ContextSnapshot) -> Vec<Command> {
    let Some(folder) = snapshot.current_folder.clone() else {
        return Vec::new();
    };
    vec![
        Command {
            id: "ctx:terminal".into(),
            title: "Open Terminal Here".into(),
            subtitle: Some(folder.clone()),
            icon: None,
            provider_id: "context".into(),
            score: 0.0,
            keywords: vec!["terminal".into(), "cmd".into()],
            category: Category::Command,
            actions: vec![Action {
                kind: ActionKind::OpenTerminalHere,
                payload: Some(ActionPayload::Path(folder.clone())),

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(folder.clone()),
        },
        Command {
            id: "ctx:copypath".into(),
            title: "Copy Folder Path".into(),
            subtitle: Some(folder.clone()),
            icon: None,
            provider_id: "context".into(),
            score: 0.0,
            keywords: vec!["copy".into(), "path".into()],
            category: Category::Command,
            actions: vec![Action {
                kind: ActionKind::Copy,
                payload: Some(ActionPayload::Path(folder.clone())),

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(folder.clone()),
        },
    ]
}

/// Default app search directories (Start Menu, common install locations).
pub fn default_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let mut p = PathBuf::from(appdata);
        p.push(r"Microsoft\Windows\Start Menu\Programs");
        dirs.push(p);
    }
    if let Some(pd) = std::env::var_os("ProgramData") {
        let mut p = PathBuf::from(pd);
        p.push(r"Microsoft\Windows\Start Menu\Programs");
        dirs.push(p);
    }
    dirs.retain(|p| p.exists());
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::QueryContext;

    struct StaticProvider {
        id: String,
        cmds: Vec<Command>,
    }

    impl Provider for StaticProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            self.cmds.clone()
        }
    }

    fn app(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    #[test]
    fn search_merges_and_ranks_providers() {
        let mut core = Core::new();
        core.register(Box::new(StaticProvider {
            id: "apps".into(),
            cmds: vec![app("1", "Visual Studio Code"), app("2", "Terminal")],
        }));
        core.register(Box::new(StaticProvider {
            id: "files".into(),
            cmds: vec![app("f1", "code_notes.txt")],
        }));
        let r = core.search("code", 10);
        // word-exact "code" (app) outranks prefix "code_notes.txt" (file);
        // tie is broken by provider id, "apps" < "files"
        assert_eq!(r.commands[0].title, "Visual Studio Code");
        assert!(r.commands.iter().any(|c| c.title == "code_notes.txt"));
        assert!(r.elapsed < std::time::Duration::from_secs(1));
    }

    #[test]
    fn history_recording_is_optional_and_safe() {
        let mut core = Core::new();
        core.record_use("x", "apps"); // no sink: no-op, must not panic
        core.set_history(Indexer::in_memory().unwrap());
        core.record_use("x", "apps");
        core.record_use("x", "apps");
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let mut core = Core::new();
        core.register(Box::new(StaticProvider {
            id: "apps".into(),
            cmds: vec![app("1", "x")],
        }));
        assert!(core.search("", 10).commands.is_empty());
    }

    #[test]
    fn search_session_supersedes_stale_queries() {
        // simulates: begin("ch") -> begin("chrome") -> "ch" result arrives late
        let session = SearchSession::new();
        let ch = session.begin();
        assert!(session.is_current(ch));
        let chrome = session.begin();
        assert_ne!(ch, chrome);
        assert!(!session.is_current(ch), "stale query must be superseded");
        assert!(session.is_current(chrome));
    }

    #[test]
    fn context_commands_need_folder() {
        assert!(context_commands(&ContextSnapshot::default()).is_empty());
        let s = ContextSnapshot {
            current_folder: Some("C:\\p".into()),
            ..Default::default()
        };
        let cmds = context_commands(&s);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].id, "ctx:terminal");
    }
}

#[cfg(test)]
mod recent_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category, QueryContext};

    struct StaticProvider {
        cmds: Vec<Command>,
    }
    impl Provider for StaticProvider {
        fn id(&self) -> &str {
            "apps"
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            self.cmds.clone()
        }
    }
    fn cmd(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    /// P2.1 §57/58: empty-query recents come from the history sink, joined
    /// against the catalog so entries stay executable.
    #[test]
    fn recent_commands_returns_used_commands_with_actions() {
        let mut core = Core::new();
        core.set_history(Indexer::in_memory().unwrap());
        core.register(Box::new(StaticProvider {
            cmds: vec![cmd("chrome", "Google Chrome"), cmd("never-used", "Other")],
        }));
        core.record_use_with_title("chrome", "apps", "Google Chrome");
        let recents = core.recent_commands(10);
        assert_eq!(recents.len(), 1, "only used commands surface");
        assert_eq!(recents[0].id, "chrome");
        assert!(!recents[0].actions.is_empty(), "entries must be executable");
    }

    #[test]
    fn recent_commands_empty_without_history() {
        let mut core = Core::new();
        core.register(Box::new(StaticProvider { cmds: vec![cmd("x", "X")] }));
        assert!(core.recent_commands(10).is_empty());
    }
}

#[cfg(test)]
mod p22f_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category, QueryContext};

    static COUNTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    struct CountingProvider {
        cmds: Vec<Command>,
    }
    impl Provider for CountingProvider {
        fn id(&self) -> &str {
            "files"
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            COUNTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.cmds.clone()
        }
    }

    fn file_cmd(path: &str) -> Command {
        Command {
            id: path.into(),
            title: path.into(),
            subtitle: None,
            icon: None,
            provider_id: "files".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::File,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(path.into()),
        }
    }

    /// P2.2-F (review 82 §10-12): context semantic change → generation bump
    /// → cache miss (provider re-queried); identical capture → cache hit.
    #[test]
    fn context_semantic_change_invalidates_cache() {
        let mut core = Core::new();
        core.register(Box::new(CountingProvider {
            cmds: vec![file_cmd(r"C:\Projects\readme.md")],
        }));
        core.set_search_context(Some(launcher_search::ContextSnapshot {
            foreground_app: None,
            current_folder: Some(r"C:\Projects".into()),
        }));
        core.search("readme", 10);
        core.search("readme", 10); // same semantic context → cache hit
        core.set_search_context(Some(launcher_search::ContextSnapshot {
            foreground_app: None,
            current_folder: Some(r"D:\Archive".into()),
        }));
        core.search("readme", 10); // semantic change → miss → re-query
        // identical recapture → same generation → cache hit again
        core.set_search_context(Some(launcher_search::ContextSnapshot {
            foreground_app: None,
            current_folder: Some(r"D:\Archive".into()),
        }));
        core.search("readme", 10);
        // counting provider is the only one registered and search dispatch
        // is sequential, so total provider calls == 2 (miss, miss; the two
        // same-context queries were cache hits)
        let calls = COUNTS.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(calls, 2, "same context hits cache; changed context misses");
        assert!(core.context_generation() >= 1);
    }
}
