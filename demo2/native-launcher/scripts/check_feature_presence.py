#!/usr/bin/env python3
"""Feature Presence Verification (P3-J gate, ADR-P3I round).

The workspace once passed 410 tests while a cleanup revert had silently
removed shipped features (ADR-0019 chain). "tests pass" is NOT evidence
that features still exist. This gate greps the REQUIRED SYMBOLS of each
P3 task directly in source and fails loudly when any is missing.

Usage: python scripts/check_feature_presence.py   (exit 1 on any miss)
"""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# task -> list of (description, relative file, required substring)
CHECKS: list[tuple[str, str, str]] = [
    # P3-B keyboard matrix
    ("P3-B", "crates/launcher-ui/ui/app.slint", "Key.F1"),
    ("P3-B", "crates/launcher-ui/ui/app.slint", "Key.Home"),
    ("P3-B", "crates/launcher-ui/ui/app.slint", "Key.End"),
    ("P3-B", "crates/launcher-ui/ui/app.slint", "secondary-action-requested"),
    ("P3-B", "crates/launcher-ui/ui/app.slint", "open-location-requested"),
    ("P3-B", "apps/launcher-app/src/main.rs", "on_secondary_action_requested"),
    ("P3-B", "apps/launcher-app/src/main.rs", "on_open_location_requested"),
    # P3-C ranking calibration
    ("P3-C", "crates/launcher-search/src/lib.rs", "title_word"),
    ("P3-C", "crates/launcher-search/src/lib.rs", "plugin_hint_matched"),
    ("P3-C", "crates/launcher-search/src/lib.rs", "p3c_f4_ties_are_deterministic"),
    # P3-D grouping + focus model
    ("P3-D", "crates/launcher-ui/src/lib.rs", "pub fn group_sections"),
    ("P3-D", "crates/launcher-ui/src/lib.rs", "pub fn cursor_step"),
    ("P3-D", "crates/launcher-ui/src/lib.rs", "ResultSection"),
    # P3-E context menu on the unified action path
    ("P3-E", "crates/launcher-ui/ui/result-row.slint", "context-requested"),
    ("P3-E", "crates/launcher-ui/ui/result-list.slint", "context-menu"),
    # P3-F plugin context scope
    ("P3-F", "crates/launcher-core/src/lib.rs", "pub fn search_plugin_scoped"),
    ("P3-F", "crates/launcher-ui/src/ui_state.rs", "PluginContext {"),
    ("P3-F", "apps/launcher-app/src/main.rs", "on_plugin_context_requested"),
    ("P3-F", "apps/launcher-app/src/main.rs", "on_plugin_context_exited"),
    # P3-H tray one-hop entries
    ("P3-H", "crates/launcher-ui/ui/app.slint", "settings-requested"),
    ("P3-H", "apps/launcher-app/src/main.rs", "on_settings_requested"),
    ("P3-H", "apps/launcher-app/src/management.rs", "pub fn open_tab"),
    # P3-I visual system
    ("P3-I", "crates/launcher-ui/ui/result-row.slint", "SectionHeader"),
    ("P3-I", "crates/launcher-ui/ui/result-row.slint", "keyboard-focused"),
    ("P3-I", "crates/launcher-ui/ui/result-row.slint", "bg-hover-row"),
    ("P3-I", "crates/launcher-ui/src/lib.rs", "pub fn result_nav"),
    ("P3-I", "crates/launcher-ui/src/lib.rs", "ResultListRow"),
    # ADR-0019 plugin lifetime chain
    ("ADR-0019", "crates/launcher-domain/src/lib.rs", "pub enum PluginLifetime"),
    ("ADR-0019", "crates/launcher-plugin-host/src/lifetime.rs", "PluginLifetimePolicy"),
    ("ADR-0019", "crates/launcher-plugin-host/src/lib.rs", "fn finish_invocation"),
    ("ADR-0019", "crates/launcher-core/src/providers/plugin.rs", "idle_expired"),
    ("ADR-0019", "apps/launcher-bench/src/main.rs", "run_plugin_lifetime"),
]


def main() -> int:
    failures: list[str] = []
    for task, rel, needle in CHECKS:
        path = ROOT / rel
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as e:
            failures.append(f"[{task}] MISSING FILE {rel}: {e}")
            continue
        if needle not in text:
            failures.append(f"[{task}] {rel}: required symbol not found: {needle!r}")
    if failures:
        print("FEATURE PRESENCE: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"FEATURE PRESENCE: OK ({len(CHECKS)} checks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
