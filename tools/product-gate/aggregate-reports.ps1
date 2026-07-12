[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $ReportDirectory,
    [Parameter(Mandatory = $true)]
    [string] $ExpectedCommit,
    [Parameter(Mandatory = $true)]
    [string] $OutputPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$nativePlatforms = @("windows", "macos", "linux")
$compatibilityPlatforms = @("linux-armhf", "linux-aarch64", "windows-arm64")
$platforms = @($nativePlatforms + $compatibilityPlatforms)
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$toolchainManifestPath = Join-Path $repositoryRoot "tools/bootstrap/toolchain.json"
$expectedToolchainHash = (Get-FileHash -LiteralPath $toolchainManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
$reportFiles = @(Get-ChildItem -LiteralPath $ReportDirectory -Recurse -File -Filter "product-gate-report.json")
$failures = @()
$summaries = [ordered]@{}

foreach ($platform in $platforms) {
    $matches = @()
    foreach ($file in $reportFiles) {
        try {
            $report = [System.IO.File]::ReadAllText($file.FullName) | ConvertFrom-Json
        }
        catch {
            $failures += "Invalid JSON report '$($file.FullName)': $($_.Exception.Message)"
            continue
        }
        $required = @($report.required_platforms | ForEach-Object { [string]$_ })
        if ($required.Count -eq 1 -and $required[0] -eq $platform) {
            $matches += [pscustomobject]@{ file = $file; report = $report }
        }
    }
    if ($matches.Count -ne 1) {
        $failures += "Expected one $platform report, found $($matches.Count)."
        continue
    }

    $match = $matches[0]
    $report = $match.report
    if ([int]$report.schema_version -ne 1 -or $report.gate -ne "chataigne-product-gate") {
        $failures += "$platform report has an unsupported schema or gate ID."
    }
    if ($report.commit.sha -ne $ExpectedCommit) {
        $failures += "$platform report commit '$($report.commit.sha)' does not match '$ExpectedCommit'."
    }
    if ([bool]$report.commit.working_tree_dirty) {
        $failures += "$platform report was produced from a dirty working tree."
    }
    if ($report.overall_status -ne "PASS") {
        $failures += "$platform report overall status is '$($report.overall_status)'."
    }
    if ($report.toolchain.canonical_manifest_sha256 -ne $expectedToolchainHash) {
        $failures += "$platform report used a different canonical toolchain manifest."
    }
    $platformResult = @($report.results | Where-Object { $_.id -eq "platform.$platform" })
    if ($platformResult.Count -ne 1 -or $platformResult[0].status -ne "PASS") {
        $failures += "$platform report does not contain one passing platform.$platform result."
    }
    if ($nativePlatforms -contains $platform) {
        $loopback = @($report.results | Where-Object { $_.id -eq "evidence.module_loopback" })
        if ($loopback.Count -ne 1 -or $loopback[0].status -ne "PASS" -or [int]$loopback[0].exit_code -ne 0) {
            $failures += "$platform report does not contain passing module-loopback evidence."
        }
    }
    else {
        $compatibility = @($report.results | Where-Object { $_.id -eq "compatibility.build" })
        if ($compatibility.Count -ne 1 -or $compatibility[0].status -ne "PASS" -or [int]$compatibility[0].exit_code -ne 0) {
            $failures += "$platform report does not contain one passing compatibility.build result."
        }
    }

    $summaries[$platform] = [ordered]@{
        source_report = $match.file.FullName
        sha256 = (Get-FileHash -LiteralPath $match.file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        overall_status = $report.overall_status
        target_host = $report.toolchain.target_host
        os_description = $report.toolchain.os_description
        canonical_manifest_sha256 = $report.toolchain.canonical_manifest_sha256
    }
}

$aggregate = [ordered]@{
    schema_version = 1
    gate = "chataigne-product-gate-aggregate"
    commit = $ExpectedCommit
    overall_status = if ($failures.Count -eq 0) { "PASS" } else { "FAIL" }
    required_platforms = $platforms
    reports = $summaries
    failures = $failures
}
$outputDirectory = Split-Path -Parent ([System.IO.Path]::GetFullPath($OutputPath))
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
[System.IO.File]::WriteAllText(
    [System.IO.Path]::GetFullPath($OutputPath),
    ($aggregate | ConvertTo-Json -Depth 10),
    [System.Text.UTF8Encoding]::new($false)
)

if ($failures.Count -gt 0) {
    throw ($failures -join " ")
}
Write-Host "Aggregated exact-commit native and ARM compatibility schema-v1 product-gate reports."
