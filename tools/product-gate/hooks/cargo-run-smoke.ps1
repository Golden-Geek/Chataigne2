. (Join-Path $PSScriptRoot "smoke-common.ps1")

Invoke-RootCommandSmoke `
    -Id "cargo-run-smoke" `
    -CargoArguments @("run") `
    -FrontendUri "http://127.0.0.1:7010/" `
    -Ports @(7010)
