# Keep Cargo flags verbatim; advanced PowerShell parameters would consume -p as -PipelineVariable.
$CargoArguments = $args
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$taskPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'windows-x64.json') -Raw | ConvertFrom-Json
if ($taskPin.root -notmatch '^[a-zA-Z0-9._-]+$') { throw 'Invalid FFmpeg root name' }
$taskTarget = Join-Path $taskRoot 'target'
$taskArchive = Join-Path $taskTarget ($taskPin.root + '.zip')
$taskUnpack = Join-Path $taskTarget 'ffmpeg-pinned'
$taskInstall = Join-Path $taskUnpack $taskPin.root
New-Item -ItemType Directory -Path $taskTarget -Force | Out-Null
if (!(Test-Path -LiteralPath $taskArchive)) {
    Invoke-WebRequest -Uri $taskPin.url -OutFile $taskArchive
}
$taskHash = (Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($taskHash -ne $taskPin.sha256 -or (Get-Item -LiteralPath $taskArchive).Length -ne $taskPin.bytes) {
    throw 'Pinned FFmpeg archive mismatch; retain it for inspection and fetch the named artifact again'
}
if (!(Test-Path -LiteralPath $taskInstall)) {
    Expand-Archive -LiteralPath $taskArchive -DestinationPath $taskUnpack
}
foreach ($taskRequired in @('include/libavcodec/avcodec.h', 'lib/avcodec.lib', 'bin/avcodec-63.dll', 'lib/swscale.lib', 'bin/swscale-10.dll', 'LICENSE.txt')) {
    if (!(Test-Path -LiteralPath (Join-Path $taskInstall $taskRequired))) {
        throw "Incomplete pinned FFmpeg install: $taskRequired"
    }
}
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
