import io

p = "apps/launcher-app/src/main.rs"
s = io.open(p, encoding="utf-8").read()

def sub(old, new, cnt=1):
    global s
    assert old in s, f"anchor missing: {old[:70]!r}"
    s = s.replace(old, new, cnt)

# 1) AppState: context generations + confirmation pending
sub('''    /// Foreground window before the popup took focus (captured at popup
    /// open); restored whenever the popup closes so the user continues where
    /// they were (MVP3.1 focus return).
    prev_foreground: Option<isize>,''',
'''    /// Foreground window before the popup took focus (captured at popup
    /// open); restored whenever the popup closes so the user continues where
    /// they were (MVP3.1 focus return).
    prev_foreground: Option<isize>,
    /// Context generation (MVP3.2-C): bumped on every popup open; results
    /// record the generation they were resolved under. A stale generation
    /// MUST NOT silently execute (INV-043/045).
    context_gen: u64,
    results_gen: u64,
    /// Pending confirmation (MVP3.2-B): (command_id, action_id) awaiting the
    /// second Enter. Confirmation is host-owned execution policy (INV-041).
    pending_confirmation: Option<(String, String)>,''')

sub('''    let state = Arc::new(Mutex::new(AppState {
        core,
        selected: 0,
        prev_foreground: None,
        current_results: Vec::new(),
    }));''',
'''    let state = Arc::new(Mutex::new(AppState {
        core,
        selected: 0,
        prev_foreground: None,
        context_gen: 0,
        results_gen: 0,
        pending_confirmation: None,
        current_results: Vec::new(),
    }));''')

# 2) context refresh bumps generation and binds suggestions to it
sub('''    std::thread::spawn(move || {
        if let Ok(mut st) = state.try_lock() {
            st.prev_foreground = Some(fg_hwnd);
        }
        let folder = if fg_app.as_deref() == Some("explorer.exe") {''',
'''    std::thread::spawn(move || {
        {
            let mut st = state.lock().expect("state lock");
            st.prev_foreground = Some(fg_hwnd);
            // new popup session -> new context generation (MVP3.2-C)
            st.context_gen += 1;
            st.results_gen = st.context_gen;
            st.pending_confirmation = None;
        }
        let folder = if fg_app.as_deref() == Some("explorer.exe") {''')

# 3) search binds results to the generation captured at query time
sub('''        let query_id = session.begin();
        let items = {
            let mut st = state.lock().expect("state lock");
            let r = st.core.search(&query, launcher_core::MAX_RESULTS);
            if !session.is_current(query_id) {
                tracing::debug!(query_id, "query superseded, dropping results");
                return;
            }
            st.current_results = r.commands;
            st.selected = 0;''',
'''        let query_id = session.begin();
        let items = {
            let mut st = state.lock().expect("state lock");
            let results_gen = st.context_gen; // bind results to current context
            let r = st.core.search(&query, launcher_core::MAX_RESULTS);
            if !session.is_current(query_id) {
                tracing::debug!(query_id, "query superseded, dropping results");
                return;
            }
            st.current_results = r.commands;
            st.results_gen = results_gen;
            st.selected = 0;''')

# 4) shared execute helper replaces scattered logic
sub('''fn spawn_execute(''',
'''/// Context staleness guard (MVP3.2-C, INV-043/045): results resolved under
/// an older context generation MUST NOT silently execute.
fn context_is_stale(state: &Arc<Mutex<AppState>>) -> bool {
    let st = state.lock().expect("state lock");
    st.results_gen != st.context_gen
}

/// Execute one action of one command by stable ids (shared by primary Enter,
/// Action Panel selection and shortcut dispatch — all paths are identical
/// below the dispatch, per INV-037/040).
#[allow(clippy::too_many_arguments)]
fn execute_action_by_id(
    state: Arc<Mutex<AppState>>,
    ui_weak: slint::Weak<AppWindow>,
    visible: Arc<AtomicBool>,
    command_id: String,
    action_id: String,
    primary: bool,
) {
    if context_is_stale(&state) {
        tracing::warn!(command = %command_id, "execute.stale_context");
        set_status(ui_weak, "⚠ Context changed - reopen the launcher".into());
        return;
    }
    let resolved = {
        let mut st = state.lock().expect("state lock");
        st.current_results.iter().find(|c| c.id == command_id).and_then(|c| {
            c.actions
                .iter()
                .find(|a| a.id.as_deref() == Some(action_id.as_str()))
                .cloned()
        })
    };
    let Some(mut action) = resolved else { return };
    info!(command = %command_id, action = %action_id, primary, "action.execute");

    // Confirmation policy (MVP3.2-B, INV-041/042): the FIRST attempt arms
    // the pending state; the SECOND Enter on the same action is the host's
    // confirmation and clears the policy flag before the engine runs.
    if action.confirmation_required {
        let mut st = state.lock().expect("state lock");
        if st.pending_confirmation.as_deref() != Some((&command_id, &action_id)) {
            st.pending_confirmation = Some((command_id.clone(), action_id.clone()));
            drop(st);
            set_status(ui_weak, "⚠ Confirm: press Enter again".into());
            return;
        }
        st.pending_confirmation = None;
        action.confirmation_required = false;
    }

    std::thread::spawn(move || {
        match launcher_action::execute(&action) {
            Ok(Effect::Launched(_) | Effect::Copied(_) | Effect::Revealed(_) | Effect::Pasted) => {
                let prev = state.lock().ok().and_then(|st| st.prev_foreground);
                close_and_restore(ui_weak.clone(), visible.clone(), prev);
            }
            Ok(_) => {}
            // committed execution: Esc does not cancel an in-flight effect
            Err(e) => {
                tracing::warn!(error = %e, "action.failed");
                set_status(ui_weak, format!("⚠ {e}"));
            }
        }
    });
}

fn spawn_execute(''')

# 5) spawn_execute delegates to the shared helper
sub('''    std::thread::spawn(move || {
        let cmd = {
            let st = state.lock().expect("state lock");
            st.command_for_id(&command_id)
        };
        let Some(cmd) = cmd else { return };
        info!(command = %cmd.id, "action.execute");
        if let Some(action) = cmd.primary_action() {
            match launcher_action::execute(action) {
                Ok(Effect::Launched(t) | Effect::Revealed(t)) => {
                    info!(target = %t, "action done");
                    let prev = state.lock().ok().and_then(|st| st.prev_foreground);
                    close_and_restore(ui_weak.clone(), visible.clone(), prev);
                }
                Ok(_) => {}
                // MVP3.1 result feedback: failures stay visible in the popup
                Err(e) => {
                    tracing::warn!(error = %e, "action failed");
                    set_status(ui_weak.clone(), format!("⚠ {e}"));
                }
            }
        }
        // bounded history for recency signals
        if let Ok(mut st) = state.try_lock() {
            st.core.record_use(&cmd.id, &cmd.provider_id);
        }
    });
}''',
'''    // Primary Enter = the first Ready action, dispatched through the exact
    // same id-based path as the Action Panel (INV-037/040).
    let action_id = {
        let st = state.lock().expect("state lock");
        st.command_for_id(&command_id)
            .and_then(|c| c.primary_action().and_then(|a| a.id.clone()))
    };
    match action_id {
        Some(id) => execute_action_by_id(state, ui_weak, visible, command_id, id, true),
        // informational command: Enter does nothing, no error
        None => tracing::debug!(command = %command_id, "informational command, no primary"),
    }
}''')

# 6) ActionItem init gains shortcut
sub('''        .map(|a| launcher_ui::ActionItem {
            action_id: a.action_id.into(),
            title: a.title.into(),
            enabled: a.enabled,
            reason: a.reason.into(),
        })''',
'''        .map(|a| launcher_ui::ActionItem {
            action_id: a.action_id.into(),
            title: a.title.into(),
            enabled: a.enabled,
            reason: a.reason.into(),
            shortcut: a.shortcut.into(),
        })''')

# 7) replace the old on_execute_action block with delegation + shortcut handler
old_block = s[s.index("        ui.on_execute_action(move |command_id, action_id| {"):]
old_block = old_block[:old_block.index("\n    }\n") + 7]
new_block = '''        ui.on_execute_action({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            let visible = visible.clone();
            move |command_id, action_id| {
                execute_action_by_id(
                    state.clone(),
                    ui_weak.clone(),
                    visible.clone(),
                    command_id.to_string(),
                    action_id.to_string(),
                    false,
                );
            }
        });
        // MVP3.2-A: shortcut dispatch resolves to the stable action_id of the
        // selected command; collision rule = first declaration wins
        // (deterministic, independent of widget order — INV-039/040).
        ui.on_action_shortcut({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            let visible = visible.clone();
            move |ch| {
                let want = format!("Ctrl+Shift+{}", ch.to_uppercase());
                let target = {
                    let st = state.lock().expect("state lock");
                    st.current_results.get(st.selected).and_then(|c| {
                        c.actions
                            .iter()
                            .find(|a| {
                                a.disabled_reason.is_none()
                                    && a.shortcut.as_deref() == Some(want.as_str())
                            })
                            .and_then(|a| a.id.clone())
                    })
                };
                if let (Some(cmd), Some(action_id)) = (
                    state.lock().ok().and_then(|st| {
                        st.current_results.get(st.selected).map(|c| c.id.clone())
                    }),
                    target,
                ) {
                    execute_action_by_id(state.clone(), ui_weak.clone(), visible.clone(), cmd, action_id, false);
                }
            }
        });
    }
'''
s = s.replace(old_block, new_block)

io.open(p, "w", encoding="utf-8", newline="\n").write(s)
print("main ok")
