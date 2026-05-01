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
}

function Assert-NoMatches {
    param(
        [string]$Name,
        [string]$Pattern,
        [string[]]$Paths,
        [string[]]$Args = @()
    )

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
Require-Command git
Require-Command npm
Require-Command rg

Run-Step "cargo fmt --check" {
    cargo fmt --check
}

Run-Step "cargo clippy" {
    cargo clippy --all-targets -- -D warnings
}

Run-Step "cargo test" {
    cargo test
}

Run-Step "cargo check" {
    cargo check
}

Push-Location src-ui
try {
    if (-not $SkipUiInstall) {
        Run-Step "npm ci" {
            npm ci
        }
    }

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
    git diff --exit-code -- src-ui/src/lib/golden_ui/generated/rust_protocol
}

Assert-NoMatches `
    -Name "no hand-written #[path] imports in app/build source" `
    -Pattern "#\[\s*path\s*=" `
    -Paths @("src", "build.rs")

Assert-NoMatches `
    -Name "no legacy Svelte on: event syntax" `
    -Pattern "<[^>]*\son:[A-Za-z]" `
    -Paths @("src-ui/src") `
    -Args @("--glob", "*.svelte")

Assert-NoMatches `
    -Name "no direct Tauri globals outside host bridge" `
    -Pattern "__TAURI__|__TAURI_INTERNALS__|@tauri-apps/api" `
    -Paths @("src-ui/src") `
    -Args @(
        "--glob", "!**/app.d.ts",
        "--glob", "!**/host/desktop.ts"
    )
