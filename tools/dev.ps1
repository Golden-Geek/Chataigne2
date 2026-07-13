[CmdletBinding()]
param(
    [switch] $SetupOnly,
    [switch] $SkipUiInstall,
    [switch] $SkipWindowsBuildTools,

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

function Install-WingetPackage {
    param(
        [string] $Id,
        [string] $DisplayName,
        [string] $Override = ""
    )

    if (-not (Get-CommandExists "winget")) {
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

function Ensure-Rustup {
    Refresh-Path
    if (-not (Get-CommandExists "rustup")) {
        Install-WingetPackage "Rustlang.Rustup" "rustup"
    }
    Refresh-Path
    if (-not (Get-CommandExists "rustup")) {
        throw "rustup was installed but is still not on PATH. Restart PowerShell, then rerun tools/dev.ps1."
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

    Install-WingetPackage `
        "Microsoft.VisualStudio.2022.BuildTools" `
        "Visual Studio 2022 Build Tools" `
        "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    if (-not (Test-VcBuildToolsInstalled)) {
        throw "Visual Studio C++ Build Tools could not be verified. Restart Windows if requested, then rerun tools/dev.ps1."
    }
}

function Activate-CanonicalToolchain {
    Write-Step "Canonical Rust, Node, npm, and Python contract"
    Ensure-Rustup
    & (Join-Path $Root "tools\bootstrap\install-rust-toolchain.ps1")
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $nodeDirectory = & (Join-Path $Root "tools\bootstrap\install-node.ps1")
    if ([string]::IsNullOrWhiteSpace($nodeDirectory)) {
        throw "Pinned Node installer did not return its local bin directory."
    }
    $env:PATH = "$nodeDirectory$([System.IO.Path]::PathSeparator)$env:PATH"
    & (Join-Path $Root "tools\bootstrap\verify-toolchain.ps1") -CheckInstalled
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
Activate-CanonicalToolchain
Ensure-UiDependencies

if (-not $SetupOnly) {
    Write-Step "Run Chataigne2"
    Invoke-External "cargo" (@("run") + $CargoArgs)
}
