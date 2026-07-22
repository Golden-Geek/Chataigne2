param(
    [string]$Id = "soak",
    [string]$SourceFixturePath = "",
    [string]$FixtureFileName = "soak.noisette",
    [int]$Port = 7041,
    [string]$ProductBinary = "",
    [long]$DurationMilliseconds = 300000,
    [int]$ClientCount = 3,
    [int]$MutationIntervalMilliseconds = 1000,
    [int]$BrowserTimeoutSeconds = 120
)

if ($DurationMilliseconds -le 0) {
    throw "DurationMilliseconds must be positive."
}
if ($ClientCount -lt 2) {
    throw "ClientCount must be at least two."
}

. (Join-Path $PSScriptRoot "browser-gate-common.ps1")

Invoke-BundledBrowserGate `
    -Id $Id `
    -BrowserCommand "product-gate-soak" `
    -BindAddress "127.0.0.1" `
    -BrowserHost "127.0.0.1" `
    -Port $Port `
    -FixtureFileName $FixtureFileName `
    -SourceFixturePath $SourceFixturePath `
    -ProductBinary $ProductBinary `
    -ExpectedBrowserContract "chataigne-multiclient-soak-v1" `
    -BrowserTimeoutSeconds $BrowserTimeoutSeconds `
    -BrowserExtraArguments @(
        "--duration-ms", [string]$DurationMilliseconds,
        "--clients", [string]$ClientCount,
        "--interval-ms", [string]$MutationIntervalMilliseconds
    ) `
    -DisableBrowserTrace
