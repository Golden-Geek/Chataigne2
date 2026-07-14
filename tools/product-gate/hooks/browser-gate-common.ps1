. (Join-Path $PSScriptRoot "smoke-common.ps1")

function Test-AllInterfacePortAvailable {
    param([int]$Port)

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Any, $Port)
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

function Get-BundledProductBinary {
    param([string]$RepositoryRoot)

    $names = if (Test-IsWindowsPlatform) {
        @("Chataigne2.exe", "chataigne2.exe")
    }
    else {
        @("Chataigne2", "chataigne2")
    }
    foreach ($name in $names) {
        $candidate = Join-Path $RepositoryRoot "target/debug/$name"
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return [System.IO.Path]::GetFullPath($candidate)
        }
    }
    throw "The bundled Chataigne2 debug binary is missing. The product gate build/smoke dependency must run first."
}

function Get-RealNonLoopbackIpv4Address {
    $candidates = foreach ($networkInterface in [System.Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces()) {
        if ($networkInterface.OperationalStatus -ne [System.Net.NetworkInformation.OperationalStatus]::Up -or
            $networkInterface.NetworkInterfaceType -eq [System.Net.NetworkInformation.NetworkInterfaceType]::Loopback -or
            $networkInterface.NetworkInterfaceType -eq [System.Net.NetworkInformation.NetworkInterfaceType]::Tunnel) {
            continue
        }

        $properties = $networkInterface.GetIPProperties()
        $hasIpv4Gateway = @(
            $properties.GatewayAddresses | Where-Object {
                $_.Address.AddressFamily -eq [System.Net.Sockets.AddressFamily]::InterNetwork -and
                -not $_.Address.Equals([System.Net.IPAddress]::Any)
            }
        ).Count -gt 0
        foreach ($unicast in $properties.UnicastAddresses) {
            $address = $unicast.Address
            if ($address.AddressFamily -ne [System.Net.Sockets.AddressFamily]::InterNetwork -or
                [System.Net.IPAddress]::IsLoopback($address) -or
                $address.Equals([System.Net.IPAddress]::Any) -or
                $address.ToString().StartsWith("169.254.")) {
                continue
            }
            [pscustomobject]@{
                Address      = $address.ToString()
                GatewayRank  = if ($hasIpv4Gateway) { 0 } else { 1 }
                Interface    = $networkInterface.Name
                InterfaceId  = $networkInterface.Id
            }
        }
    }

    $selected = @($candidates | Sort-Object GatewayRank, Address | Select-Object -First 1)
    if ($selected.Count -eq 0) {
        throw "No active real non-loopback IPv4 address is available for the LAN browser gate."
    }
    return $selected[0]
}

function Remove-VerifiedRunSubdirectory {
    param(
        [string]$RunDirectory,
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    $runRoot = [System.IO.Path]::GetFullPath($RunDirectory).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $target = [System.IO.Path]::GetFullPath($Path).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $comparison = if (Test-IsWindowsPlatform) {
        [System.StringComparison]::OrdinalIgnoreCase
    }
    else {
        [System.StringComparison]::Ordinal
    }
    $prefix = $runRoot + [System.IO.Path]::DirectorySeparatorChar
    if ($target.Equals($runRoot, $comparison) -or -not $target.StartsWith($prefix, $comparison)) {
        throw "Refusing to recursively remove '$target' because it is not a child of '$runRoot'."
    }
    Remove-Item -LiteralPath $target -Recurse -Force
}

function Remove-ReportedUploadedProject {
    param(
        [string]$ReportedPath,
        [string]$ExpectedFileName
    )

    if ([string]::IsNullOrWhiteSpace($ReportedPath)) {
        return "not_reported"
    }
    if (-not [System.IO.Path]::IsPathRooted($ReportedPath)) {
        return "runtime_managed_relative_path"
    }
    $fullPath = [System.IO.Path]::GetFullPath($ReportedPath)
    if ([System.IO.Path]::GetFileName($fullPath) -ne $ExpectedFileName) {
        throw "Refusing to remove reported project '$fullPath': filename does not match '$ExpectedFileName'."
    }
    if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
        Remove-Item -LiteralPath $fullPath -Force
        return "removed"
    }
    return "already_absent"
}

function Start-IsolatedBundledProduct {
    param(
        [string]$Binary,
        [string]$RepositoryRoot,
        [string]$Bind,
        [string]$SandboxDirectory,
        [string]$StandardOutputPath,
        [string]$StandardErrorPath
    )

    $environmentNames = @(
        "GC_UI_BIND",
        "GC_UI_FRONTEND_URL",
        "APPDATA",
        "LOCALAPPDATA",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME"
    )
    $previous = @{}
    foreach ($name in $environmentNames) {
        $previous[$name] = [System.Environment]::GetEnvironmentVariable($name, "Process")
    }

    $appData = Join-Path $SandboxDirectory "appdata"
    $localAppData = Join-Path $SandboxDirectory "local-appdata"
    $xdgConfig = Join-Path $SandboxDirectory "xdg-config"
    $xdgData = Join-Path $SandboxDirectory "xdg-data"
    foreach ($directory in @($appData, $localAppData, $xdgConfig, $xdgData)) {
        [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    }

    try {
        $env:GC_UI_BIND = $Bind
        Remove-Item Env:GC_UI_FRONTEND_URL -ErrorAction SilentlyContinue
        $env:APPDATA = $appData
        $env:LOCALAPPDATA = $localAppData
        $env:XDG_CONFIG_HOME = $xdgConfig
        $env:XDG_DATA_HOME = $xdgData

        $startParameters = @{
            FilePath               = $Binary
            ArgumentList           = @("--headless")
            WorkingDirectory       = $RepositoryRoot
            RedirectStandardOutput = $StandardOutputPath
            RedirectStandardError  = $StandardErrorPath
            PassThru               = $true
        }
        if (Test-IsWindowsPlatform) {
            $startParameters.WindowStyle = "Hidden"
        }
        return Start-Process @startParameters
    }
    finally {
        foreach ($name in $environmentNames) {
            [System.Environment]::SetEnvironmentVariable($name, $previous[$name], "Process")
        }
    }
}

function Invoke-BundledBrowserGate {
    param(
        [string]$Id,
        [ValidateSet("product-gate-workflow", "product-gate-lan")]
        [string]$BrowserCommand,
        [string]$BindAddress,
        [string]$BrowserHost,
        [int]$Port,
        [string]$FixtureFileName,
        [string]$ExpectedHost = ""
    )

    $repositoryRoot = Get-ProductGateRepositoryRoot
    $runDirectory = Get-ProductGateRunDirectory -RepositoryRoot $repositoryRoot
    $timeoutSeconds = Get-SmokeTimeoutSeconds
    $binary = Get-BundledProductBinary -RepositoryRoot $repositoryRoot
    $node = Get-CommandSource -Name "node"
    $browserScript = Join-Path $repositoryRoot "apps/chataigne/ui/scripts/ui-browser-tools.mjs"
    $sourceFixture = Join-Path $repositoryRoot "apps/chataigne/test-samples/test_simple_load.noisette"
    if (-not (Test-Path -LiteralPath $sourceFixture -PathType Leaf)) {
        throw "Representative product fixture '$sourceFixture' is missing."
    }

    $artifactDirectory = Join-Path $runDirectory "$Id-artifacts"
    $sandboxDirectory = Join-Path $runDirectory "$Id-sandbox"
    $reportPath = Join-Path $artifactDirectory "$Id.browser-report.json"
    $standardOutputPath = Join-Path $runDirectory "$Id.product.stdout.log"
    $standardErrorPath = Join-Path $runDirectory "$Id.product.stderr.log"
    [System.IO.Directory]::CreateDirectory($artifactDirectory) | Out-Null
    Remove-VerifiedRunSubdirectory -RunDirectory $runDirectory -Path $sandboxDirectory
    [System.IO.Directory]::CreateDirectory($sandboxDirectory) | Out-Null
    $fixturePath = Join-Path $sandboxDirectory $FixtureFileName
    Copy-Item -LiteralPath $sourceFixture -Destination $fixturePath -Force

    if ($BindAddress -eq "0.0.0.0") {
        if (-not (Test-AllInterfacePortAvailable -Port $Port)) {
            throw "Required all-interface product gate port $Port is occupied."
        }
    }
    else {
        Assert-LoopbackPortsAvailable -Ports @($Port)
    }

    $frontendUri = "http://${BrowserHost}:$Port/"
    $process = $null
    $rootStartTimeUtc = [datetime]::MinValue
    $ownedProcessIds = @()
    $reportedProjectPath = ""
    $uploadedProjectCleanup = "not_attempted"
    $passed = $false
    try {
        $process = Start-IsolatedBundledProduct `
            -Binary $binary `
            -RepositoryRoot $repositoryRoot `
            -Bind "${BindAddress}:$Port" `
            -SandboxDirectory $sandboxDirectory `
            -StandardOutputPath $standardOutputPath `
            -StandardErrorPath $standardErrorPath
        $rootStartTimeUtc = $process.StartTime.ToUniversalTime()

        $deadline = (Get-Date).AddSeconds($timeoutSeconds)
        Wait-ForTcpListener -Address $BrowserHost -Port $Port -Process $process -Deadline $deadline
        Wait-ForFrontendDocument -Uri $frontendUri -Process $process -Deadline $deadline

        $browserArguments = @(
            $browserScript,
            $BrowserCommand,
            "--url", $frontendUri,
            "--fixture", $fixturePath,
            "--artifact-directory", $artifactDirectory,
            "--report", $reportPath,
            "--timeout", ([Math]::Min($timeoutSeconds, 120) * 1000)
        )
        if (-not [string]::IsNullOrWhiteSpace($ExpectedHost)) {
            $browserArguments += @("--expected-host", $ExpectedHost)
        }
        & $node @browserArguments
        if ($LASTEXITCODE -ne 0) {
            throw "The $Id Playwright workflow failed with exit code $LASTEXITCODE."
        }
        if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
            throw "The $Id Playwright workflow produced no browser report."
        }
        $browserReport = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
        if ($browserReport.contract -ne "chataigne-product-browser-gate-v1" -or
            $browserReport.status -ne "passed") {
            throw "The $Id browser report did not satisfy the schema-v1 pass contract."
        }
        $reportedProjectPath = [string]$browserReport.loadedProjectPath

        $ownedProcessIds = @(Get-OwnedProcessIds `
                -RootProcessId $process.Id `
                -RootStartTimeUtc $rootStartTimeUtc)
        Stop-OwnedProcessTree `
            -RootProcessId $process.Id `
            -RootStartTimeUtc $rootStartTimeUtc
        Wait-ForRootProcessToExit -Process $process -TimeoutSeconds 20
        Wait-ForPortsReleased -Ports @($Port) -TimeoutSeconds 10
        $uploadedProjectCleanup = Remove-ReportedUploadedProject `
            -ReportedPath $reportedProjectPath `
            -ExpectedFileName $FixtureFileName

        [pscustomobject]@{
            contract                  = "chataigne-bundled-browser-hook-v1"
            id                        = $Id
            status                    = "passed"
            binary                    = $binary
            bind                      = "${BindAddress}:$Port"
            browser_url               = $frontendUri
            expected_non_loopback_host = if ([string]::IsNullOrWhiteSpace($ExpectedHost)) { $null } else { $ExpectedHost }
            browser_report            = $reportPath
            trace                     = Join-Path $artifactDirectory "$($browserReport.mode).trace.zip"
            uploaded_project_cleanup  = $uploadedProjectCleanup
            owned_process_cleanup     = "verified"
            released_port             = $Port
        } | ConvertTo-Json -Compress | Write-Host
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
                Stop-OwnedProcessTree `
                    -RootProcessId $process.Id `
                    -RootStartTimeUtc $rootStartTimeUtc
                Wait-ForOwnedProcessesToExit -ProcessIds $ownedProcessIds -TimeoutSeconds 10
            }
            catch {
                Write-Warning "Failed to stop every owned $Id process: $_"
            }
            try {
                Wait-ForPortsReleased -Ports @($Port) -TimeoutSeconds 10
            }
            catch {
                Write-Warning $_
            }
        }
        if (-not [string]::IsNullOrWhiteSpace($reportedProjectPath)) {
            try {
                $uploadedProjectCleanup = Remove-ReportedUploadedProject `
                    -ReportedPath $reportedProjectPath `
                    -ExpectedFileName $FixtureFileName
            }
            catch {
                Write-Warning "Failed to clean the reported uploaded project: $_"
            }
        }
        try {
            Remove-VerifiedRunSubdirectory -RunDirectory $runDirectory -Path $sandboxDirectory
        }
        catch {
            Write-Warning "Failed to remove the isolated browser-gate sandbox: $_"
        }
    }
}
