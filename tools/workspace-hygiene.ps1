[CmdletBinding()]
param(
    [ValidateSet("Audit", "Clean")]
    [string] $Action = "Audit",
    [ValidateRange(1, 4096)]
    [double] $MaximumGiB = 25,
    [switch] $KeepDependencies,
    [switch] $IncludeAgentWorktrees
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$runningOnWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)
$pathComparison = if ($runningOnWindows) {
    [System.StringComparison]::OrdinalIgnoreCase
}
else {
    [System.StringComparison]::Ordinal
}
$rootPrefix = $repositoryRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar
$candidates = @{}

function Get-RelativePath {
    param(
        [string] $BasePath,
        [string] $Path
    )

    $baseWithSeparator = $BasePath.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    $baseUri = [System.Uri]::new($baseWithSeparator)
    $pathUri = [System.Uri]::new($Path)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace(
        '/',
        [System.IO.Path]::DirectorySeparatorChar
    )
}

function Test-IsInsideRepository {
    param([string] $Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    return $fullPath.StartsWith($rootPrefix, $pathComparison)
}

function Add-Candidate {
    param(
        [string] $Path,
        [string] $Category,
        [bool] $Cleanable,
        [bool] $CanonicalCargoTarget = $false
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $fullPath -PathType Container)) {
        return
    }
    if (-not (Test-IsInsideRepository $fullPath)) {
        throw "Refusing to inspect a path outside the repository: $fullPath"
    }
    if ($fullPath -eq (Join-Path $repositoryRoot ".git") -or
        $fullPath.StartsWith((Join-Path $repositoryRoot ".git") + [System.IO.Path]::DirectorySeparatorChar, $pathComparison)) {
        throw "Refusing to treat Git metadata as generated data: $fullPath"
    }

    $key = $fullPath.ToLowerInvariant()
    if (-not $candidates.ContainsKey($key)) {
        $candidates[$key] = [pscustomobject]@{
            Path = $fullPath
            RelativePath = Get-RelativePath -BasePath $repositoryRoot -Path $fullPath
            Category = $Category
            Cleanable = $Cleanable
            CanonicalCargoTarget = $CanonicalCargoTarget
        }
    }
}

function Add-GeneratedChildren {
    param(
        [string] $Parent,
        [bool] $Cleanable,
        [bool] $DependencyCleanable = $Cleanable
    )

    if (-not (Test-Path -LiteralPath $Parent -PathType Container)) {
        return
    }
    foreach ($name in @(
        "node_modules",
        ".svelte-kit",
        "build",
        "dist",
        "coverage",
        "artifacts",
        "playwright-report",
        "test-results",
        ".pytest_cache",
        ".ruff_cache",
        ".mypy_cache"
    )) {
        $entryCleanable = if ($name -eq "node_modules") { $DependencyCleanable } else { $Cleanable }
        Add-Candidate -Path (Join-Path $Parent $name) -Category "generated" -Cleanable $entryCleanable
    }
}

function Get-DirectorySize {
    param([string] $Path)

    $bytes = [int64]0
    foreach ($file in Get-ChildItem -LiteralPath $Path -File -Recurse -Force -ErrorAction SilentlyContinue) {
        $bytes += [int64]$file.Length
    }
    return $bytes
}

$canonicalTarget = Join-Path $repositoryRoot "target"
Add-Candidate -Path $canonicalTarget -Category "cargo-target" -Cleanable $true -CanonicalCargoTarget $true
foreach ($directory in Get-ChildItem -LiteralPath $repositoryRoot -Directory -Force -ErrorAction SilentlyContinue) {
    if ($directory.Name -like "target-*") {
        Add-Candidate -Path $directory.FullName -Category "cargo-target-noncanonical" -Cleanable $true
    }
}

$dependencyCleanable = -not $KeepDependencies
Add-Candidate -Path (Join-Path $repositoryRoot ".venv") -Category "local-toolchain" -Cleanable $dependencyCleanable
Add-GeneratedChildren -Parent $repositoryRoot -Cleanable $true -DependencyCleanable $dependencyCleanable
Add-GeneratedChildren -Parent (Join-Path $repositoryRoot "apps\chataigne\ui") -Cleanable $true -DependencyCleanable $dependencyCleanable
Add-Candidate -Path (Join-Path $repositoryRoot "apps\chataigne\gen") -Category "generated" -Cleanable $true

foreach ($packageRoot in @(
    (Join-Path $repositoryRoot "apps"),
    (Join-Path $repositoryRoot "packages")
)) {
    if (Test-Path -LiteralPath $packageRoot -PathType Container) {
        foreach ($package in Get-ChildItem -LiteralPath $packageRoot -Directory -Force -ErrorAction SilentlyContinue) {
            Add-GeneratedChildren -Parent $package.FullName -Cleanable $true -DependencyCleanable $dependencyCleanable
        }
    }
}

foreach ($sourceRoot in @(
    (Join-Path $repositoryRoot "apps"),
    (Join-Path $repositoryRoot "crates"),
    (Join-Path $repositoryRoot "packages"),
    (Join-Path $repositoryRoot "tools"),
    (Join-Path $repositoryRoot "xtask")
)) {
    if (Test-Path -LiteralPath $sourceRoot -PathType Container) {
        foreach ($cache in Get-ChildItem -LiteralPath $sourceRoot -Directory -Recurse -Force -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -in @("__pycache__", ".pytest_cache", ".ruff_cache", ".mypy_cache") }) {
            Add-Candidate -Path $cache.FullName -Category "language-cache" -Cleanable $true
        }
    }
}

$agentRoot = Join-Path $repositoryRoot ".kilo"
Add-Candidate -Path (Join-Path $agentRoot "node_modules") -Category "agent-dependency" -Cleanable ([bool]$IncludeAgentWorktrees)
$worktreeRoot = Join-Path $agentRoot "worktrees"
if (Test-Path -LiteralPath $worktreeRoot -PathType Container) {
    foreach ($worktree in Get-ChildItem -LiteralPath $worktreeRoot -Directory -Force -ErrorAction SilentlyContinue) {
        foreach ($directory in Get-ChildItem -LiteralPath $worktree.FullName -Directory -Force -ErrorAction SilentlyContinue) {
            if ($directory.Name -eq "target" -or $directory.Name -like "target-*") {
                Add-Candidate -Path $directory.FullName -Category "cargo-target-noncanonical" -Cleanable ([bool]$IncludeAgentWorktrees)
            }
        }
        Add-Candidate -Path (Join-Path $worktree.FullName ".kilo\node_modules") -Category "agent-dependency" -Cleanable ([bool]$IncludeAgentWorktrees)
        Add-GeneratedChildren -Parent $worktree.FullName -Cleanable ([bool]$IncludeAgentWorktrees)
        Add-GeneratedChildren -Parent (Join-Path $worktree.FullName "apps\chataigne\ui") -Cleanable ([bool]$IncludeAgentWorktrees)
    }
}

$rows = @()
$totalBytes = [int64]0
foreach ($candidate in $candidates.Values) {
    $bytes = Get-DirectorySize -Path $candidate.Path
    $totalBytes += $bytes
    $rows += [pscustomobject]@{
        GiB = [math]::Round($bytes / 1GB, 3)
        Category = $candidate.Category
        Cleanable = $candidate.Cleanable
        Path = $candidate.RelativePath
    }
}

if ($rows.Count -gt 0) {
    $rows | Sort-Object GiB -Descending | Format-Table -AutoSize
}
$totalGiB = [math]::Round($totalBytes / 1GB, 3)
Write-Host "Generated workspace data: $totalGiB GiB (budget: $MaximumGiB GiB)."

if ($Action -eq "Clean") {
    $failures = @()
    foreach ($candidate in $candidates.Values | Where-Object { $_.Cleanable } | Sort-Object { $_.Path.Length } -Descending) {
        if (-not (Test-IsInsideRepository $candidate.Path)) {
            throw "Refusing to clean a path outside the repository: $($candidate.Path)"
        }
        try {
            Remove-Item -LiteralPath $candidate.Path -Recurse -Force -ErrorAction Stop
            Write-Host "Removed $($candidate.RelativePath)"
        }
        catch {
            $failures += "$($candidate.RelativePath): $($_.Exception.Message)"
        }
    }
    if ($failures.Count -gt 0) {
        throw "Workspace cleanup could not remove:`n$($failures -join "`n")"
    }
    exit 0
}

$policyViolations = @($candidates.Values | Where-Object {
    $_.Category -eq "cargo-target-noncanonical"
})
if ($policyViolations.Count -gt 0) {
    $paths = @($policyViolations | ForEach-Object { $_.RelativePath }) -join ", "
    throw "Noncanonical generated directories found: $paths. Run tools/workspace-hygiene.ps1 -Action Clean -IncludeAgentWorktrees."
}
if ($totalGiB -gt $MaximumGiB) {
    throw "Generated workspace data exceeds the $MaximumGiB GiB budget. Run tools/workspace-hygiene.ps1 -Action Clean."
}

Write-Host "PASS workspace hygiene audit"
