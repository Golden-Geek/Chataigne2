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
foreach ($taskRequired in @('include/libavcodec/avcodec.h', 'lib/avcodec.lib', 'bin/avcodec-63.dll', 'LICENSE.txt')) {
    if (!(Test-Path -LiteralPath (Join-Path $taskInstall $taskRequired))) {
        throw "Incomplete pinned FFmpeg install: $taskRequired"
    }
}
if (!$CargoArguments.Count) {
    Write-Output $taskInstall
    exit 0
}
$taskOldDirectory = Get-Location
$taskOldFfmpeg = $env:FFMPEG_DIR
$taskOldPath = $env:PATH
$taskOldClang = $env:LIBCLANG_PATH
try {
    Set-Location -LiteralPath $taskRoot
    $env:FFMPEG_DIR = $taskInstall
    $env:PATH = (Join-Path $taskInstall 'bin') + ';' + $env:PATH
    if (!$env:LIBCLANG_PATH -and (Test-Path -LiteralPath 'C:/Program Files/LLVM/bin/libclang.dll')) {
        $env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
    }
    & cargo @CargoArguments
    $taskExit = $LASTEXITCODE
} finally {
    Set-Location -LiteralPath $taskOldDirectory
    $env:FFMPEG_DIR = $taskOldFfmpeg
    $env:PATH = $taskOldPath
    $env:LIBCLANG_PATH = $taskOldClang
}
exit $taskExit
