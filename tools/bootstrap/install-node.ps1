[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$manifest = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot "toolchain.json")) | ConvertFrom-Json
$isWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)
if (-not $isWindows) {
    throw "install-node.ps1 is the Windows entry point; use install-node.sh on macOS/Linux."
}
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$distributionKey = switch ($architecture) {
    "X64" { "windows_x64" }
    "Arm64" { "windows_arm64" }
    default { throw "Unsupported Windows Node architecture '$architecture'." }
}
$distribution = $manifest.node.distributions.$distributionKey
$version = [string]$manifest.node.version
$archiveName = [string]$distribution.file
$expectedHash = ([string]$distribution.sha256).ToLowerInvariant()
$cacheRoot = Join-Path $repositoryRoot "target\toolchains"
$downloadDirectory = Join-Path $cacheRoot "downloads"
$installDirectory = Join-Path $cacheRoot "node-v$version-$distributionKey"
$nodeExecutable = Join-Path $installDirectory "node.exe"

if (-not (Test-Path -LiteralPath $nodeExecutable -PathType Leaf)) {
    New-Item -ItemType Directory -Path $downloadDirectory -Force | Out-Null
    $archivePath = Join-Path $downloadDirectory $archiveName
    $downloadRequired = $true
    if (Test-Path -LiteralPath $archivePath -PathType Leaf) {
        $downloadRequired = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash
    }
    if ($downloadRequired) {
        $url = "$($manifest.node.base_url)/v$version/$archiveName"
        Write-Host "Downloading pinned Node $version from $url"
        Invoke-WebRequest -Uri $url -OutFile $archivePath
    }
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $expectedHash) {
        Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
        throw "Node archive SHA-256 mismatch: expected $expectedHash, got $actualHash."
    }

    $temporaryDirectory = "$installDirectory.extracting-$PID"
    if (Test-Path -LiteralPath $temporaryDirectory) {
        Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
    }
    New-Item -ItemType Directory -Path $temporaryDirectory -Force | Out-Null
    try {
        Expand-Archive -LiteralPath $archivePath -DestinationPath $temporaryDirectory -Force
        $roots = @(Get-ChildItem -LiteralPath $temporaryDirectory -Directory)
        if ($roots.Count -ne 1) {
            throw "Pinned Node archive must contain exactly one root directory."
        }
        if (Test-Path -LiteralPath $installDirectory) {
            Remove-Item -LiteralPath $installDirectory -Recurse -Force
        }
        Move-Item -LiteralPath $roots[0].FullName -Destination $installDirectory
    }
    finally {
        if (Test-Path -LiteralPath $temporaryDirectory) {
            Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
        }
    }
}

if (-not (Test-Path -LiteralPath $nodeExecutable -PathType Leaf)) {
    throw "Pinned Node executable was not installed at '$nodeExecutable'."
}
Write-Output $installDirectory
