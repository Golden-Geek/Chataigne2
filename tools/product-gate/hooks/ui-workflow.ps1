. (Join-Path $PSScriptRoot "browser-gate-common.ps1")

Invoke-BundledBrowserGate `
    -Id "ui-workflow" `
    -BrowserCommand "product-gate-workflow" `
    -BindAddress "127.0.0.1" `
    -BrowserHost "127.0.0.1" `
    -Port 7021 `
    -FixtureFileName "phase0-ui-workflow.noisette"
