[CmdletBinding()]
param(
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]] $Command
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$env:CARGO_TARGET_DIR = Join-Path $repositoryRoot "target"
& (Join-Path $PSScriptRoot "verify-toolchain.ps1") -CheckInstalled
& (Join-Path $repositoryRoot "tools\workspace-hygiene.ps1") -Action Audit

if ($null -ne $Command -and $Command.Length -gt 0) {
    $executable = $Command[0]
    $arguments = @($Command | Select-Object -Skip 1)
    & $executable @arguments
    exit $LASTEXITCODE
}
