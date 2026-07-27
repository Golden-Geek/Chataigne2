[CmdletBinding()]
param(
    [switch] $SetupOnly,
    [switch] $SkipUiInstall,
    [switch] $SkipWindowsBuildTools,
    [switch] $Asio,
    [switch] $FullAudioHosts,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $CargoArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Set-Location $Root

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows
    )) {
    throw "tools/dev.ps1 is the Windows bootstrap. Use bash tools/dev.sh on macOS/Linux."
}

function Write-Step {
    param([string] $Name)

    Write-Host ""
    Write-Host "==> $Name"
}

function Get-CommandExists {
    param([string] $Name)

    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Refresh-Path {
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $cargoPath = Join-Path $HOME ".cargo\bin"
    $env:PATH = @($machinePath, $userPath, $cargoPath, $env:PATH) -join ";"
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

function Ensure-Rustup {
    Refresh-Path
    if (-not (Get-CommandExists "rustup")) {
        throw "rustup is a system prerequisite. Install it and the pinned toolchain from docs/operations/workspace-hygiene.md, then rerun tools/dev.ps1."
    }
}

function Test-VcBuildToolsInstalled {
    if (Get-CommandExists "cl.exe") {
        return $true
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
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

    throw "Visual Studio C++ Build Tools with the VCTools workload are a system prerequisite. Install them, then rerun tools/dev.ps1."
}

function Ensure-AudioBuildTools {
    Write-Step "Audio host build tools"
    if (-not $Asio -and -not $FullAudioHosts) {
        Write-Host "WASAPI is ready. Use -Asio or -FullAudioHosts for optional audio hosts."
        return
    }
    & (Join-Path $Root "tools\asio.ps1") -SetupOnly
    Write-Host "ASIO and dynamically loaded JACK are ready to compile."
}

function Activate-CanonicalToolchain {
    Write-Step "Canonical Rust, Node, npm, and Python contract"
    Ensure-Rustup
    $env:CARGO_TARGET_DIR = Join-Path $Root "target"
    & (Join-Path $Root "tools\bootstrap\verify-toolchain.ps1") -CheckInstalled
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    & (Join-Path $Root "tools\workspace-hygiene.ps1") -Action Audit
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

function Test-UiInstallNeeded {
    $packageLock = Join-Path $Root "package-lock.json"
    $installedLock = Join-Path $Root "node_modules\.package-lock.json"
    if (-not (Test-Path -LiteralPath $installedLock -PathType Leaf)) {
        return $true
    }
    return ((Get-Item -LiteralPath $installedLock).LastWriteTimeUtc -lt
        (Get-Item -LiteralPath $packageLock).LastWriteTimeUtc)
}

function Ensure-UiDependencies {
    Write-Step "Root JavaScript workspace"
    if ($SkipUiInstall) {
        Write-Host "Skipping npm ci."
        return
    }
    if (-not (Test-UiInstallNeeded)) {
        Write-Host "node_modules is current."
        return
    }
    Invoke-External "npm.cmd" @("ci")
}

Ensure-WindowsBuildTools
Ensure-AudioBuildTools
Activate-CanonicalToolchain
Ensure-UiDependencies

if (-not $SetupOnly) {
    Write-Step "Run Chataigne2"
    $audioFeatureArguments = @()
    if ($FullAudioHosts) {
        $audioFeatureArguments = @(
            "--features",
            "golden_audio/asio,golden_audio/jack,golden_audio/realtime"
        )
    } elseif ($Asio) {
        $audioFeatureArguments = @("--features", "golden_audio/asio")
    }
    Invoke-External "cargo" (@("run") + $audioFeatureArguments + $CargoArgs)
}
