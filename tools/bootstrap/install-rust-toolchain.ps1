[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$manifest = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot "toolchain.json")) | ConvertFrom-Json
$channel = [string]$manifest.rust.channel
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$runningOnWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)
$runningOnMacOS = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::OSX
)
if ($runningOnWindows) {
    $hostTriple = switch ($architecture) {
        "X64" { [string]$manifest.rust.hosts.windows_x64 }
        "Arm64" { [string]$manifest.rust.hosts.windows_arm64 }
        default { throw "Unsupported Windows Rust architecture '$architecture'." }
    }
}
elseif ($runningOnMacOS) {
    $hostTriple = switch ($architecture) {
        "X64" { [string]$manifest.rust.hosts.macos_x64 }
        "Arm64" { [string]$manifest.rust.hosts.macos_arm64 }
        default { throw "Unsupported macOS Rust architecture '$architecture'." }
    }
}
else {
    $hostTriple = switch ($architecture) {
        "X64" { [string]$manifest.rust.hosts.linux_x64 }
        "Arm64" { [string]$manifest.rust.hosts.linux_arm64 }
        "Arm" { [string]$manifest.rust.hosts.linux_armv7 }
        default { throw "Unsupported Linux Rust architecture '$architecture'." }
    }
}
$toolchain = "$channel-$hostTriple"

$arguments = @("toolchain", "install", $toolchain, "--profile", [string]$manifest.rust.profile)
foreach ($component in @($manifest.rust.components)) {
    $arguments += @("--component", [string]$component)
}
& rustup @arguments
if ($LASTEXITCODE -ne 0) {
    throw "rustup failed to install '$toolchain'."
}
& rustup override set $toolchain
if ($LASTEXITCODE -ne 0) {
    throw "rustup failed to activate '$toolchain'."
}
Write-Host "Activated repository-pinned Rust toolchain $toolchain."
