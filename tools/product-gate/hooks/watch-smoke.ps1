. (Join-Path $PSScriptRoot "smoke-common.ps1")

function Wait-ForWatchReady {
    param(
        [System.Diagnostics.Process]$Process,
        [string]$StandardOutputPath,
        [datetime]$Deadline
    )

    while ((Get-Date) -lt $Deadline) {
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "cargo xtask watch exited before watch.ready with code $($Process.ExitCode)."
        }
        if (Test-Path -LiteralPath $StandardOutputPath) {
            foreach ($line in Get-Content -LiteralPath $StandardOutputPath) {
                if ([string]::IsNullOrWhiteSpace($line)) {
                    continue
                }
                try {
                    $event = $line | ConvertFrom-Json
                }
                catch {
                    continue
                }
                if ($event.event -ne "watch.ready") {
                    continue
                }
                if ($event.version -ne 2) {
                    throw "watch.ready used unsupported version '$($event.version)'."
                }
                if ($event.backend.state -ne "ready" -or $event.frontend.state -ne "ready" -or
                    $event.engine.state -ne "ready" -or $event.session.state -ne "ready") {
                    throw "watch.ready did not report every required readiness plane."
                }
                if ([int64]$event.session.active_subscribed_websocket_clients -lt 1) {
                    throw "watch.ready reported no subscribed UI WebSocket session."
                }
                if ($null -eq $event.engine.read_model_revision) {
                    throw "watch.ready omitted the immutable read-model revision."
                }
                if ([int]$event.ports.frontend -ne 5173 -or [int]$event.ports.backend -ne 7010) {
                    throw "watch.ready reported unexpected frontend/backend ports."
                }
                return $event
            }
        }
        Start-Sleep -Milliseconds 250
    }
    throw "cargo xtask watch did not emit a valid watch.ready event before timeout."
}

$repositoryRoot = Get-ProductGateRepositoryRoot
$runDirectory = Get-ProductGateRunDirectory -RepositoryRoot $repositoryRoot
$timeoutSeconds = Get-SmokeTimeoutSeconds
$standardOutputPath = Join-Path $runDirectory "watch-smoke.stdout.log"
$standardErrorPath = Join-Path $runDirectory "watch-smoke.stderr.log"
$screenshotPath = Join-Path $runDirectory "watch-smoke.ready.png"
$shutdownPath = Join-Path $runDirectory "watch-smoke.shutdown"
$cargo = Get-CommandSource -Name "cargo"
$ports = @(5173, 7010)
$runningOnWindows = Test-IsWindowsPlatform

Assert-LoopbackPortsAvailable -Ports $ports
if (Test-Path -LiteralPath $shutdownPath) {
    Remove-Item -LiteralPath $shutdownPath -Force
}

$watchArguments = @("xtask", "watch")
if ($runningOnWindows) {
    $watchArguments += @("--shutdown-file", $shutdownPath)
}
$startParameters = @{
    FilePath               = $cargo
    ArgumentList           = $watchArguments
    WorkingDirectory       = $repositoryRoot
    RedirectStandardOutput = $standardOutputPath
    RedirectStandardError  = $standardErrorPath
    PassThru               = $true
}
if ($runningOnWindows) {
    $startParameters.WindowStyle = "Hidden"
}

$process = $null
$ownedProcessIds = @()
$passed = $false
try {
    $process = Start-Process @startParameters
    $rootStartTimeUtc = $process.StartTime.ToUniversalTime()
    $readinessDeadline = (Get-Date).AddSeconds($timeoutSeconds)
    Wait-ForFrontendDocument `
        -Uri "http://127.0.0.1:5173/" `
        -Process $process `
        -Deadline $readinessDeadline
    Wait-ForFrontendDocument `
        -Uri "http://127.0.0.1:7010/api/ui/health" `
        -Process $process `
        -Deadline $readinessDeadline
    Invoke-StrictUiReadinessProbe `
        -RepositoryRoot $repositoryRoot `
        -FrontendUri "http://127.0.0.1:5173/" `
        -ScreenshotPath $screenshotPath `
        -TimeoutSeconds ([Math]::Min($timeoutSeconds, 90)) `
        -Attempts 3

    $ready = Wait-ForWatchReady `
        -Process $process `
        -StandardOutputPath $standardOutputPath `
        -Deadline $readinessDeadline

    $ownedProcessIds = @(Get-OwnedProcessIds `
            -RootProcessId $process.Id `
            -RootStartTimeUtc $rootStartTimeUtc)
    if ($runningOnWindows) {
        [System.IO.File]::WriteAllText($shutdownPath, "stop")
    }
    else {
        Request-GracefulProductShutdown `
            -RootProcessId $process.Id `
            -RootStartTimeUtc $rootStartTimeUtc
    }
    Wait-ForOwnedProcessesToExit -ProcessIds $ownedProcessIds -TimeoutSeconds 90
    Wait-ForPortsReleased -Ports $ports -TimeoutSeconds 10

    [pscustomobject]@{
        contract          = "product-gate-watch-ready-v1"
        command           = "cargo xtask watch"
        watch_ready       = $ready
        browser_probe     = "passed"
        graceful_shutdown = "verified"
        released_ports    = $ports
        screenshot        = $screenshotPath
    } | ConvertTo-Json -Depth 8 -Compress | Write-Host
    $passed = $true
}
catch {
    Write-SmokeLogs -StandardOutputPath $standardOutputPath -StandardErrorPath $standardErrorPath
    throw
}
finally {
    if ($null -ne $process -and -not $passed) {
        try {
            $ownedProcessIds = @(
                $ownedProcessIds +
                @(Get-OwnedProcessIds `
                        -RootProcessId $process.Id `
                        -RootStartTimeUtc $rootStartTimeUtc) |
                    Sort-Object -Unique
            )
            if ($runningOnWindows) {
                [System.IO.File]::WriteAllText($shutdownPath, "stop")
            }
            else {
                Request-GracefulProductShutdown `
                    -RootProcessId $process.Id `
                    -RootStartTimeUtc $rootStartTimeUtc
            }
            Wait-ForOwnedProcessesToExit -ProcessIds $ownedProcessIds -TimeoutSeconds 5
        }
        catch {
        }
        $cleanupIds = @($ownedProcessIds)
        [array]::Reverse($cleanupIds)
        foreach ($processId in $cleanupIds) {
            Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
        }
        Stop-OwnedProcessTree -RootProcessId $process.Id
        try {
            Wait-ForPortsReleased -Ports $ports -TimeoutSeconds 10
        }
        catch {
            Write-Warning $_
        }
    }
}
