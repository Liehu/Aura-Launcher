//! P3-A UI State Architecture (P3.2-UIUX / P3-A task): an explicit,
//! deterministic UI state machine so Launcher, Plugin Context, Preview,
//! Context Menu, Plugin Center and Settings are clearly separated UX
//! states — never ad-hoc boolean layout switches (spec §4.1 hard rule).
//!
//! This module is PRESENTATION STATE ONLY: it owns no execution authority,
//! no plugin trust/capability decisions, and never executes commands. The
//! UI layer drives it; effects still flow exclusively through the frozen
//! ActionResolver path (spec §31 frozen architecture boundary).

/// Launcher sub-states. `Search` is the only state that shows the search
/// box + result list; management UI must never appear inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherState {
    Search,
    /// `→ Actions` panel over a selected result.
    ActionPanel {
        command_id: String,
    },
}

/// Parent context states opened FROM the launcher (or tray). Each state
/// remembers its parent so Esc/Back always returns to a safe parent
/// (spec §4: PluginCenter/Settings close to the previous state; Esc from
/// PluginContext returns to LauncherSearch, never quits the launcher).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiState {
    Closed,
    Launcher(LauncherState),
    /// In-launcher plugin context (spec §10): same window, scoped query,
    /// Esc → LauncherSearch. NOT a second launcher UI.
    PluginContext {
        plugin_id: String,
        /// LauncherSearch query that was active when the context opened;
        /// Esc restores it (spec §10.1 — back, not quit).
        parent_query: String,
        /// Current in-scope query text.
        query: String,
        /// Business id of the selected in-scope result ("" when none).
        selected_result: String,
    },
    /// Detail/preview surface for a selected result (F1 / Tab detail pane).
    Preview {
        command_id: String,
    },
    /// Right-click context menu over a result; shares the unified action
    /// model with the `→` panel (spec §12 — one action model, never two).
    ContextMenu {
        command_id: String,
    },
    /// Standalone plugin management window (spec §3.2 / §16 master-detail).
    PluginCenter,
    /// Standalone settings window (spec §3.3 / §17 IA).
    Settings,
}

impl UiState {
    /// PluginContext scope id, when this state IS a plugin context (P3-F).
    pub fn plugin_context_id(&self) -> Option<&str> {
        match self {
            UiState::PluginContext { plugin_id, .. } => Some(plugin_id),
            _ => None,
        }
    }

    /// True for states that own a top-level window/surface (they may be the
    /// root of the machine, opened from Closed or from the tray).
    pub fn is_root(&self) -> bool {
        matches!(
            self,
            UiState::Closed | UiState::Launcher(_) | UiState::PluginCenter | UiState::Settings
        )
    }
}

/// Why a transition was rejected. Illegal transitions are REJECTED, not
/// silently normalized (P3-A tests #6); the caller decides what to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// e.g. Settings → PluginContext, Closed → Preview, PluginCenter →
    /// ActionPanel: the target is not reachable from the current state.
    Illegal { from: UiState, to: UiState },
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransitionError::Illegal { from, to } => {
                write!(f, "illegal ui transition: {from:?} -> {to:?}")
            }
        }
    }
}

/// Explicit navigation state machine (P3-A: "navigation 必须显式且确定性").
///
/// Root states (Launcher / PluginCenter / Settings) remember the state to
/// return to when closed; overlay states (ActionPanel / PluginContext /
/// Preview / ContextMenu) always return to `Launcher(Search)` on Esc —
/// the safe parent (spec §4/§10.1).
#[derive(Debug, Clone)]
pub struct UiStateMachine {
    current: UiState,
    /// Root states to return to, outermost (usually the launcher) first.
    /// A tray-driven root switch pushes the replaced root here so closing
    /// the new root walks back through the previous one.
    root_stack: Vec<UiState>,
}

impl Default for UiStateMachine {
    fn default() -> Self {
        Self {
            current: UiState::Closed,
            root_stack: Vec::new(),
        }
    }
}

impl UiStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> &UiState {
        &self.current
    }

    /// Open a ROOT state (Launcher / PluginCenter / Settings) from Closed
    /// or switch between roots (tray is always one hop, spec §15). Not
    /// reachable from inside an overlay: the overlay's owner window must
    /// Esc first.
    pub fn open_root(&mut self, target: UiState) -> Result<(), TransitionError> {
        if !target.is_root() || matches!(target, UiState::Closed) {
            return Err(TransitionError::Illegal {
                from: self.current.clone(),
                to: target,
            });
        }
        match &self.current {
            UiState::Closed => {}
            UiState::Launcher(_) | UiState::PluginCenter | UiState::Settings => {
                self.root_stack.push(self.current.clone());
            }
            // overlays are not root-switch sources
            _ => {
                return Err(TransitionError::Illegal {
                    from: self.current.clone(),
                    to: target,
                })
            }
        }
        self.current = target;
        Ok(())
    }

    /// Enter an overlay/sub state from `Launcher(Search)`. Overlays are not
    /// reachable from other roots (Plugin Center / Settings own their own
    /// windows and never host launcher overlays).
    pub fn enter_overlay(&mut self, target: UiState) -> Result<(), TransitionError> {
        if !matches!(self.current, UiState::Launcher(LauncherState::Search)) {
            return Err(TransitionError::Illegal {
                from: self.current.clone(),
                to: target,
            });
        }
        match target {
            UiState::Launcher(LauncherState::ActionPanel { .. })
            | UiState::PluginContext { .. }
            | UiState::Preview { .. }
            | UiState::ContextMenu { .. } => {
                self.current = target;
                Ok(())
            }
            _ => Err(TransitionError::Illegal {
                from: self.current.clone(),
                to: target,
            }),
        }
    }

    /// Esc / Back: return to the safe parent state. Returns the new state,
    /// or `None` when the machine left the last root (the caller hides the
    /// window — Closed is the popup-dismissed state).
    pub fn escape(&mut self) -> Option<UiState> {
        let next = match &self.current {
            UiState::Closed => return None,
            // overlays always fall back to the launcher search
            UiState::Launcher(LauncherState::ActionPanel { .. })
            | UiState::PluginContext { .. }
            | UiState::Preview { .. }
            | UiState::ContextMenu { .. } => Some(UiState::Launcher(LauncherState::Search)),
            // roots close to the previous root, or out of the UI entirely
            UiState::Launcher(_) | UiState::PluginCenter | UiState::Settings => {
                self.root_stack.pop()
            }
        };
        match next {
            Some(s) => {
                self.current = s;
                Some(self.current.clone())
            }
            None => {
                self.current = UiState::Closed;
                None
            }
        }
    }

    /// P3-F: update the mutable presentation facts of the ACTIVE plugin
    /// context (in-scope query text / selected result). No-op outside a
    /// PluginContext.
    pub fn set_plugin_context(&mut self, query: &str, selected_result: &str) {
        if let UiState::PluginContext {
            query: q,
            selected_result: sel,
            ..
        } = &mut self.current
        {
            *q = query.to_string();
            *sel = selected_result.to_string();
        }
    }

    /// Clone of the active plugin context's identity, if any (P3-F: the UI
    /// reads identity + presentation facts; nothing else).
    pub fn plugin_context(&self) -> Option<(String, String, String, String)> {
        match &self.current {
            UiState::PluginContext {
                plugin_id,
                parent_query,
                query,
                selected_result,
            } => Some((
                plugin_id.clone(),
                parent_query.clone(),
                query.clone(),
                selected_result.clone(),
            )),
            _ => None,
        }
    }

    /// Force-reset to Closed (popup hidden by hotkey toggle / execute).
    /// Always valid: hiding the window is a physical fact, not a policy.
    pub fn reset_to_closed(&mut self) {
        self.current = UiState::Closed;
        self.root_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // P3-A required test 1: Closed → Launcher
    #[test]
    fn closed_to_launcher() {
        let mut m = UiStateMachine::new();
        assert_eq!(m.current(), &UiState::Closed);
        m.open_root(UiState::Launcher(LauncherState::Search))
            .unwrap();
        assert_eq!(m.current(), &UiState::Launcher(LauncherState::Search));
    }

    // P3-A test 2: Launcher → PluginContext → Esc → Launcher
    #[test]
    fn launcher_plugin_context_escape_returns_to_search() {
        let mut m = launcher();
        m.enter_overlay(UiState::PluginContext {
            plugin_id: "github".into(),
            parent_query: "git".into(),
            query: String::new(),
            selected_result: String::new(),
        })
        .unwrap();
        assert_eq!(m.current().plugin_context_id(), Some("github"));
        assert_eq!(
            m.escape(),
            Some(UiState::Launcher(LauncherState::Search)),
            "Esc from PluginContext returns to LauncherSearch, never quits"
        );
    }

    // P3-A test 3: Launcher → PluginCenter → Back
    #[test]
    fn launcher_plugin_center_back() {
        let mut m = launcher();
        m.open_root(UiState::PluginCenter).unwrap();
        assert_eq!(m.current(), &UiState::PluginCenter);
        assert_eq!(
            m.escape(),
            Some(UiState::Launcher(LauncherState::Search)),
            "PluginCenter closes back to the launcher"
        );
        assert_eq!(m.escape(), None, "Esc on the launcher dismisses (Closed)");
    }

    // P3-A test 4: Launcher → Settings → Back
    #[test]
    fn launcher_settings_back() {
        let mut m = launcher();
        m.open_root(UiState::Settings).unwrap();
        assert_eq!(m.current(), &UiState::Settings);
        assert_eq!(m.escape(), Some(UiState::Launcher(LauncherState::Search)));
    }

    // P3-A test 5: Launcher → Preview → Esc
    #[test]
    fn launcher_preview_escape() {
        let mut m = launcher();
        m.enter_overlay(UiState::Preview {
            command_id: "app:notepad".into(),
        })
        .unwrap();
        assert_eq!(m.escape(), Some(UiState::Launcher(LauncherState::Search)));
    }

    // P3-A test 6: illegal transitions are rejected (not normalized)
    #[test]
    fn illegal_transitions_rejected() {
        let mut m = UiStateMachine::new();
        // overlays are not reachable from Closed
        assert!(matches!(
            m.enter_overlay(UiState::Preview {
                command_id: "x".into()
            }),
            Err(TransitionError::Illegal { .. })
        ));
        // Plugin Center does not host launcher overlays
        let mut pc = launcher();
        pc.open_root(UiState::PluginCenter).unwrap();
        assert!(matches!(
            pc.enter_overlay(UiState::ContextMenu {
                command_id: "x".into()
            }),
            Err(TransitionError::Illegal { .. })
        ));
        // roots cannot be opened from inside an overlay
        let mut ov = launcher();
        ov.enter_overlay(UiState::Preview {
            command_id: "x".into(),
        })
        .unwrap();
        assert!(matches!(
            ov.open_root(UiState::Settings),
            Err(TransitionError::Illegal { .. })
        ));
        // Closed is not a transition target
        let mut m2 = UiStateMachine::new();
        assert!(matches!(
            m2.open_root(UiState::Closed),
            Err(TransitionError::Illegal { .. })
        ));
        assert_eq!(m2.current(), &UiState::Closed, "rejected = unchanged");
    }

    /// ActionPanel is a Launcher sub-state: Esc returns to Search while the
    /// window stays open.
    #[test]
    fn action_panel_esc_returns_to_search_in_place() {
        let mut m = launcher();
        m.enter_overlay(UiState::Launcher(LauncherState::ActionPanel {
            command_id: "app:code".into(),
        }))
        .unwrap();
        assert_eq!(m.escape(), Some(UiState::Launcher(LauncherState::Search)));
    }

    /// Tray one-hop rule (spec §15 + §4 "close → previous state"): from a
    /// replaced root, closing walks back through the previous root first.
    #[test]
    fn tray_root_switch_returns_to_previous_root() {
        let mut m = launcher();
        m.open_root(UiState::Settings).unwrap();
        m.open_root(UiState::PluginCenter).unwrap();
        assert_eq!(
            m.escape(),
            Some(UiState::Settings),
            "back to the replaced root"
        );
        assert_eq!(m.escape(), Some(UiState::Launcher(LauncherState::Search)));
        assert_eq!(m.escape(), None);
    }

    /// Hotkey toggle / execute can always hide: Closed is physical.
    #[test]
    fn reset_to_closed_is_always_valid() {
        let mut m = launcher();
        m.enter_overlay(UiState::ContextMenu {
            command_id: "x".into(),
        })
        .unwrap();
        m.reset_to_closed();
        assert_eq!(m.current(), &UiState::Closed);
        assert_eq!(m.escape(), None);
    }

    fn launcher() -> UiStateMachine {
        let mut m = UiStateMachine::new();
        m.open_root(UiState::Launcher(LauncherState::Search))
            .unwrap();
        m
    }
}
