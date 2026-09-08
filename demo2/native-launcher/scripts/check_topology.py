#!/usr/bin/env python3
"""Architecture topology check (INV-015).

Verifies that workspace.members in Cargo.toml, README.md and ARCHITECTURE.md
agree on the set of crates/apps, so architecture docs stay machine-verifiable.
Exits 1 on any mismatch.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def workspace_members() -> set[str]:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf8")
    block = re.search(r"members\s*=\s*\[(.*?)\]", text, re.S).group(1)
    return set(re.findall(r'"([^"]+)"', block))


def crate_names() -> set[str]:
    return {m.split("/")[-1] for m in workspace_members() if m.startswith("crates/")}


def app_names() -> set[str]:
    return {m.split("/")[-1] for m in workspace_members() if m.startswith("apps/")}


def mentioned(path: Path, names: set[str]) -> tuple[set[str], set[str]]:
    text = path.read_text(encoding="utf8")
    found = {n for n in names if n in text}
    return found, names - found


def main() -> int:
    errors: list[str] = []
    crates = crate_names()
    apps = app_names()

    for doc in [ROOT / "README.md", ROOT / "ARCHITECTURE.md"]:
        found_crates, missing_crates = mentioned(doc, crates)
        found_apps, missing_apps = mentioned(doc, apps)
        for name in sorted(missing_crates | missing_apps):
            errors.append(f"{doc.name}: workspace member '{name}' not mentioned")

    # count consistency: "N crates" / "N apps" claims in README
    readme = (ROOT / "README.md").read_text(encoding="utf8")
    for expected, label in [(crates, "crates"), (apps, "apps")]:
        claim = re.search(rf"(\d+)\s+{label}", readme)
        if claim and int(claim.group(1)) != len(expected):
            errors.append(
                f"README: claims {claim.group(1)} {label}, workspace has {len(expected)}"
            )

    # SDK dependency guard (INV-024 / ADR-0010): plugin-author-facing crates
    # MUST NOT depend on host-internal crates (public domain/protocol crates
    # excepted), so core refactors can never break the plugin API.
    SDK_CRATES = {"launcher-plugin-api"}
    FORBIDDEN = {
        "launcher-core",
        "launcher-ui",
        "launcher-context",
        "launcher-action",
        "launcher-plugin-host",
        "launcher-indexer",
        "launcher-app",
    }
    for sdk in SDK_CRATES:
        cargo = ROOT / "crates" / sdk / "Cargo.toml"
        if not cargo.exists():
            errors.append(f"SDK crate '{sdk}' missing at crates/{sdk}")
            continue
        text = cargo.read_text(encoding="utf8")
        for dep in sorted(FORBIDDEN & set(re.findall(r"(launcher-[\w-]+)\s*=\s*\{", text))):
            errors.append(f"SDK dependency guard: {sdk} MUST NOT depend on {dep} (ADR-0010)")

    # P2.3-A architecture & contract closure (review 85 §29/§116-117):
    # frozen source boundaries for the search/context/domain triangle.
    # launcher-domain is the leaf: no workspace deps at all.
    # launcher-context/launcher-search see ONLY launcher-domain.
    LAYER_GUARDS = {
        "launcher-domain": [],
        "launcher-context": ["launcher-domain"],
        "launcher-search": ["launcher-domain"],
    }
    for layer, allowed in LAYER_GUARDS.items():
        cargo = ROOT / "crates" / layer / "Cargo.toml"
        text = cargo.read_text(encoding="utf8")
        for dep in sorted(set(re.findall(r"(launcher-[\w-]+)\s*=\s*\{", text)) - {layer}):
            if dep not in allowed:
                errors.append(
                    f"P2.3-A layer guard: {layer} MUST NOT depend on {dep} "
                    f"(allowed: {allowed or 'none'})"
                )

    # P0-A dependency-direction guard (review 53 SS29): launcher-runtime is
    # infrastructure — it must never depend on any business/protocol layer.
    rt_cargo = ROOT / "crates" / "launcher-runtime" / "Cargo.toml"
    RT_FORBIDDEN = {
        "launcher-mcp", "launcher-plugin-host", "launcher-workflow",
        "launcher-action", "launcher-core", "launcher-ui", "launcher-domain",
        "launcher-ipc",
    }
    if rt_cargo.exists():
        text = rt_cargo.read_text(encoding="utf8")
        for dep in sorted(RT_FORBIDDEN & set(re.findall(r"(launcher-[\w-]+)\s*=\s*\{", text))):
            errors.append(
                f"runtime dependency guard: launcher-runtime MUST NOT depend on {dep}"
            )
    else:
        errors.append("crates/launcher-runtime missing (P0-A)")

    # P0-D dependency guard (review 55 SS48): launcher-ai's runtime
    # [dependencies] must exclude executors/transports/hosts — test-only
    # dev-dependencies are exempt (the planner suite exercises the MCP E2E).
    ai_cargo = ROOT / "crates" / "launcher-ai" / "Cargo.toml"
    AI_FORBIDDEN = {
        "launcher-mcp", "launcher-plugin-host", "launcher-core",
        "launcher-runtime", "launcher-ui",
    }
    if ai_cargo.exists():
        text = ai_cargo.read_text(encoding="utf8")
        deps_section = text.split("[dependencies]", 1)[1].split("[", 1)[0]
        for dep in sorted(AI_FORBIDDEN & set(re.findall(r"(launcher-[\w-]+)\s*=\s*\{", deps_section))):
            errors.append(
                f"AI dependency guard: launcher-ai MUST NOT depend on {dep} (runtime deps)"
            )
    else:
        errors.append("crates/launcher-ai missing (P0-D)")

    # P1-E UI architecture guard (review 60 section 58): launcher-ui renders
    # ViewModel snapshots only -- it must never depend on executors, runtimes,
    # hosts, or core internals. The host app does all Core-to-DTO projection.
    ui_cargo = ROOT / "crates" / "launcher-ui" / "Cargo.toml"
    UI_FORBIDDEN = {
        "launcher-mcp", "launcher-plugin-host", "launcher-runtime",
        "launcher-ai", "launcher-core", "launcher-workflow",
        "launcher-plugin-api",
    }
    if ui_cargo.exists():
        text = ui_cargo.read_text(encoding="utf8")
        for dep in sorted(UI_FORBIDDEN & set(re.findall(r"(launcher-[\w-]+)\s*=\s*\{", text))):
            errors.append(
                f"UI dependency guard: launcher-ui MUST NOT depend on {dep} (review 60 SS58)"
            )
    else:
        errors.append("crates/launcher-ui missing (P1-E)")

    # P0-B dependency guard (review 48-DD SS47): the HTTP client lives ONLY
    # in launcher-mcp — no other crate may gain an HTTP library dependency.
    HTTP_LIBS = {"ureq", "reqwest", "hyper", "attohttpc"}
    HTTP_ALLOWED = {"crates/launcher-mcp", "crates/launcher-ai"}
    for member in sorted(workspace_members()):
        if member in HTTP_ALLOWED:
            continue
        cargo = ROOT / member / "Cargo.toml"
        if not cargo.exists():
            continue
        text = cargo.read_text(encoding="utf8")
        used = {lib for lib in HTTP_LIBS
                if re.search(rf'^{lib}\s*=\s*{{', text, re.M) or f'{lib} =' in text}
        for lib in sorted(used):
            errors.append(f"HTTP guard: {member} MUST NOT depend on HTTP library {lib}")

    for e in errors:
        print(f"TOPOLOGY ERROR: {e}", file=sys.stderr)
    if errors:
        return 1
    print(f"topology ok: {len(crates)} crates, {len(apps)} apps consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
