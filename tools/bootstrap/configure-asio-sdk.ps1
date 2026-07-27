[CmdletBinding()]
param(
    [string] $SdkPath,
    [string] $CacheRoot,
    [switch] $Offline,
    [switch] $PassThru
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows
    )) {
    throw "The ASIO SDK setup is supported only on Windows."
}

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$manifestPath = Join-Path $PSScriptRoot "toolchain.json"
$manifest = [System.IO.File]::ReadAllText($manifestPath) | ConvertFrom-Json
$sdkContract = $manifest.audio.asio_sdk
$repository = [string]$sdkContract.repository
$revision = [string]$sdkContract.revision
$requiredPaths = @($sdkContract.required_paths | ForEach-Object { [string]$_ })

function Get-ResolvedDirectory {
    param([string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "ASIO SDK directory does not exist: $Path"
    }
    return [System.IO.Path]::GetFullPath((Get-Item -LiteralPath $Path).FullName)
}

function Test-PathInside {
    param(
        [string] $Candidate,
        [string] $Parent
    )

    $candidatePath = [System.IO.Path]::GetFullPath($Candidate)
    $parentPath = [System.IO.Path]::GetFullPath($Parent).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $parentPrefix = $parentPath + [System.IO.Path]::DirectorySeparatorChar
    return $candidatePath.StartsWith(
        $parentPrefix,
        [System.StringComparison]::OrdinalIgnoreCase
    )
}

function Assert-ExternalPath {
    param([string] $Path)

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    if ($resolvedPath -eq $repositoryRoot -or
        (Test-PathInside -Candidate $resolvedPath -Parent $repositoryRoot)) {
        throw "ASIO SDK material must remain outside the repository checkout: $resolvedPath"
    }
}

function Assert-SdkLayout {
    param([string] $Path)

    foreach ($relativePath in $requiredPaths) {
        $candidate = Join-Path $Path ($relativePath -replace "/", "\")
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            throw "ASIO SDK at '$Path' is missing required file '$relativePath'."
        }
    }
}

function Invoke-Git {
    param([string[]] $Arguments)

    $output = @(& $script:gitCommand.Source @Arguments | ForEach-Object { [string]$_ })
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed with exit code $LASTEXITCODE."
    }
    return $output
}

function Assert-PinnedCheckout {
    param([string] $Path)

    Assert-SdkLayout -Path $Path
    if (-not (Test-Path -LiteralPath (Join-Path $Path ".git") -PathType Container)) {
        throw "Pinned ASIO SDK cache is not a Git checkout: $Path"
    }
    $head = (Invoke-Git -Arguments @("-C", $Path, "rev-parse", "HEAD") | Select-Object -Last 1).Trim()
    if ($head -ne $revision) {
        throw "ASIO SDK cache revision mismatch at '$Path': expected $revision, got $head."
    }
}

$configuredPath = $null
$configuredSource = $null

if (-not [string]::IsNullOrWhiteSpace($SdkPath)) {
    $configuredPath = Get-ResolvedDirectory -Path $SdkPath
    Assert-ExternalPath -Path $configuredPath
    Assert-SdkLayout -Path $configuredPath
    $configuredSource = "explicit local SDK"
} elseif ([string]::IsNullOrWhiteSpace($CacheRoot) -and
    -not [string]::IsNullOrWhiteSpace($env:CPAL_ASIO_DIR)) {
    $configuredPath = Get-ResolvedDirectory -Path $env:CPAL_ASIO_DIR
    Assert-ExternalPath -Path $configuredPath
    Assert-SdkLayout -Path $configuredPath
    $configuredSource = "existing CPAL_ASIO_DIR"
} else {
    if ([string]::IsNullOrWhiteSpace($CacheRoot)) {
        if (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
            $CacheRoot = Join-Path $env:LOCALAPPDATA "Chataigne2\tool-cache\asio-sdk"
        } else {
            $CacheRoot = Join-Path ([System.IO.Path]::GetTempPath()) "Chataigne2\tool-cache\asio-sdk"
        }
    }

    $CacheRoot = [System.IO.Path]::GetFullPath($CacheRoot)
    Assert-ExternalPath -Path $CacheRoot
    if ($CacheRoot -eq [System.IO.Path]::GetPathRoot($CacheRoot)) {
        throw "The ASIO SDK cache root cannot be a filesystem root: $CacheRoot"
    }
    $configuredPath = Join-Path $CacheRoot $revision
    if (Test-Path -LiteralPath $configuredPath -PathType Container) {
        $script:gitCommand = Get-Command "git" -ErrorAction Stop | Select-Object -First 1
        Assert-PinnedCheckout -Path $configuredPath
    } else {
        if ($Offline) {
            throw "Pinned ASIO SDK $revision is not cached at '$configuredPath', and -Offline forbids acquisition."
        }
        $script:gitCommand = Get-Command "git" -ErrorAction Stop | Select-Object -First 1
        New-Item -ItemType Directory -Path $CacheRoot -Force | Out-Null
        $stagingPath = Join-Path $CacheRoot (".partial-{0}-{1}" -f $PID, [guid]::NewGuid().ToString("N"))
        try {
            Invoke-Git -Arguments @("init", "--quiet", $stagingPath) | Out-Null
            Invoke-Git -Arguments @("-C", $stagingPath, "config", "core.autocrlf", "false") | Out-Null
            Invoke-Git -Arguments @("-C", $stagingPath, "remote", "add", "origin", $repository) | Out-Null
            Invoke-Git -Arguments @(
                "-C",
                $stagingPath,
                "fetch",
                "--depth",
                "1",
                "origin",
                $revision
            ) | Out-Null
            Invoke-Git -Arguments @(
                "-C",
                $stagingPath,
                "checkout",
                "--quiet",
                "--detach",
                "FETCH_HEAD"
            ) | Out-Null
            Assert-PinnedCheckout -Path $stagingPath

            try {
                [System.IO.Directory]::Move($stagingPath, $configuredPath)
                $stagingPath = $null
            } catch {
                if (-not (Test-Path -LiteralPath $configuredPath -PathType Container)) {
                    throw
                }
                Assert-PinnedCheckout -Path $configuredPath
            }
        } finally {
            if ($null -ne $stagingPath -and (Test-Path -LiteralPath $stagingPath)) {
                $resolvedStaging = [System.IO.Path]::GetFullPath($stagingPath)
                $cachePrefix = $CacheRoot.TrimEnd("\") + "\"
                if (-not $resolvedStaging.StartsWith(
                        $cachePrefix,
                        [System.StringComparison]::OrdinalIgnoreCase
                    ) -or (Split-Path -Leaf $resolvedStaging) -notlike ".partial-*") {
                    throw "Refusing to clean ASIO SDK staging path outside the cache root: $resolvedStaging"
                }
                Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
            }
        }
    }
    $configuredPath = Get-ResolvedDirectory -Path $configuredPath
    $configuredSource = "pinned audiosdk/asio checkout $revision"
}

$env:CPAL_ASIO_DIR = $configuredPath
Write-Host "ASIO SDK ready from $configuredSource."
Write-Host "CPAL_ASIO_DIR=$configuredPath"

if ($PassThru) {
    Write-Output $configuredPath
}
