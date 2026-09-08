# NativeLauncher uninstaller (P2.3-G, review 91 §20/G13).
# Usage: powershell -ExecutionPolicy Bypass -File uninstall.ps1 [-DeleteUserData]
# Default: PRESERVES user data (config, index, workflows, plugins, favorites).
param([switch]$DeleteUserData)

$ErrorActionPreference = "Stop"
$dest = Join-Path $env:LOCALAPPDATA "Programs\NativeLauncher"
$sm = [Environment]::GetFolderPath("Programs")
$lnk = Join-Path $sm "Native Launcher.lnk"

# stop running instance
$proc = Get-Process launcher-app -ErrorAction SilentlyContinue
if ($proc) {
    Write-Host "Stopping launcher..."
    Stop-Process -Name launcher-app -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

# remove program files
if (Test-Path $dest) {
    Remove-Item $dest -Recurse -Force
    Write-Host "Removed $dest"
}

# remove Start Menu shortcut
if (Test-Path $lnk) {
    Remove-Item $lnk -Force
    Write-Host "Removed shortcut: $lnk"
}

if ($DeleteUserData) {
    $dataDirs = @(
        (Join-Path $env:LOCALAPPDATA "native-launcher"),
        (Join-Path $env:APPDATA "NativeLauncher")
    )
    foreach ($d in $dataDirs) {
        if (Test-Path $d) {
            Remove-Item $d -Recurse -Force
            Write-Host "Removed user data: $d"
        }
    }
    Write-Host "All user data deleted."
} else {
    Write-Host ""
    Write-Host "User data PRESERVED (config, favorites, index, workflows, plugins):"
    Write-Host "  %LOCALAPPDATA%\native-launcher"
    Write-Host "  %APPDATA%\NativeLauncher"
    Write-Host "Delete these folders explicitly if you want a full wipe."
}

Write-Host "Uninstall complete."
