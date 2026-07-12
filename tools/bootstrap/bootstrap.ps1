[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Command
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

& (Join-Path $PSScriptRoot "install-rust-toolchain.ps1")
$nodeDirectory = & (Join-Path $PSScriptRoot "install-node.ps1")
if ([string]::IsNullOrWhiteSpace($nodeDirectory)) {
    throw "Pinned Node installer did not return its local bin directory."
}
$env:PATH = "$nodeDirectory$([System.IO.Path]::PathSeparator)$env:PATH"
& (Join-Path $PSScriptRoot "verify-toolchain.ps1") -CheckInstalled

if ($null -ne $Command -and $Command.Length -gt 0) {
    $executable = $Command[0]
    $arguments = @($Command | Select-Object -Skip 1)
    & $executable @arguments
    exit $LASTEXITCODE
}
