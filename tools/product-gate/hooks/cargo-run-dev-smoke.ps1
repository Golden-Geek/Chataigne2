. (Join-Path $PSScriptRoot "smoke-common.ps1")

$repositoryRoot = Get-ProductGateRepositoryRoot
$runDirectory = Get-ProductGateRunDirectory -RepositoryRoot $repositoryRoot
$shutdownPath = Join-Path $runDirectory "cargo-run-dev-smoke.shutdown"
$cargoArguments = @("run", "--", "--dev")
$shutdownFile = ""
if (Test-IsWindowsPlatform) {
    $cargoArguments += @("--automation-shutdown-file", $shutdownPath)
    $shutdownFile = $shutdownPath
}

Invoke-RootCommandSmoke `
    -Id "cargo-run-dev-smoke" `
    -CargoArguments $cargoArguments `
    -FrontendUri "http://127.0.0.1:5173/" `
    -Ports @(7010, 5173) `
    -ShutdownFile $shutdownFile
