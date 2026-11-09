param(
    [Parameter(Mandatory = $true)][string]$TestExecutable,
    [switch]$AllowDriverReset,
    [ValidateSet('SharedTexture', 'Video')][string]$SourceKind = 'SharedTexture',
    [switch]$UseCurrentToken,
    [ValidatePattern('^PCI\\VEN_10DE&[A-Z0-9_&]+\\[A-Z0-9&]+$')]
    [string]$RestartDeviceInstanceId
)

function Test-OwnedFixtureReady {
    param([AllowEmptyString()][string]$Text, [int]$ExpectedProcessId)
    # Cmdlet empty output can behave as an empty collection under -notmatch. Always
    # return a scalar Boolean; only the exact current process can authorize a reset.
    return [bool]([string]$Text -match "(?m)^READY_EXTERNAL_DEVICE_REMOVAL pid=${ExpectedProcessId}:")
}

# Default is a no-reset preflight. Supply the switch only after explicit operator approval.
$ErrorActionPreference = 'Stop'
if ($RestartDeviceInstanceId) {
    if (-not $AllowDriverReset) { throw 'Targeted device restart requires explicit reset authorization' }
    $targetDevice = Get-PnpDevice -PresentOnly -InstanceId $RestartDeviceInstanceId -ErrorAction Stop
    if ($targetDevice.Class -ne 'Display' -or $targetDevice.FriendlyName -notlike 'NVIDIA *' -or $targetDevice.Status -ne 'OK') {
        throw 'Target must be the exact present healthy NVIDIA display adapter'
    }
}
$repository = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$binary = (Resolve-Path -LiteralPath $TestExecutable).Path
$directory = Join-Path $repository ('target/m034-removal-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($directory)
$stdoutPath = Join-Path $directory 'stdout.log'
$stderrPath = Join-Path $directory 'stderr.log'
$testName = if ($SourceKind -eq 'Video') {
    if ($AllowDriverReset) {
        'media::rendering::tests::videos::device::hardware_video_recovery_after_genuine_device_removal'
    } else {
        'media::rendering::tests::videos::device::hardware_video_recovery_preflight_without_device_removal'
    }
} elseif ($AllowDriverReset) {
    'native::backend::tests::retirement::removal::hardware_genuine_device_removal_rebuilds_owned_plans_and_retires_unobserved_old_work'
} else {
    'native::backend::tests::retirement::removal::hardware_source_recovery_fixture_preflight_without_device_removal'
}
$metadata = [ordered]@{
    started_utc = [DateTime]::UtcNow.ToString('o')
    binary = $binary
    binary_sha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    reset_authorized = [bool]$AllowDriverReset
    source_kind = $SourceKind
    use_current_token = [bool]$UseCurrentToken
    restart_device_instance_id = $RestartDeviceInstanceId
    directory = $directory
}
$qualificationProcess = $null
try {
    if ($AllowDriverReset) { $env:GOLDEN_VIDEO_EXTERNAL_DEVICE_REMOVAL = '1' }
    $qualificationProcess = Start-Process -FilePath $binary -ArgumentList @(
        '--ignored', '--exact',
        $testName,
        '--nocapture', '--test-threads=1'
    ) -WorkingDirectory $repository -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
    # Retain the process handle immediately; otherwise PowerShell can lose a short-lived
    # process's exit code. A missing exit code must never become a successful qualification.
    $metadata.qualification_pid = $qualificationProcess.Id
    $retainedHandle = $qualificationProcess.Handle
    $expected = "(?m)^READY_EXTERNAL_DEVICE_REMOVAL pid=$($qualificationProcess.Id):"
    $readinessSeconds = if ($SourceKind -eq 'Video') { 45 } else { 20 }
    $deadline = [DateTime]::UtcNow.AddSeconds($readinessSeconds)
    while (-not (Test-OwnedFixtureReady `
        -Text ([string](Get-Content -LiteralPath $stderrPath -Raw -ErrorAction SilentlyContinue)) `
        -ExpectedProcessId $qualificationProcess.Id)) {
        if ($qualificationProcess.HasExited) {
            $finalLog = [string](Get-Content -LiteralPath $stderrPath -Raw)
            if (Test-OwnedFixtureReady -Text $finalLog -ExpectedProcessId $qualificationProcess.Id) { break }
            throw 'Fixture exited before its own readiness marker'
        }
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Readiness expired; reset was not triggered' }
        Start-Sleep -Milliseconds 50
    }
    $readiness = [regex]::Match([string](Get-Content -LiteralPath $stderrPath -Raw), $expected).Value
    if (-not $readiness) { throw 'Actual readiness marker is missing; reset was not triggered' }
    $metadata.readiness_marker = $readiness
    $metadata.ready_utc = [DateTime]::UtcNow.ToString('o')
    if ($AllowDriverReset) {
        if ($qualificationProcess.HasExited) { throw 'Ready fixture has already exited' }
        $triggerOptions = @{
            WindowStyle = 'Hidden'
            PassThru = $true
        }
        if ($UseCurrentToken) {
            # Redirection selects CreateProcess with the current token, so an elevation-only
            # executable fails rather than leaving an unattended UAC dialog open.
            $triggerOptions.RedirectStandardOutput = Join-Path $directory 'trigger.stdout.log'
            $triggerOptions.RedirectStandardError = Join-Path $directory 'trigger.stderr.log'
        } else {
            $triggerOptions.Verb = 'RunAs'
        }
        if ($RestartDeviceInstanceId) {
            # Restart one verified adapter. No class-wide, persistent-disable or reboot flag.
            $triggerOptions.FilePath = Join-Path $env:SystemRoot 'System32/pnputil.exe'
            $triggerOptions.ArgumentList = @('/restart-device', ('"' + $RestartDeviceInstanceId + '"'))
        } else {
            $triggerOptions.FilePath = Join-Path $env:SystemRoot 'System32/DXCap.exe'
            $triggerOptions.ArgumentList = '-forcetdr'
        }
        $resetProcess = Start-Process @triggerOptions
        $retainedResetHandle = $resetProcess.Handle
        $resetProcess.WaitForExit()
        if ($RestartDeviceInstanceId) {
            $metadata.pnputil_exit_code = $resetProcess.ExitCode
            $restored = Get-PnpDevice -PresentOnly -InstanceId $RestartDeviceInstanceId -ErrorAction Stop
            $metadata.device_status_after = [string]$restored.Status
            if ($restored.Status -ne 'OK') { throw 'Restarted NVIDIA adapter is not healthy' }
        } else {
            $metadata.dxcap_exit_code = $resetProcess.ExitCode
        }
        if ($resetProcess.ExitCode -ne 0) { throw 'Driver trigger failed; no successful trigger claimed' }
    }
    $qualificationProcess.WaitForExit()
    $metadata.qualification_exit_code = $qualificationProcess.ExitCode
    if ($null -eq $qualificationProcess.ExitCode) { throw 'Missing authoritative fixture exit code' }
    if ($qualificationProcess.ExitCode -ne 0) { throw "Native qualification failed: $($qualificationProcess.ExitCode)" }
    if ((Get-Content -LiteralPath $stdoutPath -Raw) -notmatch '1 passed; 0 failed') {
        throw 'Expected native test did not pass'
    }
} catch {
    $metadata.error = $_.Exception.Message
    if ($qualificationProcess -and -not $qualificationProcess.HasExited) {
        # The fixture has its own bounded removal deadline. Preserve the retained handle
        # through cleanup even when the external trigger fails.
        [void]$qualificationProcess.WaitForExit(45000)
    }
    throw
} finally {
    $metadata.finished_utc = [DateTime]::UtcNow.ToString('o')
    if ($qualificationProcess) {
        $metadata.fixture_terminal = $qualificationProcess.HasExited
        if ($qualificationProcess.HasExited) {
            $metadata.qualification_exit_code = $qualificationProcess.ExitCode
            $qualificationProcess.Dispose()
        }
    }
    $metadata | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $directory 'result.json')
    Write-Output $directory
}
