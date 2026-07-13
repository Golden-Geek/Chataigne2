Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-IsWindowsPlatform {
    return [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows
    )
}

function Get-ProductGateRepositoryRoot {
    if (-not [string]::IsNullOrWhiteSpace($env:PRODUCT_GATE_REPOSITORY_ROOT)) {
        return [System.IO.Path]::GetFullPath($env:PRODUCT_GATE_REPOSITORY_ROOT)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
}

function Get-ProductGateRunDirectory {
    param([string]$RepositoryRoot)

    $directory = $env:PRODUCT_GATE_RUN_DIRECTORY
    if ([string]::IsNullOrWhiteSpace($directory)) {
        $directory = Join-Path $RepositoryRoot "target\product-gate\local-hooks"
    }
    $directory = [System.IO.Path]::GetFullPath($directory)
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    return $directory
}

function Get-SmokeTimeoutSeconds {
    $timeoutSeconds = 240
    if (-not [string]::IsNullOrWhiteSpace($env:PRODUCT_GATE_SMOKE_STARTUP_TIMEOUT_SECONDS)) {
        if (-not [int]::TryParse($env:PRODUCT_GATE_SMOKE_STARTUP_TIMEOUT_SECONDS, [ref]$timeoutSeconds) -or
            $timeoutSeconds -le 0) {
            throw "PRODUCT_GATE_SMOKE_STARTUP_TIMEOUT_SECONDS must be a positive integer."
        }
    }
    return $timeoutSeconds
}

function Get-CommandSource {
    param([string]$Name)

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        throw "Required command '$Name' was not found on PATH."
    }
    if (-not [string]::IsNullOrWhiteSpace($command.Source)) {
        return $command.Source
    }
    if (-not [string]::IsNullOrWhiteSpace($command.Path)) {
        return $command.Path
    }
    return $command.Name
}

function Test-LoopbackPortAvailable {
    param([int]$Port)

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
    try {
        $listener.Start()
        return $true
    }
    catch {
        return $false
    }
    finally {
        $listener.Stop()
    }
}

function Assert-LoopbackPortsAvailable {
    param([int[]]$Ports)

    foreach ($port in $Ports) {
        if (-not (Test-LoopbackPortAvailable -Port $port)) {
            throw "Required smoke port 127.0.0.1:$port is already occupied; refusing to attach to or terminate an unrelated process."
        }
    }
}

function Wait-ForTcpListener {
    param(
        [string]$Address,
        [int]$Port,
        [System.Diagnostics.Process]$Process,
        [datetime]$Deadline
    )

    while ((Get-Date) -lt $Deadline) {
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "The root command exited before backend readiness with code $($Process.ExitCode)."
        }
        $client = [System.Net.Sockets.TcpClient]::new()
        try {
            $connect = $client.ConnectAsync($Address, $Port)
            if ($connect.Wait(500) -and $client.Connected) {
                return
            }
        }
        catch {
        }
        finally {
            $client.Dispose()
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Backend listener 127.0.0.1:$Port did not become reachable before the smoke timeout."
}

function Wait-ForFrontendDocument {
    param(
        [string]$Uri,
        [System.Diagnostics.Process]$Process,
        [datetime]$Deadline
    )

    while ((Get-Date) -lt $Deadline) {
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "The root command exited before frontend readiness with code $($Process.ExitCode)."
        }
        try {
            $response = Invoke-WebRequest -Uri $Uri -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -ge 200 -and $response.StatusCode -lt 300 -and
                -not [string]::IsNullOrWhiteSpace($response.Content)) {
                return
            }
        }
        catch {
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Frontend document '$Uri' did not become available before the smoke timeout."
}

function Get-OwnedProcessIds {
    param([int]$RootProcessId)

    if (-not (Test-IsWindowsPlatform)) {
        $rows = @(& ps -A -o pid= -o ppid= 2>$null)
        $processes = foreach ($row in $rows) {
            if ($row -match '^\s*(\d+)\s+(\d+)\s*$') {
                [pscustomobject]@{ ProcessId = [int]$Matches[1]; ParentProcessId = [int]$Matches[2] }
            }
        }
    }
    else {
        $processes = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
    }

    $owned = [System.Collections.Generic.HashSet[int]]::new()
    [void]$owned.Add($RootProcessId)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($process in $processes) {
            if ($owned.Contains([int]$process.ParentProcessId) -and
                $owned.Add([int]$process.ProcessId)) {
                $changed = $true
            }
        }
    }
    return @($owned)
}

function Test-AnyProcessAlive {
    param([System.Diagnostics.Process[]]$Processes)

    foreach ($process in $Processes) {
        try {
            if (-not $process.HasExited) {
                return $true
            }
        }
        catch {
        }
    }
    return $false
}

function Request-GracefulProductShutdown {
    param([int]$RootProcessId)

    if (-not (Test-IsWindowsPlatform)) {
        $ownedProcessIds = @(Get-OwnedProcessIds -RootProcessId $RootProcessId)
        $shutdownOrder = @(
            $ownedProcessIds | Where-Object { $_ -ne $RootProcessId }
            $RootProcessId
        )
        $requested = $false
        foreach ($processId in $shutdownOrder) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                continue
            }
            & /bin/kill -TERM $processId 2>$null
            if ($LASTEXITCODE -ne 0 -and
                $null -ne (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "SIGTERM failed for owned process $processId."
            }
            $requested = $true
        }
        if (-not $requested) {
            throw "No owned process accepted a graceful SIGTERM shutdown request."
        }
        return
    }

    $requested = $false
    foreach ($processId in (Get-OwnedProcessIds -RootProcessId $RootProcessId)) {
        if ($processId -eq $RootProcessId) {
            continue
        }
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if ($null -eq $process) {
            continue
        }
        try {
            if ($process.MainWindowHandle -ne [IntPtr]::Zero -and $process.CloseMainWindow()) {
                $requested = $true
            }
        }
        catch {
        }
    }
    if (-not $requested) {
        throw "No owned Tauri window accepted a graceful close request."
    }
}

function Wait-ForOwnedProcessesToExit {
    param(
        [int[]]$ProcessIds,
        [int]$TimeoutSeconds
    )

    # Keep handles to the exact processes that were owned at shutdown time. Looking
    # them up by PID on every poll can mistake a later process that reused the PID
    # for an orphan from the product tree.
    $trackedProcesses = @(
        $ProcessIds | ForEach-Object {
            Get-Process -Id $_ -ErrorAction SilentlyContinue
        }
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (-not (Test-AnyProcessAlive -Processes $trackedProcesses)) {
            return
        }
        Start-Sleep -Milliseconds 250
    }
    $remaining = @(
        $trackedProcesses | Where-Object {
            try {
                -not $_.HasExited
            }
            catch {
                $false
            }
        } | ForEach-Object { "$($_.ProcessName) ($($_.Id))" }
    )
    throw "Owned processes did not exit after graceful shutdown: $($remaining -join ', ')."
}

function Wait-ForPortsReleased {
    param(
        [int[]]$Ports,
        [int]$TimeoutSeconds
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $occupied = @($Ports | Where-Object { -not (Test-LoopbackPortAvailable -Port $_) })
        if ($occupied.Count -eq 0) {
            return
        }
        Start-Sleep -Milliseconds 250
    }
    $remaining = @($Ports | Where-Object { -not (Test-LoopbackPortAvailable -Port $_) })
    throw "Smoke ports remained occupied after shutdown: $($remaining -join ', ')."
}

function Stop-OwnedProcessTree {
    param([int]$RootProcessId)

    $owned = @(Get-OwnedProcessIds -RootProcessId $RootProcessId)
    [array]::Reverse($owned)
    foreach ($processId in $owned) {
        Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    }
}

function Write-SmokeLogs {
    param(
        [string]$StandardOutputPath,
        [string]$StandardErrorPath
    )

    foreach ($entry in @(
            [pscustomobject]@{ Label = "root command stdout"; Path = $StandardOutputPath },
            [pscustomobject]@{ Label = "root command stderr"; Path = $StandardErrorPath }
        )) {
        Write-Host "--- $($entry.Label): $($entry.Path)"
        if (Test-Path -LiteralPath $entry.Path) {
            Get-Content -LiteralPath $entry.Path -Tail 120
        }
        else {
            Write-Host "<not created>"
        }
    }
}

function Invoke-StrictUiReadinessProbe {
    param(
        [string]$RepositoryRoot,
        [string]$FrontendUri,
        [string]$ScreenshotPath,
        [int]$TimeoutSeconds,
        [int]$Attempts = 1
    )

    $node = Get-CommandSource -Name "node"
    $probe = Join-Path $PSScriptRoot "strict-ui-readiness.mjs"
    $exitCode = 1
    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        & $node $probe `
            --repository-root $RepositoryRoot `
            --url $FrontendUri `
            --screenshot $ScreenshotPath `
            --timeout-ms ($TimeoutSeconds * 1000)
        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            return
        }
        if ($attempt -lt $Attempts) {
            Start-Sleep -Seconds 2
        }
    }
    throw "The strict mounted-UI readiness probe failed with exit code $exitCode after $Attempts attempt(s)."
}

function Invoke-RootCommandSmoke {
    param(
        [string]$Id,
        [string[]]$CargoArguments,
        [string]$FrontendUri,
        [int[]]$Ports,
        [string]$ShutdownFile = ""
    )

    $repositoryRoot = Get-ProductGateRepositoryRoot
    $runDirectory = Get-ProductGateRunDirectory -RepositoryRoot $repositoryRoot
    $timeoutSeconds = Get-SmokeTimeoutSeconds
    $standardOutputPath = Join-Path $runDirectory "$Id.stdout.log"
    $standardErrorPath = Join-Path $runDirectory "$Id.stderr.log"
    $screenshotPath = Join-Path $runDirectory "$Id.ready.png"
    $cargo = Get-CommandSource -Name "cargo"
    Get-CommandSource -Name "node" | Out-Null

    Assert-LoopbackPortsAvailable -Ports $Ports
    if (-not [string]::IsNullOrWhiteSpace($ShutdownFile) -and (Test-Path -LiteralPath $ShutdownFile)) {
        Remove-Item -LiteralPath $ShutdownFile -Force
    }

    $previousBind = $env:GC_UI_BIND
    $env:GC_UI_BIND = "127.0.0.1:7010"
    $startParameters = @{
        FilePath               = $cargo
        ArgumentList           = $CargoArguments
        WorkingDirectory       = $repositoryRoot
        RedirectStandardOutput = $standardOutputPath
        RedirectStandardError  = $standardErrorPath
        PassThru               = $true
    }
    if (Test-IsWindowsPlatform) {
        $startParameters.WindowStyle = "Hidden"
    }

    $process = $null
    $ownedProcessIds = @()
    $passed = $false
    try {
        $process = Start-Process @startParameters
        $deadline = (Get-Date).AddSeconds($timeoutSeconds)
        Wait-ForTcpListener -Address "127.0.0.1" -Port 7010 -Process $process -Deadline $deadline
        Wait-ForFrontendDocument -Uri $FrontendUri -Process $process -Deadline $deadline
        Invoke-StrictUiReadinessProbe `
            -RepositoryRoot $repositoryRoot `
            -FrontendUri $FrontendUri `
            -ScreenshotPath $screenshotPath `
            -TimeoutSeconds ([Math]::Min($timeoutSeconds, 90))

        $ownedProcessIds = @(Get-OwnedProcessIds -RootProcessId $process.Id)
        if ([string]::IsNullOrWhiteSpace($ShutdownFile)) {
            Request-GracefulProductShutdown -RootProcessId $process.Id
        }
        else {
            [System.IO.File]::WriteAllText($ShutdownFile, "stop")
        }
        Wait-ForOwnedProcessesToExit -ProcessIds $ownedProcessIds -TimeoutSeconds 20
        Wait-ForPortsReleased -Ports $Ports -TimeoutSeconds 10

        [pscustomobject]@{
            contract          = "product-gate-root-command-ready-v1"
            command           = "cargo $($CargoArguments -join ' ')"
            backend           = "ready"
            frontend          = "ready"
            engine_connection = "ready"
            graceful_shutdown = "verified"
            released_ports    = $Ports
            screenshot        = $screenshotPath
        } | ConvertTo-Json -Compress | Write-Host
        $passed = $true
    }
    catch {
        Write-SmokeLogs -StandardOutputPath $standardOutputPath -StandardErrorPath $standardErrorPath
        throw
    }
    finally {
        $env:GC_UI_BIND = $previousBind
        if ($null -ne $process -and -not $passed) {
            try {
                $ownedProcessIds = @(
                    $ownedProcessIds +
                    @(Get-OwnedProcessIds -RootProcessId $process.Id) |
                        Sort-Object -Unique
                )
                if ([string]::IsNullOrWhiteSpace($ShutdownFile)) {
                    Request-GracefulProductShutdown -RootProcessId $process.Id
                }
                else {
                    [System.IO.File]::WriteAllText($ShutdownFile, "stop")
                }
                Wait-ForOwnedProcessesToExit -ProcessIds $ownedProcessIds -TimeoutSeconds 5
            }
            catch {
                $cleanupIds = @($ownedProcessIds)
                [array]::Reverse($cleanupIds)
                foreach ($processId in $cleanupIds) {
                    Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
                }
                Stop-OwnedProcessTree -RootProcessId $process.Id
            }
            try {
                Wait-ForPortsReleased -Ports $Ports -TimeoutSeconds 10
            }
            catch {
                Write-Warning $_
            }
        }
    }
}
