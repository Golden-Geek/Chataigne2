param(
    [switch]$SkipInstall
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT -and (Get-Command rustup -ErrorAction SilentlyContinue)) {
    $env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-msvc"
}

if (-not $SkipInstall) {
    npm ci
}

cargo check --workspace --all-targets --all-features
npm run check

Write-Host "Golden monorepo dependencies and workspace checks are ready."
