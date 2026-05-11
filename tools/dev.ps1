param(
    [switch] $SetupOnly,
    [switch] $SkipUiInstall,
    [switch] $SkipWindowsBuildTools,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $CargoArgs
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($env:OS -ne "Windows_NT") {
    throw "tools/dev.ps1 is the Windows bootstrap. Use bash tools/dev.sh on macOS/Linux."
}

$RequiredNodeRange = "Node.js 20.19+ or 22.12+"

function Write-Step {
    param([string] $Name)

    Write-Host ""
    Write-Host "==> $Name"
}

function Command-Exists {
    param([string] $Name)

    return [bool] (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-NpmCommand {
    if (Command-Exists "npm.cmd") {
        return "npm.cmd"
    }

    return "npm"
}

function Refresh-Path {
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $cargoPath = Join-Path $HOME ".cargo\bin"
    $env:Path = @($machinePath, $userPath, $cargoPath, $env:Path) -join ";"
}

function Invoke-External {
    param(
        [string] $FilePath,
        [string[]] $Arguments = @()
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

function Install-WingetPackage {
    param(
        [string] $Id,
        [string] $DisplayName,
        [string] $Override = ""
    )

    if (-not (Command-Exists winget)) {
        throw "Cannot install $DisplayName automatically because winget was not found. Install it manually, restart PowerShell, then rerun tools/dev.ps1."
    }

    $arguments = @(
        "install",
        "--id", $Id,
        "--exact",
        "--source", "winget",
        "--accept-package-agreements",
        "--accept-source-agreements"
    )

    if (-not [string]::IsNullOrWhiteSpace($Override)) {
        $arguments += @("--override", $Override)
    }

    Invoke-External "winget" $arguments
    Refresh-Path
}

function Ensure-GitSubmodules {
    Write-Step "Git submodules"

    if (-not (Command-Exists git)) {
        throw "git was not found on PATH. Install Git, then rerun tools/dev.ps1."
    }

    Invoke-External "git" @("submodule", "update", "--init", "--recursive")
}

function Ensure-GitHooks {
    Write-Step "Git hooks"

    $hooksPath = git config --local --get core.hooksPath 2>$null
    if ($LASTEXITCODE -ne 0) {
        $hooksPath = $null
    }

    if ($hooksPath -eq ".githooks") {
        Write-Host "core.hooksPath is already set to .githooks."
        return
    }

    if (-not [string]::IsNullOrWhiteSpace($hooksPath)) {
        Write-Host "Updating core.hooksPath from '$hooksPath' to .githooks."
    }

    Invoke-External "git" @("config", "--local", "core.hooksPath", ".githooks")
    Write-Host "Configured core.hooksPath -> .githooks"
}

function Ensure-Rust {
    Write-Step "Rust toolchain"
    Refresh-Path

    if (-not (Command-Exists rustup)) {
        Install-WingetPackage "Rustlang.Rustup" "rustup"
    }

    Refresh-Path

    if (-not (Command-Exists rustup)) {
        throw "rustup was installed but is still not on PATH. Restart PowerShell, then rerun tools/dev.ps1."
    }

    Invoke-External "rustup" @("toolchain", "install", "stable-msvc")
    Invoke-External "rustup" @("default", "stable-msvc")

    Refresh-Path

    if (-not (Command-Exists cargo)) {
        throw "cargo was not found after installing rustup. Restart PowerShell, then rerun tools/dev.ps1."
    }

    cargo --version
}

function Test-VcBuildToolsInstalled {
    if (Command-Exists cl.exe) {
        return $true
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        return $false
    }

    $installationPath = & $vswhere `
        -latest `
        -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath `
        2>$null

    return ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($installationPath))
}

function Ensure-WindowsBuildTools {
    Write-Step "Windows desktop build tools"

    if ($SkipWindowsBuildTools) {
        Write-Host "Skipping Visual Studio C++ Build Tools check."
        return
    }

    if (Test-VcBuildToolsInstalled) {
        Write-Host "Visual Studio C++ Build Tools found."
        return
    }

    Install-WingetPackage `
        "Microsoft.VisualStudio.2022.BuildTools" `
        "Visual Studio 2022 Build Tools" `
        "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

    if (-not (Test-VcBuildToolsInstalled)) {
        throw "Visual Studio C++ Build Tools could not be verified. Restart Windows if the installer requested it, then rerun tools/dev.ps1."
    }
}

function Test-NodeVersionSupported {
    if (-not (Command-Exists node)) {
        return $false
    }

    $version = (& node -p "process.versions.node" 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($version)) {
        return $false
    }

    $parts = $version.Trim().Split(".")
    if ($parts.Count -lt 2) {
        return $false
    }

    $major = 0
    $minor = 0
    if (-not [int]::TryParse($parts[0], [ref] $major)) {
        return $false
    }
    if (-not [int]::TryParse($parts[1], [ref] $minor)) {
        return $false
    }

    return (($major -eq 20 -and $minor -ge 19) -or ($major -eq 22 -and $minor -ge 12) -or ($major -ge 23))
}

function Ensure-Node {
    Write-Step "Node.js and npm"
    Refresh-Path

    if (-not (Test-NodeVersionSupported) -or -not (Command-Exists (Get-NpmCommand))) {
        Install-WingetPackage "OpenJS.NodeJS.LTS" "Node.js LTS"
        Refresh-Path
    }

    if (-not (Test-NodeVersionSupported)) {
        $found = if (Command-Exists node) { (& node --version) } else { "not found" }
        throw "$RequiredNodeRange is required by the Svelte/Vite frontend. Found: $found. Install Node.js LTS or fix PATH, then rerun tools/dev.ps1."
    }

    $npm = Get-NpmCommand
    if (-not (Command-Exists $npm)) {
        throw "npm was not found after installing Node.js. Restart PowerShell, then rerun tools/dev.ps1."
    }

    node --version
    Invoke-External $npm @("--version")
}

function Test-UiInstallNeeded {
    $packageLock = Join-Path $Root "src-ui\package-lock.json"
    $nodeModulesLock = Join-Path $Root "src-ui\node_modules\.package-lock.json"

    if (-not (Test-Path $nodeModulesLock)) {
        return $true
    }

    return ((Get-Item $nodeModulesLock).LastWriteTimeUtc -lt (Get-Item $packageLock).LastWriteTimeUtc)
}

function Ensure-UiDependencies {
    if ($SkipUiInstall) {
        Write-Host ""
        Write-Host "==> Svelte dependencies"
        Write-Host "Skipping npm ci."
        return
    }

    Write-Step "Svelte dependencies"
    if (-not (Test-UiInstallNeeded)) {
        Write-Host "src-ui/node_modules is current."
        return
    }

    $npm = Get-NpmCommand
    Push-Location "src-ui"
    try {
        Invoke-External $npm @("ci")
    }
    finally {
        Pop-Location
    }
}

function Run-App {
    Write-Step "Run Chataigne2"
    Invoke-External "cargo" (@("run") + $CargoArgs)
}

Ensure-GitSubmodules
Ensure-GitHooks
Ensure-Rust
Ensure-WindowsBuildTools
Ensure-Node
Ensure-UiDependencies

if (-not $SetupOnly) {
    Run-App
}
