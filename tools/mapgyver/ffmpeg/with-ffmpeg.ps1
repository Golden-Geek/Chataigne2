# Keep Cargo flags verbatim; advanced PowerShell parameters would consume -p as -PipelineVariable.
$CargoArguments = $args
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
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
try {
    Set-Location -LiteralPath $taskRoot
    $env:FFMPEG_DIR = $taskInstall
    $env:PATH = (Join-Path $taskInstall 'bin') + ';' + $env:PATH
    # Bindgen's Windows SDK traversal can exhaust the MSVC executable's default main stack.
    # Reserve virtual address space, without increasing committed stack or changing the system.
    if ($taskOldEncodedRustflags) {
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
        & npm.cmd exec -- tauri @taskTauriArguments
    } else {
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
}
exit $taskExit
