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
    "tools/product-gate/hooks/browser-gate-common.ps1",
    "tools/product-gate/hooks/module-loopback-smoke.ps1",
    "tools/product-gate/hooks/smoke-common.ps1",
    "tools/product-gate/hooks/watch-smoke.ps1",
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

$workflowSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot ".github/workflows/product-gate.yml"))
if ($workflowSource -notmatch 'native_platform:' -or
    $workflowSource -notmatch 'default: windows' -or
    $workflowSource -notmatch 'run_compatibility:' -or
    $workflowSource -notmatch "inputs\.native_platform != 'none'" -or
    $workflowSource -notmatch "inputs\.run_compatibility == true") {
    throw "Manual product qualification must default to a targeted Windows-only dispatch."
}
if ($workflowSource -notmatch 'cache-on-failure: true') {
    throw "Product-gate dependency caches must survive a late smoke failure."
}
if ($workflowSource -match 'key: (?:product-gate|compatibility)-[^\r\n]*hashFiles') {
    throw "rust-cache custom keys must not defer file hashing to the post-job phase."
}
if ($workflowSource -notmatch 'mozilla-actions/sccache-action@[0-9a-f]{40}' -or
    $workflowSource -notmatch 'SCCACHE_GHA_ENABLED: "true"' -or
    $workflowSource -notmatch 'RUSTC_WRAPPER: sccache' -or
    $workflowSource -notmatch 'cache-targets: false') {
    throw "Product qualification must use content-addressed compiler caching without duplicating target caches."
}
if ($workflowSource -notmatch 'windows_debug_session:' -or
    $workflowSource -notmatch "github.event_name == 'workflow_dispatch'.*inputs.windows_debug_session == true" -or
    $workflowSource -notmatch 'mxschmitt/action-tmate@[0-9a-f]{40}' -or
    $workflowSource -notmatch 'detached: true' -or
    $workflowSource -notmatch 'limit-access-to-actor: true') {
    throw "The live Windows debug session must be manual-only, pinned, detached, and actor-restricted."
}
if ($workflowSource -notmatch "inputs\.native_platform == 'all'.*inputs\.run_compatibility == true") {
    throw "Exact-commit aggregation must run only for the complete requested matrix."
}

$gateSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/product-gate.ps1"))
if ($gateSource -notmatch '\& \$resolvedExecutable \@Arguments 2>&1 \| ForEach-Object' -or
    $gateSource -notmatch 'Write-Host \$line') {
    throw "Product-gate commands must stream child output while retaining complete log files."
}
if ([regex]::Matches($gateSource, '-Id "evidence\.module_loopback"').Count -ne 1) {
    throw "The product gate must contain exactly one evidence.module_loopback item."
}
if ($gateSource -notmatch '-HookFile "module-loopback-smoke\.ps1"') {
    throw "The module-loopback gate item is not wired to its strict hook."
}
if ($gateSource -notmatch '-Id "ui\.browser_install"' -or
    $gateSource -notmatch '@\("exec", "--", "playwright-core", "install", "chromium"\)') {
    throw "The product gate must install the lockfile-pinned Playwright Chromium browser."
}
$uiBuildIndex = $gateSource.IndexOf('-Id "ui.build"')
$rustBuildIndex = $gateSource.IndexOf('-Id "rust.build"')
if ($uiBuildIndex -lt 0 -or $rustBuildIndex -lt 0 -or $uiBuildIndex -gt $rustBuildIndex) {
    throw "The product gate must produce the final UI bundle before compiling the Rust app."
}
if ($gateSource -notmatch '-Id "rust\.build"(?s:.*?)-DependsOn @\("toolchain\.contract", "ui\.build"\)') {
    throw "The Rust workspace build must depend on the final UI bundle."
}
$assumeBuiltIndex = $gateSource.IndexOf('$env:GC_UI_ASSUME_BUILT = "1"')
if ($assumeBuiltIndex -lt $uiBuildIndex -or $assumeBuiltIndex -gt $rustBuildIndex) {
    throw "Cargo checks must consume the validated UI bundle without rebuilding it."
}
$clippyIndex = $gateSource.IndexOf('-Id "rust.clippy"')
$testIndex = $gateSource.IndexOf('-Id "rust.test"')
$runtimeBuildIndex = $gateSource.IndexOf('-Id "rust.runtime_build"')
if ($clippyIndex -lt 0 -or $testIndex -lt 0 -or $runtimeBuildIndex -lt 0 -or
    $runtimeBuildIndex -lt $clippyIndex -or $runtimeBuildIndex -lt $testIndex) {
    throw "The final runtime build must restore normal Cargo artifacts after clippy and tests."
}
if ($gateSource -notmatch '-Id "rust\.runtime_build"(?s:.*?)-Arguments @\("build", "-p", "Chataigne2", "--bin", "Chataigne2"\)(?s:.*?)-DependsOn @\("rust\.format", "rust\.clippy", "rust\.test", "runtime\.phase6_release_fixtures"\)') {
    throw "The final runtime build must compile the exact Chataigne binary after Rust checks."
}
if ($gateSource -notmatch '-Id "smoke\.cargo_run"(?s:.*?)-DependsOn @\("rust\.runtime_build", "ui\.browser_install"\)') {
    throw "The root cargo-run smoke must consume the final runtime build."
}

$appBuildSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "apps/chataigne/build.rs"))
if ($appBuildSource -notmatch 'if env_flag\(GC_UI_ASSUME_BUILT\)(?s:.*?)emit_rerun_if_changed_for_dir\(&paths\.ui_root\.join\("build"\)\)') {
    throw "Assume-built Cargo runs must watch the validated UI artifact instead of live source caches."
}

$smokeCommonSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/hooks/smoke-common.ps1"))
if ($smokeCommonSource -notmatch '& /bin/kill -TERM \$processId') {
    throw "The root smoke workflow must request graceful SIGTERM shutdown on non-Windows hosts."
}
if ($smokeCommonSource -notmatch '\[System\.Diagnostics\.Process\[\]\]\$Processes' -or
    $smokeCommonSource -notmatch '\$process\.HasExited') {
    throw "Process shutdown verification must track process instances instead of reusable numeric PIDs."
}
if ($smokeCommonSource -notmatch 'function Wait-ForRootProcessToExit' -or
    $smokeCommonSource -notmatch '\$Process\.WaitForExit\(\$TimeoutSeconds \* 1000\)' -or
    $smokeCommonSource -notmatch 'Wait-ForRootProcessToExit -Process \$process -TimeoutSeconds 20') {
    throw "Product qualification must require only the root command exit and released product ports."
}
if ($smokeCommonSource -notmatch '\[datetime\]\$RootStartTimeUtc' -or
    $smokeCommonSource -notmatch '\$_\.CreationDate' -or
    $smokeCommonSource -notmatch 'CreationDate\)\.ToUniversalTime\(\) -ge \$RootStartTimeUtc') {
    throw "Windows process-tree ownership must reject stale parent PIDs from processes older than the root instance."
}

$cargoRunSmokeSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/hooks/cargo-run-smoke.ps1"))
if ($cargoRunSmokeSource -notmatch '"--automation-shutdown-file"' -or
    $cargoRunSmokeSource -notmatch 'Test-IsWindowsPlatform' -or
    $cargoRunSmokeSource -notmatch '-ShutdownFile \$shutdownFile' -or
    $smokeCommonSource -notmatch '\[System\.IO\.File\]::WriteAllText\(\$ShutdownFile, "stop"\)') {
    throw "The cargo-run smoke must stop the desktop runtime through its deterministic shutdown contract."
}

$cargoRunDevSmokeSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/hooks/cargo-run-dev-smoke.ps1"))
if ($cargoRunDevSmokeSource -notmatch '"--dev"' -or
    $cargoRunDevSmokeSource -notmatch '"--automation-shutdown-file"' -or
    $cargoRunDevSmokeSource -notmatch 'Test-IsWindowsPlatform' -or
    $cargoRunDevSmokeSource -notmatch '-ShutdownFile \$shutdownFile') {
    throw "The cargo-run dev smoke must use the deterministic Windows shutdown contract."
}

$desktopHostSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "crates/host_desktop/src/desktop.rs"))
if ($desktopHostSource -notmatch '"--automation-shutdown-file"' -or
    $desktopHostSource -notmatch 'app_handle\.exit\(0\)') {
    throw "The reusable desktop runtime must expose the automation shutdown contract."
}

$watchSmokeSource = [System.IO.File]::ReadAllText((Join-Path $repositoryRoot "tools/product-gate/hooks/watch-smoke.ps1"))
$probeIndex = $watchSmokeSource.IndexOf("Invoke-StrictUiReadinessProbe")
$readyIndex = $watchSmokeSource.IndexOf("`$ready = Wait-ForWatchReady")
if ($probeIndex -lt 0 -or $readyIndex -lt 0 -or $probeIndex -gt $readyIndex) {
    throw "The watch smoke must establish a subscribed browser session before awaiting watch.ready."
}
if ($watchSmokeSource -notmatch 'Test-IsWindowsPlatform' -or
    $watchSmokeSource -notmatch '"--shutdown-file"' -or
    $watchSmokeSource -notmatch '\[System\.IO\.File\]::WriteAllText\(\$shutdownPath, "stop"\)' -or
    $watchSmokeSource -notmatch 'Request-GracefulProductShutdown(?s:.*?)-RootProcessId \$process\.Id(?s:.*?)-RootStartTimeUtc \$rootStartTimeUtc') {
    throw "The watch smoke must use deterministic Windows shutdown and signal-based Unix shutdown."
}
if ($watchSmokeSource -notmatch 'Wait-ForRootProcessToExit -Process \$process -TimeoutSeconds 90') {
    throw "The watch smoke must qualify root-command exit without treating unrelated descendants as product failures."
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
                [ordered]@{ id = "compatibility.compile"; status = "PASS"; exit_code = 0 }
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
