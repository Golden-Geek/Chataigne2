# Keep Cargo flags verbatim; advanced PowerShell parameters would consume -p as -PipelineVariable.
$CargoArguments = $args
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$taskTarget = $env:CARGO_BUILD_TARGET
$taskSeparator = [Array]::IndexOf($CargoArguments, '--')
if ($taskSeparator -lt 0) { $taskSeparator = $CargoArguments.Count }
$taskExplicitTarget = $false
for ($i = 0; $i -lt $taskSeparator; $i++) {
    $option = $CargoArguments[$i]
    if ($option -match '^(--target|-t)=(.*)$') {
        $taskExplicitTarget = $true
        $taskTarget = $Matches[2]
    } elseif ($option -eq '--target' -or ($CargoArguments[0] -eq '--tauri' -and $option -eq '-t')) {
        if ($i + 1 -ge $taskSeparator) { throw "Missing value for $option" }
        $taskExplicitTarget = $true
        $taskTarget = $CargoArguments[++$i]
    }
}
if (!$taskTarget -and [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') {
    $taskTarget = 'aarch64-pc-windows-msvc'
}
$taskArm64 = $taskTarget -eq 'aarch64-pc-windows-msvc'
if ($taskArm64) {
    if (!$env:MAPGYVER_FFMPEG_ARM64_SDK) {
        throw 'Set MAPGYVER_FFMPEG_ARM64_SDK to the Windows ARM64 SDK produced by build-windows-arm64.sh.'
    }
    $taskInstall = (Resolve-Path -LiteralPath $env:MAPGYVER_FFMPEG_ARM64_SDK).ProviderPath
    $taskRuntime = & (Join-Path $PSScriptRoot 'prepare-windows-arm64.ps1') -Install $taskInstall
} else {
    $taskPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'windows-x64.json') -Raw | ConvertFrom-Json
    if ($taskPin.source_build -ne 'source-build.json') { throw 'Invalid FFmpeg source manifest' }
    $taskSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot $taskPin.source_build) -Raw | ConvertFrom-Json
    if ($taskSource.build_directory -notmatch '^[a-z0-9-]+$' -or $taskSource.install_name -notmatch '^[a-z0-9-]+$') {
        throw 'Invalid FFmpeg source directory'
    }
    $taskInstall = Join-Path (Join-Path $taskRoot ('target/' + $taskSource.build_directory + '/stage')) $taskSource.install_name
    $taskReady = $true
    $taskRequired = @(
        'include/libavcodec/avcodec.h', 'include/libavformat/avformat.h', 'include/libavutil/frame.h',
        'include/libswresample/swresample.h', 'include/libswscale/swscale.h',
        'lib/avcodec.lib', 'lib/avformat.lib', 'lib/avutil.lib', 'lib/swresample.lib', 'lib/swscale.lib', 'LICENSE.txt'
    )
    foreach ($taskRelative in $taskRequired) {
        if (!(Test-Path -LiteralPath (Join-Path $taskInstall $taskRelative) -PathType Leaf)) { $taskReady = $false }
    }
    $taskLicense = Join-Path $taskInstall 'LICENSE.txt'
    if ((Test-Path -LiteralPath $taskLicense -PathType Leaf) -and
        (Get-FileHash -LiteralPath $taskLicense -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskPin.license_sha256) {
        $taskReady = $false
    }
    $taskCompilerNotice = Join-Path $taskRoot ('target/' + $taskSource.build_directory + '/tools/' + $taskSource.toolchain.root + '/COPYING.MinGW-w64-runtime.txt')
    if (!(Test-Path -LiteralPath $taskCompilerNotice -PathType Leaf)) { $taskReady = $false }
    foreach ($taskDll in $taskPin.runtime) {
        $taskPath = Join-Path $taskInstall ('bin/' + $taskDll.name)
        if (!(Test-Path -LiteralPath $taskPath -PathType Leaf) -or
            (Get-Item -LiteralPath $taskPath).Length -ne $taskDll.bytes -or
            (Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskDll.sha256) {
            $taskReady = $false
        }
    }
    if (!$taskReady) { & (Join-Path $PSScriptRoot 'build-source.ps1') | Out-Host }
    foreach ($taskRelative in $taskRequired) {
        if (!(Test-Path -LiteralPath (Join-Path $taskInstall $taskRelative) -PathType Leaf)) {
            throw "Incomplete pinned FFmpeg install: $taskRelative"
        }
    }
    & (Join-Path $PSScriptRoot 'prepare-source-kit.ps1') | Out-Host
    & (Join-Path $PSScriptRoot 'prepare-runtime.ps1') -Install $taskInstall
}
if (!$CargoArguments.Count) {
    Write-Output $taskInstall
    exit 0
}
$taskOldDirectory = Get-Location
$taskOldFfmpeg = $env:FFMPEG_DIR
$taskOldPath = $env:PATH
$taskOldClang = $env:LIBCLANG_PATH
$taskOldRustflags = $env:RUSTFLAGS
$taskOldEncodedRustflags = $env:CARGO_ENCODED_RUSTFLAGS
$taskOldLinkOptions = $env:_LINK_
$taskOldTarget = $env:CARGO_BUILD_TARGET
$taskOldTauriConfig = $env:TAURI_CONFIG
$taskOldSpoutInclude = $env:SPOUT_SSE2NEON_INCLUDE
try {
    Set-Location -LiteralPath $taskRoot
    $env:FFMPEG_DIR = $taskInstall
    $env:PATH = (Join-Path $taskInstall 'bin') + ';' + $env:PATH
    if ($taskArm64) {
        $env:CARGO_BUILD_TARGET = $taskTarget
        $env:SPOUT_SSE2NEON_INCLUDE = $taskRuntime.SpoutInclude
    }
    # Bindgen needs more reserved stack for Windows SDK traversal. An explicit
    # Cargo target excludes host build scripts from RUSTFLAGS. MSVC reads _LINK_
    # when linking both the host build script and the ARM64 app.
    if ($taskArm64) {
        $env:_LINK_ = ($taskOldLinkOptions + ' /STACK:16777216').Trim()
    } elseif ($taskOldEncodedRustflags) {
        $env:CARGO_ENCODED_RUSTFLAGS = $taskOldEncodedRustflags + [char]31 + '-C' + [char]31 + 'link-arg=/STACK:16777216'
    } else {
        $env:RUSTFLAGS = (($taskOldRustflags + ' -C link-arg=/STACK:16777216').Trim())
    }
    if (!$env:LIBCLANG_PATH -and (Test-Path -LiteralPath 'C:/Program Files/LLVM/bin/libclang.dll')) {
        $env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
    }
    if ($CargoArguments[0] -eq '--tauri') {
        Set-Location -LiteralPath (Join-Path $taskRoot 'apps/mapgyver')
        $taskTauriArguments = @($CargoArguments | Select-Object -Skip 1)
        $taskBundleNdi = !$taskArm64 -and $taskTauriArguments -notcontains '--without-bundled-ndi'
        $taskTauriArguments = @($taskTauriArguments | Where-Object { $_ -ne '--without-bundled-ndi' })
        if ($taskBundleNdi -and ($taskTauriArguments[0] -eq 'bundle' -or
            ($taskTauriArguments[0] -eq 'build' -and $taskTauriArguments -notcontains '--no-bundle'))) {
            & (Join-Path $taskRoot 'tools/mapgyver/ndi/prepare-runtime.ps1')
            $taskTauriArguments += @('--config', 'tauri.ndi.conf.json')
        }
        if ($taskArm64) {
            $taskOptions = @('--config', $taskRuntime.ConfigPath)
            if (!$taskExplicitTarget) { $taskOptions += @('--target', $taskTarget) }
            $taskTauriArguments = @($taskTauriArguments[0]) + $taskOptions + @($taskTauriArguments | Select-Object -Skip 1)
        }
        & npm.cmd exec -- tauri @taskTauriArguments
    } else {
        if ($taskArm64) {
            $taskConfig = if ($taskOldTauriConfig) { $taskOldTauriConfig | ConvertFrom-Json } else { [pscustomobject]@{} }
            if (!$taskConfig.bundle) { $taskConfig | Add-Member NoteProperty bundle ([pscustomobject]@{}) -Force }
            if (!$taskConfig.bundle.resources) {
                $taskConfig.bundle | Add-Member NoteProperty resources ([pscustomobject]@{}) -Force
            }
            $taskOverlay = $taskRuntime.Overlay | ConvertFrom-Json
            foreach ($resource in $taskOverlay.bundle.resources.PSObject.Properties) {
                $taskConfig.bundle.resources | Add-Member NoteProperty $resource.Name $resource.Value -Force
            }
            $env:TAURI_CONFIG = $taskConfig | ConvertTo-Json -Depth 100 -Compress
        }
        & cargo @CargoArguments
    }
    $taskExit = $LASTEXITCODE
} finally {
    Set-Location -LiteralPath $taskOldDirectory
    $env:FFMPEG_DIR = $taskOldFfmpeg
    $env:PATH = $taskOldPath
    $env:LIBCLANG_PATH = $taskOldClang
    $env:RUSTFLAGS = $taskOldRustflags
    $env:CARGO_ENCODED_RUSTFLAGS = $taskOldEncodedRustflags
    $env:_LINK_ = $taskOldLinkOptions
    $env:CARGO_BUILD_TARGET = $taskOldTarget
    $env:TAURI_CONFIG = $taskOldTauriConfig
    $env:SPOUT_SSE2NEON_INCLUDE = $taskOldSpoutInclude
}
exit $taskExit
