param([Parameter(Mandatory = $true)][string]$Install)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$taskPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'windows-x64.json') -Raw | ConvertFrom-Json
$taskStage = Join-Path $taskRoot 'target/mapgyver-ffmpeg-runtime'
$taskKit = Join-Path $taskRoot $taskPin.source_kit

# Verify every input before changing the managed stage, including an already extracted SDK.
$taskFiles = @()
foreach ($taskDll in $taskPin.runtime) {
    if ($taskDll.name -notmatch '^[a-z0-9-]+\.dll$') { throw 'Invalid pinned runtime filename' }
    $taskSource = Join-Path $Install ('bin/' + $taskDll.name)
    if (!(Test-Path -LiteralPath $taskSource -PathType Leaf) -or
        (Get-Item -LiteralPath $taskSource).Length -ne $taskDll.bytes -or
        (Get-FileHash -LiteralPath $taskSource -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskDll.sha256) {
        throw "Pinned FFmpeg runtime mismatch: $($taskDll.name)"
    }
    $taskFiles += @{ source = $taskSource; target = 'bin/' + $taskDll.name }
}
$taskLicense = Join-Path $Install 'LICENSE.txt'
if ((Get-FileHash -LiteralPath $taskLicense -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskPin.license_sha256) {
    throw 'Pinned FFmpeg license mismatch'
}
$taskFiles += @(
    @{ source = $taskLicense; target = 'licenses/FFmpeg-LGPL-3.0.txt' },
    @{ source = (Join-Path $taskRoot 'LICENSE'); target = 'licenses/GPL-3.0.txt' },
    @{ source = (Join-Path $PSScriptRoot 'NOTICE.txt'); target = 'licenses/VIDEO-NOTICE.txt' },
    @{ source = (Join-Path $taskRoot 'crates/golden_video/vendor/hap/hap.h'); target = 'licenses/HAP-reference-header.h' },
    @{ source = (Join-Path $taskRoot 'crates/golden_video/vendor/hap/snappy-c.h'); target = 'licenses/Snappy-C-header.h' },
    @{ source = (Join-Path $PSScriptRoot 'windows-x64.json'); target = 'licenses/ffmpeg-pin.json' },
    @{ source = (Join-Path $PSScriptRoot 'source-build.json'); target = 'licenses/ffmpeg-source-build.json' },
    @{ source = (Join-Path $taskKit 'README.md'); target = 'licenses/VIDEO-SOURCES.md' },
    @{ source = (Join-Path $taskKit 'licenses/COPYING.MinGW-w64-runtime.txt'); target = 'licenses/MinGW-runtime.txt' },
    @{ source = (Join-Path $taskKit 'licenses/GCC-Runtime-Exception.txt'); target = 'licenses/GCC-Runtime-Exception.txt' },
    @{ source = (Join-Path $taskKit 'licenses/GCC-GPL-3.0.txt'); target = 'licenses/GCC-GPL-3.0.txt' }
)
foreach ($taskFile in $taskFiles) {
    if (!(Test-Path -LiteralPath $taskFile.source -PathType Leaf)) { throw "Missing runtime input: $($taskFile.source)" }
}
foreach ($taskFile in $taskFiles) {
    $taskDestination = Join-Path $taskStage $taskFile.target
    New-Item -ItemType Directory -Path (Split-Path -Parent $taskDestination) -Force | Out-Null
    $taskCopy = !(Test-Path -LiteralPath $taskDestination -PathType Leaf)
    if (!$taskCopy) {
        $taskCopy = (Get-FileHash -LiteralPath $taskDestination -Algorithm SHA256).Hash -ne
            (Get-FileHash -LiteralPath $taskFile.source -Algorithm SHA256).Hash
    }
    if ($taskCopy) { Copy-Item -LiteralPath $taskFile.source -Destination $taskDestination -Force }
}
