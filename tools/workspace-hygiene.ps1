[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = "Medium")]
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
$inspectionFailures = [System.Collections.Generic.List[string]]::new()

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
    $ancestor = $fullPath
    while (Test-IsInsideRepository $ancestor) {
        if ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
            throw "Refusing a generated path through a filesystem link: $ancestor"
        }
        $ancestor = Split-Path -Parent $ancestor
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
    $pending = [System.Collections.Generic.Stack[System.IO.DirectoryInfo]]::new()
    $pending.Push([System.IO.DirectoryInfo]::new($Path))
    while ($pending.Count -gt 0) {
        $directory = $pending.Pop()
        try {
            $entries = @($directory.EnumerateFileSystemInfos())
        } catch {
            $inspectionFailures.Add($directory.FullName)
            Write-Warning "Cannot measure $($directory.FullName): $($_.Exception.Message)"
            continue
        }
        foreach ($entry in $entries) {
            if ($entry.Attributes -band [System.IO.FileAttributes]::ReparsePoint) { continue }
            if ($entry -is [System.IO.DirectoryInfo]) {
                $pending.Push($entry)
            } else {
                $bytes += [int64]$entry.Length
            }
        }
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

foreach ($sourceRoot in @(
    (Join-Path $repositoryRoot "apps"),
    (Join-Path $repositoryRoot "tools"),
    (Join-Path $repositoryRoot "xtask")
)) {
    if (Test-Path -LiteralPath $sourceRoot -PathType Container) {
        $pending = [System.Collections.Generic.Stack[string]]::new()
        $pending.Push($sourceRoot)
        while ($pending.Count -gt 0) {
            foreach ($directory in Get-ChildItem -LiteralPath $pending.Pop() -Directory -Force) {
                if ($directory.Attributes -band [System.IO.FileAttributes]::ReparsePoint) { continue }
                if ($directory.Name -eq "target" -or $directory.Name -like "target-*") {
                    Add-Candidate -Path $directory.FullName -Category "cargo-target-noncanonical" -Cleanable $true
                } elseif ($directory.Name -in @("__pycache__", ".pytest_cache", ".ruff_cache", ".mypy_cache")) {
                    Add-Candidate -Path $directory.FullName -Category "language-cache" -Cleanable $true
                } elseif ($directory.Name -notin @("node_modules", ".git", "build", "dist", ".svelte-kit")) {
                    $pending.Push($directory.FullName)
                }
            }
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
    # Validate the entire deletion set before changing anything. An accidentally
    # tracked build fixture must never be removed by a hygiene command.
    $trackedDirectories = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $trackedFiles = @(git -C $repositoryRoot -c core.quotepath=false ls-files)
    if ($LASTEXITCODE -ne 0) { throw "Cannot verify tracked files before cleanup." }
    foreach ($trackedFile in $trackedFiles) {
        $parent = Split-Path -Parent (Join-Path $repositoryRoot $trackedFile)
        while (Test-IsInsideRepository $parent) {
            [void]$trackedDirectories.Add($parent)
            $parent = Split-Path -Parent $parent
        }
    }
    foreach ($candidate in $candidates.Values | Where-Object { $_.Cleanable }) {
        if ($trackedDirectories.Contains($candidate.Path)) {
            throw "Refusing to delete a directory containing tracked files: $($candidate.Path)"
        }
    }
    $failures = @()
    foreach ($candidate in $candidates.Values | Where-Object { $_.Cleanable } | Sort-Object { $_.Path.Length } -Descending) {
        if (-not (Test-IsInsideRepository $candidate.Path)) {
            throw "Refusing to clean a path outside the repository: $($candidate.Path)"
        }
        try {
            if ($PSCmdlet.ShouldProcess($candidate.Path, "Remove reproducible workspace output")) {
                Remove-Item -LiteralPath $candidate.Path -Recurse -Force -ErrorAction Stop
                Write-Host "Removed $($candidate.RelativePath)"
            }
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
    throw "Noncanonical generated directories found: $paths. Run npm run clean. For agent worktrees, stop their builds and add -IncludeAgentWorktrees."
}
if ($inspectionFailures.Count -gt 0) {
    throw "Workspace audit is incomplete: $($inspectionFailures.Count) unreadable directories. Reported size is a lower bound."
}
if ($totalGiB -gt $MaximumGiB) {
    throw "Generated workspace data exceeds the $MaximumGiB GiB budget. Run npm run clean to keep installed dependencies and prepared SDKs."
}

Write-Host "PASS workspace hygiene audit"
