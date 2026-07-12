[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
$scripts = @(
    "tools/bootstrap/bootstrap.ps1",
    "tools/bootstrap/install-node.ps1",
    "tools/bootstrap/install-rust-toolchain.ps1",
    "tools/bootstrap/verify-toolchain.ps1",
    "tools/product-gate/aggregate-reports.ps1",
    "tools/product-gate/hooks/module-loopback-smoke.ps1",
    "tools/product-gate/hooks/smoke-common.ps1",
    "tools/product-gate/product-gate.ps1"
)
foreach ($relativePath in $scripts) {
    $tokens = $null
    $errors = $null
    $path = Join-Path $repositoryRoot $relativePath
    [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -gt 0) {
        throw "PowerShell parse failure in ${relativePath}: $($errors[0].Message)"
    }
}

& (Join-Path $repositoryRoot "tools/bootstrap/verify-toolchain.ps1") | Out-Null

$gateSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/product-gate.ps1"))
if ([regex]::Matches($gateSource, '-Id "evidence\.module_loopback"').Count -ne 1) {
    throw "The product gate must contain exactly one evidence.module_loopback item."
}
if ($gateSource -notmatch '-HookFile "module-loopback-smoke\.ps1"') {
    throw "The module-loopback gate item is not wired to its strict hook."
}

$smokeCommonSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/hooks/smoke-common.ps1"))
if ($smokeCommonSource -notmatch '& /bin/kill -TERM \$processId') {
    throw "The root smoke workflow must request graceful SIGTERM shutdown on non-Windows hosts."
}

$testRoot = Join-Path $repositoryRoot "target/product-gate/contract-tests"
if (Test-Path -LiteralPath $testRoot) {
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$expectedCommit = "0123456789abcdef0123456789abcdef01234567"
$toolchainHash = (Get-FileHash -LiteralPath (Join-Path $repositoryRoot "tools/bootstrap/toolchain.json") -Algorithm SHA256).Hash.ToLowerInvariant()
$nativePlatforms = @("windows", "macos", "linux")
$platforms = @($nativePlatforms + @("linux-armhf", "linux-aarch64", "windows-arm64"))
foreach ($platform in $platforms) {
    $directory = Join-Path $testRoot $platform
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $report = [ordered]@{
        schema_version = 1
        gate = "chataigne-product-gate"
        overall_status = "PASS"
        commit = [ordered]@{ sha = $expectedCommit; working_tree_dirty = $false }
        toolchain = [ordered]@{
            target_host = "test-$platform"
            os_description = "test-$platform"
            canonical_manifest_sha256 = $toolchainHash
        }
        required_platforms = @($platform)
        results = @(
            if ($nativePlatforms -contains $platform) {
                [ordered]@{ id = "evidence.module_loopback"; status = "PASS"; exit_code = 0 }
            }
            else {
                [ordered]@{ id = "compatibility.build"; status = "PASS"; exit_code = 0 }
            }
            [ordered]@{ id = "platform.$platform"; status = "PASS"; exit_code = 0 }
        )
    }
    [System.IO.File]::WriteAllText(
        (Join-Path $directory "product-gate-report.json"),
        ($report | ConvertTo-Json -Depth 8),
        [System.Text.UTF8Encoding]::new($false)
    )
}

$aggregatePath = Join-Path $testRoot "product-gate-aggregate.json"
& (Join-Path $repositoryRoot "tools/product-gate/aggregate-reports.ps1") `
    -ReportDirectory $testRoot `
    -ExpectedCommit $expectedCommit `
    -OutputPath $aggregatePath
$aggregate = [System.IO.File]::ReadAllText($aggregatePath) | ConvertFrom-Json
if ($aggregate.schema_version -ne 1 -or $aggregate.overall_status -ne "PASS") {
    throw "Exact-commit aggregate contract did not pass."
}
if (@($aggregate.reports.psobject.Properties).Count -ne 6) {
    throw "Exact-commit aggregate did not contain all native and ARM compatibility reports."
}

$mismatchPath = Join-Path $testRoot "product-gate-aggregate-mismatch.json"
$powershellExecutable = (Get-Process -Id $PID).Path
$mismatchExitCode = -1
$previousErrorActionPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = "Continue"
    & $powershellExecutable `
        -NoLogo `
        -NoProfile `
        -NonInteractive `
        -File (Join-Path $repositoryRoot "tools/product-gate/aggregate-reports.ps1") `
        -ReportDirectory $testRoot `
        -ExpectedCommit "ffffffffffffffffffffffffffffffffffffffff" `
        -OutputPath $mismatchPath 2>&1 | Out-Null
    $mismatchExitCode = $LASTEXITCODE
}
finally {
    $ErrorActionPreference = $previousErrorActionPreference
}
if ($mismatchExitCode -eq 0) {
    throw "Aggregate contract accepted reports from a different commit."
}
$mismatch = [System.IO.File]::ReadAllText($mismatchPath) | ConvertFrom-Json
if ($mismatch.overall_status -ne "FAIL" -or @($mismatch.failures).Count -lt 6) {
    throw "Commit-mismatch aggregate did not preserve explicit failure evidence."
}

Write-Host "PASS product-gate PowerShell, wiring, toolchain, and aggregate contracts"
