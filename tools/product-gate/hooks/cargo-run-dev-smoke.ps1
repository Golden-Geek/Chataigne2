. (Join-Path $PSScriptRoot "smoke-common.ps1")

Invoke-RootCommandSmoke `
    -Id "cargo-run-dev-smoke" `
    -CargoArguments @("run", "--", "--dev") `
    -FrontendUri "http://127.0.0.1:5173/" `
    -Ports @(7010, 5173)
