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
    param(
        [string]$Name,
        [scriptblock]$Command
    )

    Write-Host ""
    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

function Assert-NoMatches {
    param(
        [string]$Name,
        [string]$Pattern,
        [string[]]$Paths,
        [string[]]$RgArgs = @()
    )

    Write-Host ""
    Write-Host "==> $Name"
    $rgOutput = & rg -n @RgArgs $Pattern @Paths
    if ($LASTEXITCODE -eq 0) {
        $rgOutput
        throw "$Name failed."
    }
    if ($LASTEXITCODE -ne 1) {
        throw "$Name could not complete."
    }
}

Require-Command cargo
Require-Command git
Require-Command npm
Require-Command rg

Run-Step "cargo fmt --check" {
    cargo fmt --check
}

Run-Step "cargo clippy" {
    cargo clippy -p Chataigne2 --all-targets --no-deps -- -D warnings
}

Run-Step "cargo test" {
    cargo test --workspace
}

Run-Step "cargo check" {
    cargo check --workspace
}

Run-Step "Phase 2 architecture contracts" {
    python tools/migration/check_phase2_contracts.py
}

Run-Step "Phase 3 foundation contracts" {
    python tools/migration/check_phase3_contracts.py
}

if (-not $SkipUiInstall) {
    Run-Step "npm ci" {
        npm ci
    }
}

Push-Location apps/chataigne/ui
try {
    Run-Step "npm run check" {
        npm run check
    }

    Run-Step "npm run build" {
        npm run build
    }

    Run-Step "generated protocol freshness" {
        npm run codegen:golden-ui-protocol
    }
}
finally {
    Pop-Location
}

Run-Step "generated protocol diff" {
    git diff --exit-code -- packages/golden-ui/generated/rust_protocol
}

Assert-NoMatches `
    -Name "no hand-written #[path] imports in app/build source" `
    -Pattern "#\[\s*path\s*=" `
    -Paths @("apps/chataigne/src", "apps/chataigne/build.rs")

Assert-NoMatches `
    -Name "no legacy Svelte on: event syntax" `
    -Pattern "<[^>]*\son:[A-Za-z]" `
    -Paths @("apps/chataigne/ui/src", "packages") `
    -RgArgs @("--glob", "*.svelte")

Assert-NoMatches `
    -Name "no direct Tauri globals outside host bridge" `
    -Pattern "__TAURI__|__TAURI_INTERNALS__|@tauri-apps/api" `
    -Paths @("apps/chataigne/ui/src", "packages") `
    -RgArgs @(
        "--glob", "!**/app.d.ts",
        "--glob", "!**/host/desktop.ts"
    )
