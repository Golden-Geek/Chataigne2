# Run with Windows PowerShell or pwsh. Exercise real deletion only in an isolated Git fixture.
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$fixtureRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("golden-hygiene-" + [guid]::NewGuid())
$scriptPath = Join-Path $fixtureRoot "tools/workspace-hygiene.ps1"
$shellPath = (Get-Process -Id $PID).Path

function Add-FixtureFile([string] $RelativePath) {
    $path = Join-Path $fixtureRoot $RelativePath
    New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
    [System.IO.File]::WriteAllText($path, "fixture")
}

function Assert-Exists([string] $RelativePath, [bool] $Expected) {
    if ((Test-Path -LiteralPath (Join-Path $fixtureRoot $RelativePath)) -ne $Expected) {
        throw "Unexpected existence for $RelativePath (expected $Expected)"
    }
}

function Invoke-Hygiene([string[]] $Arguments) {
    # Windows PowerShell surfaces native stderr as errors even when redirected;
    # expected rejections are asserted through the child exit code below.
    $ErrorActionPreference = "Continue"
    & $shellPath -NoProfile -File $scriptPath @Arguments *> $null
    return $LASTEXITCODE
}

try {
    New-Item -ItemType Directory -Path (Split-Path -Parent $scriptPath) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "../workspace-hygiene.ps1") -Destination $scriptPath
    & git -C $fixtureRoot init --quiet
    if ($LASTEXITCODE -ne 0) { throw "Cannot initialize fixture repository" }

    foreach ($path in @(
        "target/debug/cache", "target/release/cache", "target/experiment/debug/cache",
        "golden/target/debug/cache",
        "target/scratch.rs", "node_modules/dependency/package.json",
        "apps/chataigne/example/target/debug/cache", "apps/chataigne/ui/build/index.html",
        "apps/chataigne/ui/.svelte-kit/cache", "apps/chataigne/gen/cache"
    )) { Add-FixtureFile $path }

    if ((Invoke-Hygiene @("-Action", "Audit")) -eq 0) { throw "Audit missed a nested Cargo target" }
    if ((Invoke-Hygiene @("-Action", "Clean", "-KeepDependencies", "-WhatIf")) -ne 0) {
        throw "Cleanup preview failed"
    }
    Assert-Exists "target/debug/cache" $true

    Add-FixtureFile "target/debug/tracked.txt"
    & git -C $fixtureRoot add --force target/debug/tracked.txt
    if ($LASTEXITCODE -ne 0) { throw "Cannot stage fixture" }
    if ((Invoke-Hygiene @("-Action", "Clean", "-KeepDependencies")) -eq 0) {
        throw "Cleanup accepted tracked content"
    }
    Assert-Exists "target/release/cache" $true
    Assert-Exists "target/debug/tracked.txt" $true
    & git -C $fixtureRoot rm --cached --quiet target/debug/tracked.txt
    if ($LASTEXITCODE -ne 0) { throw "Cannot unstage fixture" }

    if ((Invoke-Hygiene @("-Action", "Clean", "-KeepDependencies")) -ne 0) {
        throw "Routine cleanup failed"
    }
    foreach ($path in @("target", "apps/chataigne/example/target", "apps/chataigne/ui/build",
        "apps/chataigne/ui/.svelte-kit", "apps/chataigne/gen")) {
        Assert-Exists $path $false
    }
    foreach ($path in @("node_modules/dependency/package.json", "golden/target/debug/cache")) {
        Assert-Exists $path $true
    }
    if ((Invoke-Hygiene @("-Action", "Audit")) -ne 0) { throw "Audit failed after cleanup" }
    if ((Invoke-Hygiene @("-Action", "Clean")) -ne 0) { throw "Full reset failed" }
    Assert-Exists "target" $false
    Assert-Exists "node_modules" $false
    Assert-Exists "golden/target/debug/cache" $true
    Write-Host "PASS hygiene: preview, tracked-file safety, nested targets, checkout and dependency preservation, audit, full reset"
} finally {
    $resolvedFixture = [System.IO.Path]::GetFullPath($fixtureRoot)
    $tempPrefix = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
    if (-not $resolvedFixture.StartsWith($tempPrefix, [System.StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $resolvedFixture) -notlike "golden-hygiene-*") {
        throw "Unsafe fixture cleanup path: $resolvedFixture"
    }
    Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
}
