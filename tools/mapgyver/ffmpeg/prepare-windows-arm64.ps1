param([Parameter(Mandatory = $true)][string]$Install)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-pe.ps1')
. (Join-Path $PSScriptRoot 'source-inputs.ps1')

$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$stage = Join-Path $root 'target/mapgyver-ffmpeg-runtime-arm64'
$sdkPin = Join-Path $Install 'source-build.json'
$sdk = Get-Content -LiteralPath $sdkPin -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'source-build.json') -Raw | ConvertFrom-Json
$arm64 = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'windows-arm64.json') -Raw | ConvertFrom-Json
if ($sdk.revision -cne $arm64.revision -or $sdk.ffmpeg.commit -cne $source.ffmpeg.commit -or
    $sdk.toolchain.sha256 -cne $arm64.toolchain.sha256 -or
    ($sdk.configure -join "`n") -cne ($arm64.configure -join "`n")) {
    throw "FFmpeg SDK does not match the pinned Windows ARM64 build: $Install"
}

$configPath = Join-Path $root 'apps/mapgyver/tauri.windows.conf.json'
$windowsConfig = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
$resources = @{}
$dlls = @()
foreach ($resource in $windowsConfig.bundle.resources.PSObject.Properties) {
    $prefix = '../../target/mapgyver-ffmpeg-runtime/'
    if (!$resource.Name.StartsWith($prefix, [StringComparison]::Ordinal)) { continue }
    $relative = $resource.Name.Substring($prefix.Length)
    $resources[$resource.Name] = $null
    $resources['../../target/mapgyver-ffmpeg-runtime-arm64/' + $relative] = $resource.Value
    if ($relative -match '^bin/[a-z0-9-]+\.dll$') { $dlls += $relative }
}
if ($dlls.Count -ne 5) { throw 'Expected five FFmpeg runtime DLLs in the Windows bundle.' }
foreach ($dll in $dlls) {
    if ((Get-WindowsPeMachine (Join-Path $Install $dll)) -ne 0xaa64) {
        throw "FFmpeg runtime is not Windows ARM64: $dll"
    }
}
foreach ($library in @('avcodec', 'avformat', 'avutil', 'swresample', 'swscale')) {
    $import = Join-Path $Install "lib/$library.lib"
    if (!(Test-Path -LiteralPath $import -PathType Leaf)) { throw "Missing FFmpeg import library: $import" }
}

$licenses = @{
    'FFmpeg-LGPL-3.0.txt' = (Join-Path $Install 'LICENSE.txt')
    'GPL-3.0.txt' = (Join-Path $Install 'GPL-3.0.txt')
    'ffmpeg-source-build.json' = $sdkPin
    'LLVM-MinGW.txt' = (Join-Path $Install 'compiler-notices/LICENSE.TXT')
    'VIDEO-NOTICE.txt' = (Join-Path $PSScriptRoot 'NOTICE-WINDOWS-ARM64.txt')
    'HAP-reference-header.h' = (Join-Path $root 'crates/golden_video/vendor/hap/hap.h')
    'Snappy-C-header.h' = (Join-Path $root 'crates/golden_video/vendor/hap/snappy-c.h')
}
foreach ($notice in $licenses.Values) {
    if (!(Test-Path -LiteralPath $notice -PathType Leaf)) { throw "Missing video notice: $notice" }
}

$spout = Join-Path $root 'target/mapgyver-spout-arm64'
New-Item -ItemType Directory -Path $spout -Force | Out-Null
$spoutPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../spout/windows-arm64.json') -Raw | ConvertFrom-Json
$licenses['sse2neon.h'] = Get-PinnedVideoInput $spoutPin.sse2neon (Join-Path $spout 'sse2neon.h')

New-Item -ItemType Directory -Path (Join-Path $stage 'bin'), (Join-Path $stage 'licenses') -Force | Out-Null
foreach ($dll in $dlls) { Copy-Item -LiteralPath (Join-Path $Install $dll) -Destination (Join-Path $stage $dll) -Force }
foreach ($notice in $licenses.GetEnumerator()) {
    Copy-Item -LiteralPath $notice.Value -Destination (Join-Path $stage ('licenses/' + $notice.Key)) -Force
}

# The platform file contains the x64 resource keys. JSON Merge Patch removes
# those keys and adds the architecture-specific resources without dropping fonts.
$overlay = @{ bundle = @{ resources = $resources } } | ConvertTo-Json -Depth 4 -Compress
$overlayPath = Join-Path $stage 'tauri.arm64.conf.json'
[IO.File]::WriteAllText($overlayPath, $overlay)
[pscustomobject]@{ ConfigPath = $overlayPath; Overlay = $overlay; SpoutInclude = $spout }
