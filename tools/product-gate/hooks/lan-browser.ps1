. (Join-Path $PSScriptRoot "browser-gate-common.ps1")

$lanAddress = Get-RealNonLoopbackIpv4Address
Write-Host "Using non-loopback IPv4 $($lanAddress.Address) on '$($lanAddress.Interface)' for the LAN product gate."

Invoke-BundledBrowserGate `
    -Id "lan-browser" `
    -BrowserCommand "product-gate-lan" `
    -BindAddress "0.0.0.0" `
    -BrowserHost $lanAddress.Address `
    -Port 7022 `
    -FixtureFileName "lan-browser.noisette" `
    -ExpectedHost $lanAddress.Address
