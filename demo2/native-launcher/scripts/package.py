r"""Launcher 1.0 packaging (MUST-3, review 66).

Builds a release exe and assembles a distributable zip:

  dist/NativeLauncher-<version>-win64.zip
    ├── launcher-app.exe
    ├── install.ps1        (copy to %LOCALAPPDATA%\Programs, Start Menu shortcut)
    ├── workflows/demo.json
    └── README.txt

Upgrade path: all user state lives in %LOCALAPPDATA%\native-launcher
(index.db, workflows/) and %APPDATA%\NativeLauncher (config.toml, logs,
plugins) — reinstalling over an old version preserves both. Run:

    python scripts/package.py [--skip-build]
"""

import argparse
import json
import os
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

INSTALL_PS1 = r"""# NativeLauncher installer (MUST-3).
# Usage: powershell -ExecutionPolicy Bypass -File install.ps1 [-NoShortcut]
# Upgrade-safe: never touches %LOCALAPPDATA%\native-launcher (index,
# workflows) or %APPDATA%\NativeLauncher (config, plugins, logs).
param([switch]$NoShortcut)

$ErrorActionPreference = "Stop"
$src = Split-Path -Parent $MyInvocation.MyCommand.Path
$dest = Join-Path $env:LOCALAPPDATA "Programs\NativeLauncher"

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item (Join-Path $src "launcher-app.exe") $dest -Force

$workflowDir = Join-Path $env:LOCALAPPDATA "native-launcher\workflows"
New-Item -ItemType Directory -Force -Path $workflowDir | Out-Null
if (-not (Test-Path (Join-Path $workflowDir "demo.json"))) {
    Copy-Item (Join-Path $src "workflows\demo.json") $workflowDir -Force
}

if (-not $NoShortcut) {
    $sm = [Environment]::GetFolderPath("Programs")
    $lnk = Join-Path $sm "Native Launcher.lnk"
    $ws = New-Object -ComObject WScript.Shell
    $sc = $ws.CreateShortcut($lnk)
    $sc.TargetPath = Join-Path $dest "launcher-app.exe"
    $sc.WorkingDirectory = $dest
    $sc.Description = "Native Launcher (keyboard-first Windows launcher)"
    $sc.Save()
    Write-Host "Start Menu shortcut created: $lnk"
}

Write-Host "Installed to $dest"
Write-Host "Launch once, then set your hotkey in %APPDATA%\NativeLauncher\config.toml"
Write-Host ""
Write-Host "Uninstall: remove '$dest' + Start Menu shortcut."
Write-Host "User data (%LOCALAPPDATA%\native-launcher, %APPDATA%\NativeLauncher) is PRESERVED."
Write-Host "Delete those two folders explicitly if you want a full wipe."
"""

README = """Native Launcher 1.0
===================

Run install.ps1 to install (upgrade-safe: config, index, workflows and
plugins are preserved). Launch the exe, press the global hotkey
(default Ctrl+Space) and type.

Settings: search "Open Settings" inside the launcher, edit the TOML file,
save, then recall the launcher - changes apply on the next popup
(hotkey / accent / scale / result_limit are hot-reloaded).

Workflows: drop JSON workflow definitions into
%LOCALAPPDATA%\\native-launcher\\workflows\\ (see workflows/demo.json).
They appear as searchable commands ("Run workflow").
"""

DEMO_WORKFLOW = {
    "id": "wf.demo",
    "version": 1,
    "name": "Demo: Copy Greeting",
    "steps": [
        {
            "step_id": "s1",
            "action": {
                "Inline": {
                    "id": "s1",
                    "title": "Copy greeting",
                    "type": "system.copy_to_clipboard",
                    "input": {"text": "Hello from Native Launcher!"},
                }
            },
            "input": {"text": "Hello from Native Launcher!"},
        }
    ],
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-build", action="store_true")
    args = ap.parse_args()

    if not args.skip_build:
        print("building release...")
        subprocess.run(["cargo", "build", "--release", "-p", "launcher-app"],
                       cwd=ROOT, check=True)

    exe = ROOT / "target" / "release" / "launcher-app.exe"
    if not exe.exists():
        print("FATAL: release exe not found; run without --skip-build", file=sys.stderr)
        return 1

    version = "1.0.0"
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    in_pkg = False
    for line in cargo.splitlines():
        if line.strip() == "[workspace.package]":
            in_pkg = True
        elif line.strip().startswith("["):
            in_pkg = False
        elif in_pkg and line.strip().startswith("version"):
            v = line.split("=", 1)[1].strip().strip('"')
            if v and v != "true":
                version = v
            break

    dist = ROOT / "artifacts" / "dist"
    dist.mkdir(parents=True, exist_ok=True)
    stage = dist / "stage"
    if stage.exists():
        import shutil
        shutil.rmtree(stage)
    (stage / "workflows").mkdir(parents=True)

    import shutil
    shutil.copy2(exe, stage / "launcher-app.exe")
    (stage / "install.ps1").write_text(INSTALL_PS1, encoding="utf-8")
    uninstall_src = ROOT / "scripts" / "uninstall.ps1"
    if uninstall_src.exists():
        shutil.copy2(uninstall_src, stage / "uninstall.ps1")
    (stage / "README.txt").write_text(README, encoding="utf-8")
    (stage / "workflows" / "demo.json").write_text(
        json.dumps(DEMO_WORKFLOW, indent=2), encoding="utf-8")

    zip_path = dist / f"NativeLauncher-{version}-win64.zip"
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
        for f in sorted(stage.rglob("*")):
            if f.is_file():
                z.write(f, f.relative_to(stage))
    import shutil
    shutil.rmtree(stage)
    print(f"packaged: {zip_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
