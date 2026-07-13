. (Join-Path $PSScriptRoot "smoke-common.ps1")

$repositoryRoot = Get-ProductGateRepositoryRoot
$runDirectory = Get-ProductGateRunDirectory -RepositoryRoot $repositoryRoot
$shutdownPath = Join-Path $runDirectory "cargo-run-smoke.shutdown"

Invoke-RootCommandSmoke `
    -Id "cargo-run-smoke" `
    -CargoArguments @("run", "--", "--automation-shutdown-file", $shutdownPath) `
    -FrontendUri "http://127.0.0.1:7010/" `
    -Ports @(7010) `
    -ShutdownFile $shutdownPath
