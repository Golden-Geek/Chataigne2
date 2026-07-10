param(
    [switch]$SkipUiInstall
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Require-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found on PATH."
    }
}

function Run-Step {
    param([string]$Name, [scriptblock]$Command)
    Write-Host ""
    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

function Assert-NoMatches {
    param([string]$Name, [string]$Pattern, [string[]]$Paths, [string[]]$Args = @())
    Write-Host ""
    Write-Host "==> $Name"
    $rgOutput = & rg -n @Args $Pattern @Paths
    if ($LASTEXITCODE -eq 0) {
        $rgOutput
        throw "$Name failed."
    }
    if ($LASTEXITCODE -ne 1) {
        throw "$Name could not complete."
    }
}

Require-Command cargo
Require-Command npm
Require-Command python
Require-Command rg

if ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT -and (Get-Command rustup -ErrorAction SilentlyContinue)) {
    $env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-msvc"
}

Run-Step "root cargo fmt --check" { cargo fmt --all --check }

$LegacyGoldenCore = Join-Path $Root "legacy/repositories/golden_core/Cargo.toml"
if (Test-Path $LegacyGoldenCore) {
    Run-Step "legacy golden_core cargo fmt --check" {
        cargo fmt --manifest-path $LegacyGoldenCore --all --check
    }
}

Run-Step "workspace cargo clippy" {
    cargo clippy --workspace --all-targets --all-features -- -D warnings
}
Run-Step "workspace cargo test" { cargo test --workspace --all-features }
Run-Step "workspace cargo check" { cargo check --workspace --all-targets --all-features }

if (-not $SkipUiInstall) {
    Run-Step "npm ci" { npm ci }
}
Run-Step "npm test" { npm test }
Run-Step "npm check" { npm run check }
Run-Step "npm build" { npm run build }
Run-Step "production npm audit" { npm audit --omit=dev --audit-level=high }
Run-Step "workspace architecture" { python tools/check_workspace_architecture.py }
Run-Step "architecture contract" { python tools/check_architecture_contract.py }

Assert-NoMatches `
    -Name "no hand-written #[path] imports in app/build source" `
    -Pattern "#\[\s*path\s*=" `
    -Paths @("apps", "crates") `
    -Args @("--glob", "*.rs")

Assert-NoMatches `
    -Name "no legacy Svelte on: event syntax" `
    -Pattern "<[^>]*\son:[A-Za-z]" `
    -Paths @("apps", "packages") `
    -Args @("--glob", "*.svelte")
