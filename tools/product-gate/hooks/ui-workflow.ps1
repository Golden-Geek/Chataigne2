param(
    [string]$Id = "ui-workflow",
    [string]$SourceFixturePath = "",
    [string]$FixtureFileName = "ui-workflow.noisette",
    [int]$Port = 7021,
    [string]$ProductBinary = "",
    [int]$BrowserTimeoutSeconds = 120,
    [switch]$DisableBrowserTrace
)

. (Join-Path $PSScriptRoot "browser-gate-common.ps1")

Invoke-BundledBrowserGate `
    -Id $Id `
    -BrowserCommand "product-gate-workflow" `
    -BindAddress "127.0.0.1" `
    -BrowserHost "127.0.0.1" `
    -Port $Port `
    -FixtureFileName $FixtureFileName `
    -SourceFixturePath $SourceFixturePath `
    -ProductBinary $ProductBinary `
    -BrowserTimeoutSeconds $BrowserTimeoutSeconds `
    -DisableBrowserTrace:$DisableBrowserTrace
