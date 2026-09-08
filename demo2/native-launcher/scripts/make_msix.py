r"""MSIX packaging (P2.3-G RC1 item, unsigned).

Stages the packaged app layout (same content as package.py's zip) plus an
AppxManifest and produces artifacts/dist/NativeLauncher-<version>-win64.msix
using the Windows SDK's MakeAppx.exe.

The .msix is UNSIGNED — installing it requires signing first:

    signtool sign /fd SHA256 /a /f <cert>.pfx /p <pass> <file>.msix

Signing is a GA-external step (no signing certificate exists in this repo);
this script only guarantees the packaging artifact is reproducible. If
MakeAppx.exe is not found, the script prints the SDK search paths tried and
exits non-zero so CI notices.

Run:
    python scripts/make_msix.py
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "artifacts" / "dist"

# Minimal set of MSIX-required square logo assets. Windows validates presence
# and manifest references, not artistic content — a 1x1 scaled placeholder PNG
# is generated per size; visual assets are a v0.2 polish item.
LOGO_SIZES = [44, 150, 310]

APPX_MANIFEST = """<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities">
  <Identity Name="Aura.NativeLauncher"
            Version="{version}.0"
            Publisher="CN=NativeLauncher"
            ProcessorArchitecture="x64"/>
  <Properties>
    <DisplayName>Native Launcher</DisplayName>
    <PublisherDisplayName>Native Launcher</PublisherDisplayName>
    <Logo>assets\\Logo{logo_default}.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26200.0"/>
  </Dependencies>
  <Resources>
    <Resource Language="en-us"/>
  </Resources>
  <Applications>
    <Application Id="LauncherApp"
                Executable="launcher-app.exe"
                EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="Native Launcher"
        Description="Keyboard-first Windows launcher"
        BackgroundColor="#1E1E28"
        Square150x150Logo="assets\\Logo150.png"
        Square44x44Logo="assets\\Logo44.png">
        <uap:DefaultTile Square310x310Logo="assets\\Logo310.png" Wide310x150Logo="assets\\Logo310.png"/>
      </uap:VisualElements>
    </Application>
  </Applications>
  <Capabilities>
    <!-- Win32 exe (FullTrustApplication entry point) requires the
         restricted runFullTrust capability; allowed for signed packages. -->
    <rescap:Capability Name="runFullTrust"/>
  </Capabilities>
</Package>
"""


def workspace_version() -> str:
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    in_pkg = False
    for line in cargo.splitlines():
        if line.strip() == "[workspace.package]":
            in_pkg = True
        elif line.strip().startswith("["):
            in_pkg = False
        elif in_pkg and line.strip().startswith("version"):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("FATAL: workspace version not found in Cargo.toml")


def make_placeholder_png(size: int) -> bytes:
    """Minimal valid RGB PNG (solid dark tile), no external deps."""
    import struct
    import zlib

    raw = b"".join(b"\x00" + b"\x1e\x1e\x28" * size for _ in range(size))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def find_makeappx() -> Path | None:
    kits = sorted(
        Path(r"C:\Program Files (x86)\Windows Kits\10\bin").glob(
            "*/x64/makeappx.exe"
        ),
        reverse=True,
    )
    return kits[0] if kits else None


def main() -> int:
    exe = ROOT / "target" / "release" / "launcher-app.exe"
    if not exe.exists():
        print("FATAL: run `cargo build --release -p launcher-app` (or package.py) first",
              file=sys.stderr)
        return 1

    makeappx = find_makeappx()
    if makeappx is None:
        print("FATAL: MakeAppx.exe not found under Windows Kits\\10\\bin; "
              "install the Windows SDK", file=sys.stderr)
        return 1

    version = workspace_version()
    import shutil

    stage = DIST / "msix-stage"
    if stage.exists():
        shutil.rmtree(stage)
    (stage / "assets").mkdir(parents=True)
    shutil.copy2(exe, stage / "launcher-app.exe")
    for size in LOGO_SIZES:
        (stage / "assets" / f"Logo{size}.png").write_bytes(make_placeholder_png(size))
    (stage / "AppxManifest.xml").write_text(
        APPX_MANIFEST.format(version=version, logo_default=150), encoding="utf-8"
    )

    msix = DIST / f"NativeLauncher-{version}-win64.msix"
    if msix.exists():
        msix.unlink()
    result = subprocess.run(
        [str(makeappx), "pack", "/o", "/d", str(stage), "/p", str(msix)],
        capture_output=True,
        text=True,
    )
    shutil.rmtree(stage)
    if result.returncode != 0:
        print(result.stdout, file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        print("FATAL: MakeAppx failed", file=sys.stderr)
        return 1

    print(f"packaged (UNSIGNED — sign with signtool before install): {msix}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
