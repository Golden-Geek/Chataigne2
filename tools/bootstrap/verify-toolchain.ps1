[CmdletBinding()]
param(
    [switch] $CheckInstalled,
    [switch] $CheckQualificationTools
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$manifestPath = Join-Path $PSScriptRoot "toolchain.json"
$manifest = [System.IO.File]::ReadAllText($manifestPath) | ConvertFrom-Json

if ([int]$manifest.schema_version -ne 1) {
    throw "Unsupported toolchain manifest schema '$($manifest.schema_version)'."
}

$requiredValues = @(
    $manifest.rust.channel,
    $manifest.rust.cargo_version,
    $manifest.rust.profile,
    $manifest.node.version,
    $manifest.node.npm_version,
    $manifest.python.version,
    $manifest.qualification_tools.cargo_deny,
    $manifest.qualification_tools.cargo_machete
)
if (@($requiredValues | Where-Object { [string]::IsNullOrWhiteSpace([string]$_) }).Count -gt 0) {
    throw "tools/bootstrap/toolchain.json contains an empty required version."
}
$platformKeys = @("windows_x64", "windows_arm64", "macos_x64", "macos_arm64", "linux_x64", "linux_arm64")
foreach ($platformKey in $platformKeys) {
    $hostTriple = [string]$manifest.rust.hosts.$platformKey
    if ([string]::IsNullOrWhiteSpace($hostTriple)) {
        throw "Missing Rust host triple '$platformKey'."
    }
}
if ([string]::IsNullOrWhiteSpace([string]$manifest.rust.hosts.linux_armv7)) {
    throw "Missing Rust host triple 'linux_armv7'."
}

$rustVersionPath = Join-Path $repositoryRoot ([string]$manifest.consumers.rust_version)
$nodeVersionPath = Join-Path $repositoryRoot ([string]$manifest.consumers.node_version)
if (-not (Test-Path -LiteralPath $rustVersionPath -PathType Leaf)) {
    throw "Missing generated Rust consumer '$rustVersionPath'."
}
if (-not (Test-Path -LiteralPath $nodeVersionPath -PathType Leaf)) {
    throw "Missing generated Node consumer '$nodeVersionPath'."
}

$rustVersion = [System.IO.File]::ReadAllText($rustVersionPath).Trim()
if ($rustVersion -ne [string]$manifest.rust.channel) {
    throw "tools/bootstrap/rust-version does not match tools/bootstrap/toolchain.json."
}

$nodeVersion = [System.IO.File]::ReadAllText($nodeVersionPath).Trim()
if ($nodeVersion -ne [string]$manifest.node.version) {
    throw ".nvmrc does not match tools/bootstrap/toolchain.json."
}

function Get-CommandVersion {
    param(
        [string] $Executable,
        [string[]] $Arguments
    )

    $command = Get-Command $Executable -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $command) {
        throw "Required system tool '$Executable' was not found on PATH. Install the pinned version documented in docs/operations/workspace-hygiene.md; repository-local toolchains are not supported."
    }
    $output = @(& $command.Source @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw "'$Executable $($Arguments -join ' ')' exited with code $LASTEXITCODE."
    }
    return $output.Trim()
}

if ($CheckInstalled) {
    $rustc = Get-CommandVersion -Executable "rustc" -Arguments @("--version")
    $cargo = Get-CommandVersion -Executable "cargo" -Arguments @("--version")
    $node = Get-CommandVersion -Executable "node" -Arguments @("--version")
    $npmExecutable = if (Get-Command "npm.cmd" -ErrorAction SilentlyContinue) { "npm.cmd" } else { "npm" }
    $npm = Get-CommandVersion -Executable $npmExecutable -Arguments @("--version")
    $python = Get-CommandVersion -Executable "python" -Arguments @("--version")

    if ($rustc -notmatch ("^rustc {0}(?:\s|$)" -f [regex]::Escape([string]$manifest.rust.channel))) {
        throw "Installed rustc does not match pinned $($manifest.rust.channel): $rustc"
    }
    if ($cargo -notmatch ("^cargo {0}(?:\s|$)" -f [regex]::Escape([string]$manifest.rust.cargo_version))) {
        throw "Installed Cargo does not match pinned $($manifest.rust.cargo_version): $cargo"
    }
    if ($node -ne ("v{0}" -f $manifest.node.version)) {
        throw "Installed Node does not match pinned $($manifest.node.version): $node"
    }
    if ($npm -ne [string]$manifest.node.npm_version) {
        throw "Installed npm does not match pinned $($manifest.node.npm_version): $npm"
    }
    if ($python -notmatch ("^Python {0}(?:\.\d+)?(?:\s|$)" -f [regex]::Escape([string]$manifest.python.version))) {
        throw "Installed Python does not match supported $($manifest.python.version).x: $python"
    }
    $verboseRustc = Get-CommandVersion -Executable "rustc" -Arguments @("-vV")
    $hostMatch = [regex]::Match($verboseRustc, '(?m)^host:\s*(\S+)\s*$')
    if (-not $hostMatch.Success) {
        throw "rustc -vV did not report a host triple."
    }
    $runningOnWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows
    )
    if ($runningOnWindows -and $hostMatch.Groups[1].Value -notmatch '-pc-windows-msvc$') {
        throw "Windows qualification requires an MSVC Rust host, got '$($hostMatch.Groups[1].Value)'."
    }

}

if ($CheckQualificationTools) {
    $cargoDeny = Get-CommandVersion -Executable "cargo-deny" -Arguments @("--version")
    $cargoMachete = Get-CommandVersion -Executable "cargo-machete" -Arguments @("--version")
    if ($cargoDeny -notmatch ("^cargo-deny {0}(?:\s|$)" -f [regex]::Escape([string]$manifest.qualification_tools.cargo_deny))) {
        throw "Installed cargo-deny does not match pinned $($manifest.qualification_tools.cargo_deny): $cargoDeny"
    }
    if ($cargoMachete -ne [string]$manifest.qualification_tools.cargo_machete) {
        throw "Installed cargo-machete does not match pinned $($manifest.qualification_tools.cargo_machete): $cargoMachete"
    }
}

[pscustomobject]@{
    schema_version = 1
    status = "PASS"
    manifest = $manifestPath
    installed_versions_checked = [bool]$CheckInstalled
    qualification_tools_checked = [bool]$CheckQualificationTools
    rust = [string]$manifest.rust.channel
    node = [string]$manifest.node.version
    npm = [string]$manifest.node.npm_version
    python = [string]$manifest.python.version
} | ConvertTo-Json -Compress
