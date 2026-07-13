[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$manifest = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot "toolchain.json")) | ConvertFrom-Json
$tools = @(
    @{
        package = "cargo-deny"
        executable = "cargo-deny"
        version = [string]$manifest.qualification_tools.cargo_deny
        version_pattern = "^cargo-deny {0}(?:\s|$)"
    },
    @{
        package = "cargo-machete"
        executable = "cargo-machete"
        version = [string]$manifest.qualification_tools.cargo_machete
        version_pattern = "^{0}$"
    }
)

foreach ($tool in $tools) {
    $installed = ""
    $command = Get-Command $tool.executable -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $command) {
        $installed = (& $command.Source --version 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    }
    if ($installed -notmatch ($tool.version_pattern -f [regex]::Escape($tool.version))) {
        & cargo install --locked --force $tool.package --version $tool.version
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to install $($tool.package) $($tool.version)."
        }
    }
}

& (Join-Path $PSScriptRoot "verify-toolchain.ps1") -CheckQualificationTools
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
