. (Join-Path $PSScriptRoot "smoke-common.ps1")

$repositoryRoot = Get-ProductGateRepositoryRoot
$runDirectory = Get-ProductGateRunDirectory -RepositoryRoot $repositoryRoot
$shutdownPath = Join-Path $runDirectory "cargo-run-smoke.shutdown"
$cargoArguments = @("run")
$shutdownFile = ""
if (Test-IsWindowsPlatform) {
    $cargoArguments += @("--", "--automation-shutdown-file", $shutdownPath)
    $shutdownFile = $shutdownPath
}

Invoke-RootCommandSmoke `
    -Id "cargo-run-smoke" `
    -CargoArguments $cargoArguments `
    -FrontendUri "http://127.0.0.1:7010/" `
    -Ports @(7010) `
    -ShutdownFile $shutdownFile
