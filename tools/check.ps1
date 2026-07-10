param(
    [switch]$SkipUiInstall
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$env:GC_SKIP_UI_BUILD = "1"

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

Run-Step "root cargo fmt --check" {
    cargo fmt --all --check
}

Run-Step "golden_core cargo fmt --check" {
    cargo fmt --manifest-path submodules/golden_core/Cargo.toml --all --check
}

Run-Step "golden_alchemist_core cargo fmt --check" {
    cargo fmt --manifest-path submodules/golden_alchemist_core/Cargo.toml --all --check
}

Run-Step "root workspace cargo clippy" {
    python tools/clippy_gate.py --baseline tools/clippy-baseline-root.json
}

Run-Step "golden_core workspace cargo clippy" {
    python tools/clippy_gate.py --manifest-path submodules/golden_core/Cargo.toml --baseline tools/clippy-baseline-golden-core.json
}

Run-Step "golden_alchemist_core workspace cargo clippy" {
    python tools/clippy_gate.py --manifest-path submodules/golden_alchemist_core/Cargo.toml --baseline tools/clippy-baseline-golden-alchemist-core.json
}

Run-Step "root workspace cargo test" {
    cargo test --workspace -- --test-threads=1
}

Run-Step "golden_core workspace cargo test" {
    cargo test --manifest-path submodules/golden_core/Cargo.toml --workspace -- --test-threads=1
}

Run-Step "golden_alchemist_core workspace cargo test" {
    cargo test --manifest-path submodules/golden_alchemist_core/Cargo.toml --workspace --all-features
}

Run-Step "root workspace cargo check" {
    cargo check --workspace --all-targets --all-features
}

Run-Step "golden_core workspace cargo check" {
    cargo check --manifest-path submodules/golden_core/Cargo.toml --workspace --all-targets --all-features
}

Run-Step "golden_alchemist_core workspace cargo check" {
    cargo check --manifest-path submodules/golden_alchemist_core/Cargo.toml --workspace --all-targets --all-features
}

Push-Location src-ui
try {
    if (-not $SkipUiInstall) {
        Run-Step "npm ci" {
            npm ci
        }
    }

    Run-Step "npm run lint" {
        npm run lint
    }

    Run-Step "npm run check" {
        npm run check
    }

    Run-Step "npm run build" {
        npm run build
    }

    Run-Step "generated protocol freshness" {
        npm run codegen:golden-ui-protocol
        npm run codegen:state-machine-protocol
    }

    Run-Step "production npm audit" {
        npm audit --omit=dev --audit-level=high
    }
}
finally {
    Pop-Location
}

Run-Step "generated protocol diff" {
    git diff --exit-code -- src-ui/src/lib/golden_ui/generated/rust_protocol src-ui/src/lib/state_machine/generated
}

Run-Step "root cargo audit" {
    cargo audit
}

Run-Step "golden_core cargo audit" {
    cargo audit --file submodules/golden_core/Cargo.lock
}

Run-Step "golden_alchemist_core cargo audit" {
    cargo audit --file submodules/golden_alchemist_core/Cargo.lock
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
