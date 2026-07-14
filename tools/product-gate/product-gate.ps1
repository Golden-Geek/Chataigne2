[CmdletBinding()]
param(
    [string]$ReportPath,
    [string]$HookDirectory,
    [string[]]$EvidenceReportPath = @(),
    [switch]$SkipUiInstall,
    [switch]$DependencyAudit,
    [switch]$PlanOnly,
    [ValidateSet("normal-developer-product-core", "feature-complete-platform")]
    [string]$FeatureMatrix = "normal-developer-product-core",
    [string[]]$CargoFeatures = @(),
    [ValidateSet("windows", "macos", "linux")]
    [string[]]$RequiredPlatforms = @()
)

$ErrorActionPreference = "Stop"

$RepositoryRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $PSScriptRoot "..\..")
)
$StartedAt = [DateTimeOffset]::UtcNow

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $runName = $StartedAt.ToString("yyyyMMddTHHmmssZ")
    $ReportPath = Join-Path $RepositoryRoot "target\product-gate\$runName\product-gate-report.json"
}
elseif (-not [System.IO.Path]::IsPathRooted($ReportPath)) {
    $ReportPath = Join-Path $RepositoryRoot $ReportPath
}
$ReportPath = [System.IO.Path]::GetFullPath($ReportPath)
$RunDirectory = Split-Path -Parent $ReportPath
$LogDirectory = Join-Path $RunDirectory "logs"

if ([string]::IsNullOrWhiteSpace($HookDirectory)) {
    $HookDirectory = Join-Path $PSScriptRoot "hooks"
}
elseif (-not [System.IO.Path]::IsPathRooted($HookDirectory)) {
    $HookDirectory = Join-Path $RepositoryRoot $HookDirectory
}
$HookDirectory = [System.IO.Path]::GetFullPath($HookDirectory)

New-Item -ItemType Directory -Path $LogDirectory -Force | Out-Null

$script:Results = @()
$script:ExternalReports = @()

function Convert-ToCommandText {
    param(
        [string]$Executable,
        [string[]]$Arguments = @()
    )

    $tokens = @($Executable) + $Arguments
    return (($tokens | ForEach-Object {
                $token = [string]$_
                if ($token -match '[\s''"]') {
                    "'" + $token.Replace("'", "''") + "'"
                }
                else {
                    $token
                }
            }) -join " ")
}

function Get-SafeLogName {
    param([string]$Id)

    return ($Id -replace "[^A-Za-z0-9_.-]", "_") + ".log"
}

function Add-GateResult {
    param(
        [string]$Id,
        [string]$Name,
        [ValidateSet("PASS", "FAIL", "BLOCKED", "NOT_RUN")]
        [string]$Status,
        [bool]$Required = $true,
        [string]$Command = "",
        [string]$Executable = "",
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory = $RepositoryRoot,
        [Nullable[int]]$ExitCode = $null,
        [string]$Reason = "",
        [string[]]$BlockedBy = @(),
        [Nullable[DateTimeOffset]]$Started = $null,
        [Nullable[DateTimeOffset]]$Finished = $null,
        [Nullable[long]]$DurationMs = $null,
        [string]$LogPath = "",
        [string]$EvidenceSource = ""
    )

    $result = [ordered]@{
        id                = $Id
        name              = $Name
        status            = $Status
        required          = $Required
        command           = $Command
        executable        = $Executable
        arguments         = @($Arguments)
        working_directory = $WorkingDirectory
        started_at_utc    = if ($null -ne $Started) { ([DateTimeOffset]$Started).ToString("o") } else { $null }
        finished_at_utc   = if ($null -ne $Finished) { ([DateTimeOffset]$Finished).ToString("o") } else { $null }
        duration_ms       = if ($null -ne $DurationMs) { [long]$DurationMs } else { $null }
        exit_code         = if ($null -ne $ExitCode) { [int]$ExitCode } else { $null }
        reason            = $Reason
        blocked_by        = @($BlockedBy)
        log_path          = $LogPath
        evidence_source   = $EvidenceSource
    }
    $script:Results += [pscustomobject]$result

    $color = switch ($Status) {
        "PASS" { "Green" }
        "FAIL" { "Red" }
        "BLOCKED" { "Yellow" }
        default { "DarkYellow" }
    }
    Write-Host ("[{0}] {1}" -f $Status, $Name) -ForegroundColor $color
    if (-not [string]::IsNullOrWhiteSpace($Reason)) {
        Write-Host ("       {0}" -f $Reason)
    }

    return $script:Results[-1]
}

function Get-GateResult {
    param([string]$Id)

    return @($script:Results | Where-Object { $_.id -eq $Id } | Select-Object -Last 1)
}

function Get-BlockingDependencies {
    param([string[]]$DependsOn = @())

    $blocked = @()
    foreach ($dependency in $DependsOn) {
        $result = @(Get-GateResult -Id $dependency)
        if ($result.Count -eq 0 -or $result[0].status -ne "PASS") {
            $blocked += $dependency
        }
    }
    return $blocked
}

function Add-BlockedResult {
    param(
        [string]$Id,
        [string]$Name,
        [string[]]$BlockedBy,
        [string]$Reason = "A prerequisite did not pass.",
        [bool]$Required = $true,
        [string]$Command = ""
    )

    return Add-GateResult `
        -Id $Id `
        -Name $Name `
        -Status "BLOCKED" `
        -Required $Required `
        -Command $Command `
        -Reason $Reason `
        -BlockedBy $BlockedBy
}

function Add-NotRunResult {
    param(
        [string]$Id,
        [string]$Name,
        [string]$Reason,
        [bool]$Required = $true,
        [string]$Command = ""
    )

    return Add-GateResult `
        -Id $Id `
        -Name $Name `
        -Status "NOT_RUN" `
        -Required $Required `
        -Command $Command `
        -Reason $Reason
}

function Invoke-GateCommand {
    param(
        [string]$Id,
        [string]$Name,
        [string]$Executable,
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory = $RepositoryRoot,
        [string[]]$DependsOn = @(),
        [bool]$Required = $true
    )

    $commandText = Convert-ToCommandText -Executable $Executable -Arguments $Arguments
    $blocking = @(Get-BlockingDependencies -DependsOn $DependsOn)
    if ($blocking.Count -gt 0) {
        return Add-BlockedResult `
            -Id $Id `
            -Name $Name `
            -BlockedBy $blocking `
            -Required $Required `
            -Command $commandText
    }

    if ($PlanOnly) {
        return Add-NotRunResult `
            -Id $Id `
            -Name $Name `
            -Reason "Plan-only mode: command was deliberately not executed." `
            -Required $Required `
            -Command $commandText
    }

    $commandInfo = Get-Command $Executable -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $commandInfo) {
        return Add-GateResult `
            -Id $Id `
            -Name $Name `
            -Status "FAIL" `
            -Required $Required `
            -Command $commandText `
            -Executable $Executable `
            -Arguments $Arguments `
            -WorkingDirectory $WorkingDirectory `
            -ExitCode (-1) `
            -Reason "Required executable '$Executable' was not found on PATH."
    }

    $resolvedExecutable = if ([string]::IsNullOrWhiteSpace($commandInfo.Source)) {
        $commandInfo.Name
    }
    else {
        $commandInfo.Source
    }
    $commandText = Convert-ToCommandText -Executable $resolvedExecutable -Arguments $Arguments
    $logPath = Join-Path $LogDirectory (Get-SafeLogName -Id $Id)
    $relativeLogPath = Join-Path "logs" (Get-SafeLogName -Id $Id)
    $started = [DateTimeOffset]::UtcNow
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $output = @()
    $exitCode = -1
    $failureReason = ""
    $previousErrorActionPreference = $ErrorActionPreference

    Write-Host ""
    Write-Host ("==> {0}" -f $Name)
    Write-Host ("    {0}" -f $commandText)

    Push-Location $WorkingDirectory
    try {
        # Windows PowerShell promotes redirected native stderr to ErrorRecord objects.
        # Keep those records in the log without turning a normal nonzero native exit
        # into a terminating PowerShell exception that would erase the real exit code.
        $ErrorActionPreference = "Continue"
        $output = @(& $resolvedExecutable @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
        if ($null -eq $exitCode) {
            $exitCode = 0
        }
    }
    catch {
        $output += $_ | Out-String
        $exitCode = -1
        $failureReason = $_.Exception.Message
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
        $stopwatch.Stop()
    }

    $outputText = @($output | ForEach-Object { [string]$_ })
    [System.IO.File]::WriteAllLines($logPath, $outputText, [System.Text.UTF8Encoding]::new($false))
    foreach ($line in $outputText) {
        Write-Host $line
    }

    $finished = [DateTimeOffset]::UtcNow
    if ($exitCode -eq 0) {
        return Add-GateResult `
            -Id $Id `
            -Name $Name `
            -Status "PASS" `
            -Required $Required `
            -Command $commandText `
            -Executable $resolvedExecutable `
            -Arguments $Arguments `
            -WorkingDirectory $WorkingDirectory `
            -ExitCode $exitCode `
            -Started $started `
            -Finished $finished `
            -DurationMs $stopwatch.ElapsedMilliseconds `
            -LogPath $relativeLogPath
    }

    if ([string]::IsNullOrWhiteSpace($failureReason)) {
        $failureReason = "Command exited with code $exitCode."
    }
    return Add-GateResult `
        -Id $Id `
        -Name $Name `
        -Status "FAIL" `
        -Required $Required `
        -Command $commandText `
        -Executable $resolvedExecutable `
        -Arguments $Arguments `
        -WorkingDirectory $WorkingDirectory `
        -ExitCode $exitCode `
        -Reason $failureReason `
        -Started $started `
        -Finished $finished `
        -DurationMs $stopwatch.ElapsedMilliseconds `
        -LogPath $relativeLogPath
}

function Add-DerivedResult {
    param(
        [string]$Id,
        [string]$Name,
        [string[]]$DependsOn,
        [string]$Reason,
        [bool]$Required = $true
    )

    $blocking = @(Get-BlockingDependencies -DependsOn $DependsOn)
    if ($blocking.Count -gt 0) {
        return Add-BlockedResult `
            -Id $Id `
            -Name $Name `
            -BlockedBy $blocking `
            -Required $Required
    }

    $dependencyResults = @($DependsOn | ForEach-Object {
            @(Get-GateResult -Id $_) | Select-Object -First 1
        })
    $commands = @($dependencyResults | ForEach-Object { $_.command } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) })

    return Add-GateResult `
        -Id $Id `
        -Name $Name `
        -Status "PASS" `
        -Required $Required `
        -Command ($commands -join " && ") `
        -ExitCode 0 `
        -Reason $Reason `
        -BlockedBy @()
}

function Get-LogContent {
    param([string]$Id)

    $result = @(Get-GateResult -Id $Id)
    if ($result.Count -eq 0 -or [string]::IsNullOrWhiteSpace($result[0].log_path)) {
        return ""
    }
    $path = Join-Path $RunDirectory $result[0].log_path
    if (-not (Test-Path -LiteralPath $path)) {
        return ""
    }
    return ([System.IO.File]::ReadAllText($path)).Trim()
}

function Get-CurrentPlatform {
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
            [System.Runtime.InteropServices.OSPlatform]::Windows
        )) {
        return "windows"
    }
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
            [System.Runtime.InteropServices.OSPlatform]::OSX
        )) {
        return "macos"
    }
    return "linux"
}

function Invoke-HookResult {
    param(
        [string]$Id,
        [string]$Name,
        [string]$HookFile,
        [string[]]$DependsOn
    )

    $hookPath = Join-Path $HookDirectory $HookFile
    $blocking = @(Get-BlockingDependencies -DependsOn $DependsOn)
    if ($blocking.Count -gt 0) {
        return Add-BlockedResult `
            -Id $Id `
            -Name $Name `
            -BlockedBy $blocking `
            -Reason "The real smoke/evidence hook cannot run until its build prerequisites pass." `
            -Command $hookPath
    }
    if (-not (Test-Path -LiteralPath $hookPath -PathType Leaf)) {
        return Add-BlockedResult `
            -Id $Id `
            -Name $Name `
            -BlockedBy @("missing-hook:$HookFile") `
            -Reason "No real readiness/evidence hook is registered at '$hookPath'; this check is not implemented and cannot pass." `
            -Command $hookPath
    }

    $powershellExecutable = (Get-Process -Id $PID).Path
    $previousRoot = $env:PRODUCT_GATE_REPOSITORY_ROOT
    $previousRun = $env:PRODUCT_GATE_RUN_DIRECTORY
    $previousCommit = $env:PRODUCT_GATE_COMMIT_SHA
    try {
        $env:PRODUCT_GATE_REPOSITORY_ROOT = $RepositoryRoot
        $env:PRODUCT_GATE_RUN_DIRECTORY = $RunDirectory
        $env:PRODUCT_GATE_COMMIT_SHA = Get-LogContent -Id "fingerprint.git_commit"
        return Invoke-GateCommand `
            -Id $Id `
            -Name $Name `
            -Executable $powershellExecutable `
            -Arguments @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $hookPath) `
            -WorkingDirectory $RepositoryRoot `
            -DependsOn $DependsOn
    }
    finally {
        $env:PRODUCT_GATE_REPOSITORY_ROOT = $previousRoot
        $env:PRODUCT_GATE_RUN_DIRECTORY = $previousRun
        $env:PRODUCT_GATE_COMMIT_SHA = $previousCommit
    }
}

function Import-ExternalReports {
    foreach ($pathValue in $EvidenceReportPath) {
        $path = $pathValue
        if (-not [System.IO.Path]::IsPathRooted($path)) {
            $path = Join-Path $RepositoryRoot $path
        }
        $path = [System.IO.Path]::GetFullPath($path)
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            Add-GateResult `
                -Id ("evidence.report.{0}" -f $script:ExternalReports.Count) `
                -Name "External evidence report" `
                -Status "FAIL" `
                -Command $path `
                -ExitCode (-1) `
                -Reason "Evidence report does not exist." | Out-Null
            continue
        }
        try {
            $report = [System.IO.File]::ReadAllText($path) | ConvertFrom-Json
            $script:ExternalReports += [pscustomobject]@{
                path   = $path
                report = $report
            }
        }
        catch {
            Add-GateResult `
                -Id ("evidence.report.{0}" -f $script:ExternalReports.Count) `
                -Name "External evidence report" `
                -Status "FAIL" `
                -Command $path `
                -ExitCode (-1) `
                -Reason ("Evidence report is not valid JSON: {0}" -f $_.Exception.Message) | Out-Null
        }
    }
}

function Add-ExternalEvidenceResult {
    param(
        [string]$Id,
        [string]$Name,
        [string]$CurrentCommit,
        [bool]$WorkingTreeDirty
    )

    if ($WorkingTreeDirty) {
        return Add-BlockedResult `
            -Id $Id `
            -Name $Name `
            -BlockedBy @("dirty-working-tree") `
            -Reason "External evidence cannot qualify uncommitted content. Commit the exact tree and rerun its platform gate."
    }

    foreach ($external in $script:ExternalReports) {
        $report = $external.report
        if ($report.schema_version -ne 1 -or $report.commit.sha -ne $CurrentCommit) {
            continue
        }
        $candidate = @($report.results | Where-Object { $_.id -eq $Id } | Select-Object -Last 1)
        if ($candidate.Count -eq 0) {
            continue
        }
        if (
            $candidate[0].status -eq "PASS" -and
            $candidate[0].exit_code -eq 0 -and
            -not [string]::IsNullOrWhiteSpace($candidate[0].command)
        ) {
            return Add-GateResult `
                -Id $Id `
                -Name $Name `
                -Status "PASS" `
                -Command $candidate[0].command `
                -Executable $candidate[0].executable `
                -Arguments @($candidate[0].arguments) `
                -WorkingDirectory $candidate[0].working_directory `
                -ExitCode 0 `
                -Reason "Imported passing evidence for the exact commit." `
                -EvidenceSource $external.path
        }
        return Add-GateResult `
            -Id $Id `
            -Name $Name `
            -Status "FAIL" `
            -Command $candidate[0].command `
            -ExitCode $candidate[0].exit_code `
            -Reason "External evidence exists for this commit but is not a valid PASS with exit code 0 and an exact command." `
            -EvidenceSource $external.path
    }

    return Add-BlockedResult `
        -Id $Id `
        -Name $Name `
        -BlockedBy @("missing-external-evidence") `
        -Reason "No passing product-gate report for this platform and exact commit was supplied."
}

function Get-OverallStatus {
    $required = @($script:Results | Where-Object { $_.required })
    if (@($required | Where-Object { $_.status -eq "FAIL" }).Count -gt 0) {
        return "FAIL"
    }
    if (@($required | Where-Object { $_.status -eq "BLOCKED" }).Count -gt 0) {
        return "BLOCKED"
    }
    if (@($required | Where-Object { $_.status -eq "NOT_RUN" }).Count -gt 0) {
        return "NOT_RUN"
    }
    return "PASS"
}

function Write-GateReport {
    $finishedAt = [DateTimeOffset]::UtcNow
    $toolchainManifestPath = Join-Path $RepositoryRoot "tools/bootstrap/toolchain.json"
    $rustcOutput = Get-LogContent -Id "fingerprint.rustc"
    $hostMatch = [regex]::Match($rustcOutput, "(?m)^host:\s*(.+)$")
    $gitStatus = Get-LogContent -Id "fingerprint.git_status"
    $currentCommit = Get-LogContent -Id "fingerprint.git_commit"
    $overallStatus = Get-OverallStatus
    $counts = [ordered]@{}
    foreach ($status in @("PASS", "FAIL", "BLOCKED", "NOT_RUN")) {
        $counts[$status] = @($script:Results | Where-Object { $_.status -eq $status }).Count
    }

    $report = [ordered]@{
        schema_version = 1
        gate           = "chataigne-product-gate"
        validation     = "RUNNABLE"
        overall_status = $overallStatus
        started_at_utc = $StartedAt.ToString("o")
        finished_at_utc = $finishedAt.ToString("o")
        duration_ms    = [long]($finishedAt - $StartedAt).TotalMilliseconds
        repository_root = $RepositoryRoot
        report_path    = $ReportPath
        commit         = [ordered]@{
            sha               = $currentCommit
            working_tree_dirty = -not [string]::IsNullOrWhiteSpace($gitStatus)
        }
        toolchain      = [ordered]@{
            rustc            = $rustcOutput
            cargo            = Get-LogContent -Id "fingerprint.cargo"
            target_host      = if ($hostMatch.Success) { $hostMatch.Groups[1].Value.Trim() } else { "" }
            node             = Get-LogContent -Id "fingerprint.node"
            package_manager  = Get-LogContent -Id "fingerprint.npm"
            powershell       = $PSVersionTable.PSVersion.ToString()
            os_description   = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
            process_arch     = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
            canonical_manifest = "tools/bootstrap/toolchain.json"
            canonical_manifest_sha256 = (Get-FileHash -LiteralPath $toolchainManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
            feature_matrix   = $FeatureMatrix
            cargo_features   = @($CargoFeatures)
        }
        required_platforms = @($RequiredPlatforms)
        hook_directory = $HookDirectory
        plan_only      = [bool]$PlanOnly
        counts         = $counts
        results        = @($script:Results)
    }

    [System.IO.File]::WriteAllText(
        $ReportPath,
        ($report | ConvertTo-Json -Depth 12),
        [System.Text.UTF8Encoding]::new($false)
    )

    Write-Host ""
    Write-Host ("Product gate: {0}" -f $overallStatus)
    Write-Host ("Report: {0}" -f $ReportPath)
    return $overallStatus
}

Set-Location $RepositoryRoot

if ($RequiredPlatforms.Count -eq 0) {
    $RequiredPlatforms = @(Get-CurrentPlatform)
}

$npmExecutable = if (Get-Command "npm.cmd" -ErrorAction SilentlyContinue) {
    "npm.cmd"
}
else {
    "npm"
}
$gatePowerShellExecutable = (Get-Process -Id $PID).Path

$cargoFeatureArguments = @()
if ($CargoFeatures.Count -gt 0) {
    $cargoFeatureArguments = @("--features", ($CargoFeatures -join ","))
}

Invoke-GateCommand `
    -Id "fingerprint.git_commit" `
    -Name "git commit fingerprint" `
    -Executable "git" `
    -Arguments @("rev-parse", "HEAD") | Out-Null
Invoke-GateCommand `
    -Id "fingerprint.git_status" `
    -Name "git working-tree fingerprint" `
    -Executable "git" `
    -Arguments @("status", "--porcelain=v1", "--untracked-files=all") | Out-Null
Invoke-GateCommand `
    -Id "fingerprint.rustc" `
    -Name "rustc -vV" `
    -Executable "rustc" `
    -Arguments @("-vV") | Out-Null
Invoke-GateCommand `
    -Id "fingerprint.cargo" `
    -Name "cargo -vV" `
    -Executable "cargo" `
    -Arguments @("-vV") | Out-Null
Invoke-GateCommand `
    -Id "fingerprint.node" `
    -Name "node --version" `
    -Executable "node" `
    -Arguments @("--version") | Out-Null
Invoke-GateCommand `
    -Id "fingerprint.npm" `
    -Name "npm --version" `
    -Executable $npmExecutable `
    -Arguments @("--version") | Out-Null
Invoke-GateCommand `
    -Id "toolchain.contract" `
    -Name "Canonical supported toolchain contract" `
    -Executable $gatePowerShellExecutable `
    -Arguments @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", (Join-Path $RepositoryRoot "tools/bootstrap/verify-toolchain.ps1"), "-CheckInstalled") `
    -DependsOn @("fingerprint.rustc", "fingerprint.cargo", "fingerprint.node", "fingerprint.npm") | Out-Null

$uiDirectory = Join-Path $RepositoryRoot "apps/chataigne/ui"
if ($SkipUiInstall) {
    Add-NotRunResult `
        -Id "ui.npm_ci" `
        -Name "UI dependency installation" `
        -Reason "Skipped by -SkipUiInstall; existing installation is verified separately." `
        -Required $false `
        -Command "npm ci" | Out-Null

    if ($PlanOnly) {
        Add-NotRunResult `
            -Id "ui.dependencies_ready" `
            -Name "UI dependencies ready" `
            -Reason "Plan-only mode: dependency lock state was not inspected." | Out-Null
    }
    else {
        $packageLock = Join-Path $RepositoryRoot "package-lock.json"
        $installedLock = Join-Path $RepositoryRoot "node_modules\.package-lock.json"
        $ready = (
            (Test-Path -LiteralPath $packageLock -PathType Leaf) -and
            (Test-Path -LiteralPath $installedLock -PathType Leaf) -and
            ((Get-Item -LiteralPath $installedLock).LastWriteTimeUtc -ge
                (Get-Item -LiteralPath $packageLock).LastWriteTimeUtc)
        )
        if ($ready) {
            Add-GateResult `
                -Id "ui.dependencies_ready" `
                -Name "UI dependencies ready" `
                -Status "PASS" `
                -Command "verify node_modules/.package-lock.json is current" `
                -Reason "Existing node_modules lock is present and not older than package-lock.json." | Out-Null
        }
        else {
            Add-GateResult `
                -Id "ui.dependencies_ready" `
                -Name "UI dependencies ready" `
                -Status "FAIL" `
                -Command "verify node_modules/.package-lock.json is current" `
                -ExitCode (-1) `
                -Reason "-SkipUiInstall was used, but the installed dependency lock is missing or stale." | Out-Null
        }
    }
}
else {
    Invoke-GateCommand `
        -Id "ui.npm_ci" `
        -Name "UI dependency installation" `
        -Executable $npmExecutable `
        -Arguments @("ci") `
        -WorkingDirectory $RepositoryRoot `
        -DependsOn @("toolchain.contract") | Out-Null
    Add-DerivedResult `
        -Id "ui.dependencies_ready" `
        -Name "UI dependencies ready" `
        -DependsOn @("ui.npm_ci") `
        -Reason "npm ci completed successfully." | Out-Null
}

if ($DependencyAudit) {
    Invoke-GateCommand `
        -Id "dependency.qualification_tools" `
        -Name "Pinned dependency qualification tools" `
        -Executable $gatePowerShellExecutable `
        -Arguments @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", (Join-Path $RepositoryRoot "tools/bootstrap/verify-toolchain.ps1"), "-CheckInstalled", "-CheckQualificationTools") `
        -DependsOn @("toolchain.contract") | Out-Null
    Invoke-GateCommand `
        -Id "dependency.cargo_deny" `
        -Name "Cargo advisories, licenses, sources, and bans" `
        -Executable "cargo" `
        -Arguments @("deny", "check") `
        -DependsOn @("dependency.qualification_tools") | Out-Null
    Invoke-GateCommand `
        -Id "dependency.cargo_machete" `
        -Name "Cargo unused dependency check" `
        -Executable "cargo" `
        -Arguments @("machete") `
        -DependsOn @("dependency.qualification_tools") | Out-Null
    Invoke-GateCommand `
        -Id "dependency.duplicate_versions" `
        -Name "Cargo reviewed duplicate-version baseline" `
        -Executable "python" `
        -Arguments @("tools/dependency-gate/check_duplicate_versions.py") `
        -DependsOn @("toolchain.contract") | Out-Null
    Invoke-GateCommand `
        -Id "dependency.npm_audit" `
        -Name "npm production dependency audit" `
        -Executable $npmExecutable `
        -Arguments @("audit", "--omit=dev", "--audit-level=moderate") `
        -WorkingDirectory $RepositoryRoot `
        -DependsOn @("ui.dependencies_ready") | Out-Null
}
else {
    Add-NotRunResult -Id "dependency.qualification_tools" -Name "Pinned dependency qualification tools" -Reason "Dependency audit profile was not requested." -Required $false | Out-Null
    Add-NotRunResult -Id "dependency.cargo_deny" -Name "Cargo advisories, licenses, sources, and bans" -Reason "Dependency audit profile was not requested." -Required $false | Out-Null
    Add-NotRunResult -Id "dependency.cargo_machete" -Name "Cargo unused dependency check" -Reason "Dependency audit profile was not requested." -Required $false | Out-Null
    Add-NotRunResult -Id "dependency.duplicate_versions" -Name "Cargo reviewed duplicate-version baseline" -Reason "Dependency audit profile was not requested." -Required $false | Out-Null
    Add-NotRunResult -Id "dependency.npm_audit" -Name "npm production dependency audit" -Reason "Dependency audit profile was not requested." -Required $false | Out-Null
}

Invoke-GateCommand `
    -Id "ui.build" `
    -Name "Svelte production build" `
    -Executable $npmExecutable `
    -Arguments @("run", "build") `
    -WorkingDirectory $uiDirectory `
    -DependsOn @("ui.dependencies_ready") | Out-Null

# Every Cargo invocation below consumes the same validated bundle. Keeping this
# environment stable also prevents dev-server cache writes from invalidating Rust artifacts.
$env:GC_UI_ASSUME_BUILT = "1"

Invoke-GateCommand `
    -Id "rust.build" `
    -Name "Rust workspace build" `
    -Executable "cargo" `
    -Arguments (@("build", "--workspace", "--all-targets") + $cargoFeatureArguments) `
    -DependsOn @("toolchain.contract", "ui.build") | Out-Null

Invoke-GateCommand `
    -Id "ui.browser_install" `
    -Name "Pinned Playwright Chromium installation" `
    -Executable $npmExecutable `
    -Arguments @("exec", "--", "playwright-core", "install", "chromium") `
    -WorkingDirectory $uiDirectory `
    -DependsOn @("ui.dependencies_ready") | Out-Null

Invoke-GateCommand `
    -Id "product_manifest.drift" `
    -Name "Product manifest drift" `
    -Executable "python" `
    -Arguments @("tools/migration/product_manifest.py", "check") `
    -DependsOn @("toolchain.contract") | Out-Null
Invoke-GateCommand `
    -Id "product_manifest.schema" `
    -Name "Product manifest schema" `
    -Executable "python" `
    -Arguments @("tools/migration/product_manifest.py", "validate") `
    -DependsOn @("product_manifest.drift") | Out-Null
Invoke-GateCommand `
    -Id "product_manifest.tests" `
    -Name "Product manifest generator tests" `
    -Executable "python" `
    -Arguments @("-m", "unittest", "discover", "-s", "tools/migration/tests", "-v") `
    -DependsOn @("product_manifest.schema") | Out-Null
Invoke-GateCommand `
    -Id "architecture.phase2_contracts" `
    -Name "Phase 2 application seam contracts" `
    -Executable "python" `
    -Arguments @("tools/migration/check_phase2_contracts.py") `
    -DependsOn @("product_manifest.tests") | Out-Null
Invoke-GateCommand `
    -Id "architecture.phase3_contracts" `
    -Name "Phase 3 foundation contracts" `
    -Executable "python" `
    -Arguments @("tools/migration/check_phase3_contracts.py") `
    -DependsOn @("architecture.phase2_contracts") | Out-Null

Invoke-GateCommand `
    -Id "rust.format" `
    -Name "Rust formatting" `
    -Executable "cargo" `
    -Arguments @("fmt", "--all", "--", "--check") `
    -DependsOn @("rust.build") | Out-Null
Invoke-GateCommand `
    -Id "rust.clippy" `
    -Name "Rust clippy" `
    -Executable "cargo" `
    -Arguments (@("clippy", "-p", "Chataigne2", "--all-targets", "--no-deps") + $cargoFeatureArguments + @("--", "-D", "warnings")) `
    -DependsOn @("rust.build") | Out-Null
Invoke-GateCommand `
    -Id "rust.test" `
    -Name "Rust workspace tests" `
    -Executable "cargo" `
    -Arguments (@("test", "--workspace") + $cargoFeatureArguments) `
    -DependsOn @("rust.build") | Out-Null
Invoke-GateCommand `
    -Id "rust.runtime_build" `
    -Name "Final Chataigne runtime build" `
    -Executable "cargo" `
    -Arguments @("build", "-p", "Chataigne2", "--bin", "Chataigne2") `
    -DependsOn @("rust.format", "rust.clippy", "rust.test") | Out-Null

Invoke-GateCommand `
    -Id "ui.check" `
    -Name "Svelte check" `
    -Executable $npmExecutable `
    -Arguments @("run", "check") `
    -WorkingDirectory $uiDirectory `
    -DependsOn @("ui.build") | Out-Null
Invoke-GateCommand `
    -Id "ui.lint" `
    -Name "Svelte lint" `
    -Executable $npmExecutable `
    -Arguments @("run", "lint") `
    -WorkingDirectory $uiDirectory `
    -DependsOn @("ui.build") | Out-Null
Invoke-GateCommand `
    -Id "ui.unit_tests" `
    -Name "Svelte unit tests" `
    -Executable $npmExecutable `
    -Arguments @("run", "test") `
    -WorkingDirectory $uiDirectory `
    -DependsOn @("ui.build") | Out-Null

Invoke-HookResult `
    -Id "smoke.cargo_run" `
    -Name "Root cargo run readiness smoke" `
    -HookFile "cargo-run-smoke.ps1" `
    -DependsOn @("rust.runtime_build", "ui.browser_install") | Out-Null
Invoke-HookResult `
    -Id "evidence.module_loopback" `
    -Name "Production OSC module loopback evidence" `
    -HookFile "module-loopback-smoke.ps1" `
    -DependsOn @("rust.runtime_build") | Out-Null
Invoke-HookResult `
    -Id "smoke.watch" `
    -Name "Root watch readiness smoke" `
    -HookFile "watch-smoke.ps1" `
    -DependsOn @("rust.runtime_build", "ui.browser_install") | Out-Null
Invoke-HookResult `
    -Id "smoke.cargo_run_dev" `
    -Name "Root cargo run -- --dev readiness smoke" `
    -HookFile "cargo-run-dev-smoke.ps1" `
    -DependsOn @("rust.runtime_build", "ui.browser_install") | Out-Null
Invoke-HookResult `
    -Id "e2e.ui_workflow" `
    -Name "Mounted real-app UI workflow" `
    -HookFile "ui-workflow.ps1" `
    -DependsOn @("smoke.cargo_run_dev", "ui.browser_install") | Out-Null
Invoke-HookResult `
    -Id "e2e.lan_non_loopback" `
    -Name "Non-loopback LAN browser workflow" `
    -HookFile "lan-browser.ps1" `
    -DependsOn @("smoke.cargo_run_dev", "ui.browser_install") | Out-Null

Import-ExternalReports
$currentPlatform = Get-CurrentPlatform
$currentCommit = Get-LogContent -Id "fingerprint.git_commit"
$workingTreeDirty = -not [string]::IsNullOrWhiteSpace(
    (Get-LogContent -Id "fingerprint.git_status")
)
foreach ($platform in @("windows", "macos", "linux")) {
    $id = "platform.$platform"
    $name = "$platform product build"
    if ($RequiredPlatforms -notcontains $platform) {
        Add-NotRunResult `
            -Id $id `
            -Name $name `
            -Reason "Platform is not part of this invocation's declared required matrix." `
            -Required $false | Out-Null
    }
    elseif ($platform -eq $currentPlatform) {
        Add-DerivedResult `
            -Id $id `
            -Name $name `
            -DependsOn @("rust.runtime_build", "ui.build") `
            -Reason "The real Rust targets and production UI built on this platform." | Out-Null
    }
    else {
        Add-ExternalEvidenceResult `
            -Id $id `
            -Name $name `
            -CurrentCommit $currentCommit `
            -WorkingTreeDirty $workingTreeDirty | Out-Null
    }
}

$overallStatus = Write-GateReport
switch ($overallStatus) {
    "PASS" { exit 0 }
    "FAIL" { exit 1 }
    "BLOCKED" { exit 2 }
    default { exit 3 }
}
