[CmdletBinding()]
param(
    [ValidateRange(10, 600)]
    [int] $TimeoutSeconds = 180
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scenario = "osc-loopback.v1"
$expectedDigest = "fnv1a64:9da80781af2c7655"
$expectedReloadDigest = "fnv1a64:78a1a93d927b4a39"
$repositoryRoot = if ([string]::IsNullOrWhiteSpace($env:PRODUCT_GATE_REPOSITORY_ROOT)) {
    [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
}
else {
    [System.IO.Path]::GetFullPath($env:PRODUCT_GATE_REPOSITORY_ROOT)
}
$runDirectory = if ([string]::IsNullOrWhiteSpace($env:PRODUCT_GATE_RUN_DIRECTORY)) {
    Join-Path $repositoryRoot "target\product-gate\module-loopback-standalone"
}
else {
    [System.IO.Path]::GetFullPath($env:PRODUCT_GATE_RUN_DIRECTORY)
}
$artifactDirectory = Join-Path $runDirectory "evidence\module-loopback"
$stdoutPath = Join-Path $artifactDirectory "stdout.log"
$stderrPath = Join-Path $artifactDirectory "stderr.log"
$verificationPath = Join-Path $artifactDirectory "verification.json"
New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null

$beforeProcessIds = @(Get-Process -Name "Chataigne2" -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
$process = $null
$stdout = ""
$stderr = ""
$result = $null
$failure = $null
$cleanup = [ordered]@{
    cargo_process_exited = $false
    new_chataigne_processes = @()
    forced_process_ids = @()
}

function Assert-Equal {
    param(
        $Actual,
        $Expected,
        [string] $Field
    )
    if ($Actual -ne $Expected) {
        throw "Evidence field '$Field' mismatch: expected '$Expected', got '$Actual'."
    }
}

function Stop-OwnedProcess {
    param([System.Diagnostics.Process] $OwnedProcess)

    if ($null -eq $OwnedProcess -or $OwnedProcess.HasExited) {
        return
    }
    try {
        $OwnedProcess.Kill($true)
    }
    catch {
        Stop-Process -Id $OwnedProcess.Id -Force -ErrorAction SilentlyContinue
    }
    $OwnedProcess.WaitForExit(5000) | Out-Null
}

try {
    $cargo = Get-Command "cargo" -ErrorAction Stop | Select-Object -First 1
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $cargo.Source
    $startInfo.Arguments = "run -- --product-evidence $scenario"
    $startInfo.WorkingDirectory = $repositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "Failed to start literal product-evidence command."
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-OwnedProcess -OwnedProcess $process
        throw "Product-evidence command exceeded the ${TimeoutSeconds}s timeout."
    }
    $process.WaitForExit()
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    $cleanup.cargo_process_exited = $process.HasExited

    if ($process.ExitCode -ne 0) {
        throw "Product-evidence command exited with code $($process.ExitCode)."
    }

    $machineResults = @()
    foreach ($line in @($stdout -split "`r?`n")) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        try {
            $candidate = $line | ConvertFrom-Json
            if ($candidate.event -eq "chataigne.product_evidence.result") {
                $machineResults += $candidate
            }
        }
        catch {
            # Production logger lines are allowed; only the named JSON event is evidence.
        }
    }
    if ($machineResults.Count -ne 1) {
        throw "Expected exactly one schema-v1 product evidence result, found $($machineResults.Count)."
    }
    $result = $machineResults[0]

    Assert-Equal $result.schema_version 1 "schema_version"
    Assert-Equal $result.scenario $scenario "scenario"
    Assert-Equal $result.status "pass" "status"
    Assert-Equal $result.semantic_digest $expectedDigest "semantic_digest"
    Assert-Equal $result.evidence.command_creation_ack $true "evidence.command_creation_ack"
    Assert-Equal $result.evidence.input.address "/evidence/input" "evidence.input.address"
    Assert-Equal $result.evidence.input.value 42 "evidence.input.value"
    Assert-Equal $result.evidence.save_reload.semantic_digest $expectedReloadDigest "evidence.save_reload.semantic_digest"
    Assert-Equal $result.evidence.save_reload.state.command_address "/evidence/output/2" "save_reload.command_address"
    Assert-Equal $result.evidence.save_reload.state.input_value 42 "save_reload.input_value"
    $effectOrder = @($result.evidence.effect_order)
    Assert-Equal $effectOrder.Count 2 "evidence.effect_order.count"
    Assert-Equal $effectOrder[0] "/evidence/output/1" "evidence.effect_order[0]"
    Assert-Equal $effectOrder[1] "/evidence/output/2" "evidence.effect_order[1]"
}
catch {
    $failure = $_.Exception.Message
}
finally {
    Stop-OwnedProcess -OwnedProcess $process
    if ($null -ne $process) {
        $cleanup.cargo_process_exited = $process.HasExited
    }
    Start-Sleep -Milliseconds 250
    $newProcesses = @(
        Get-Process -Name "Chataigne2" -ErrorAction SilentlyContinue |
            Where-Object { $beforeProcessIds -notcontains $_.Id }
    )
    $cleanup.new_chataigne_processes = @($newProcesses | ForEach-Object { $_.Id })
    foreach ($leftover in $newProcesses) {
        $cleanup.forced_process_ids += $leftover.Id
        Stop-Process -Id $leftover.Id -Force -ErrorAction SilentlyContinue
    }
    if ($newProcesses.Count -gt 0) {
        $cleanupFailure = "Product-evidence command leaked Chataigne process IDs: $($cleanup.new_chataigne_processes -join ', ')."
        $failure = if ([string]::IsNullOrWhiteSpace($failure)) { $cleanupFailure } else { "$failure $cleanupFailure" }
    }
    [System.IO.File]::WriteAllText($stdoutPath, $stdout, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($stderrPath, $stderr, [System.Text.UTF8Encoding]::new($false))
}

$verification = [ordered]@{
    schema_version = 1
    hook = "module-loopback-smoke"
    commit = $env:PRODUCT_GATE_COMMIT_SHA
    command = "cargo run -- --product-evidence $scenario"
    status = if ([string]::IsNullOrWhiteSpace($failure)) { "PASS" } else { "FAIL" }
    expected = [ordered]@{
        scenario = $scenario
        semantic_digest = $expectedDigest
        save_reload_digest = $expectedReloadDigest
        effect_order = @("/evidence/output/1", "/evidence/output/2")
    }
    result = $result
    cleanup = $cleanup
    failure = $failure
}
[System.IO.File]::WriteAllText(
    $verificationPath,
    ($verification | ConvertTo-Json -Depth 12),
    [System.Text.UTF8Encoding]::new($false)
)

if (-not [string]::IsNullOrWhiteSpace($stdout)) {
    Write-Host $stdout.TrimEnd()
}
if (-not [string]::IsNullOrWhiteSpace($stderr)) {
    Write-Host $stderr.TrimEnd()
}
if (-not [string]::IsNullOrWhiteSpace($failure)) {
    Write-Error $failure
    exit 1
}

Write-Host "Verified schema-v1 OSC module loopback evidence and clean process teardown."
exit 0
