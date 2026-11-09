param([ValidateRange(1, 32)][int]$Jobs = 12)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$taskPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'source-build.json') -Raw | ConvertFrom-Json
$taskBase = Join-Path $taskRoot 'target/mapgyver-ffmpeg-candidate'
$taskCache = Join-Path $taskBase 'downloads'
New-Item -ItemType Directory -Path $taskCache -Force | Out-Null
. (Join-Path $PSScriptRoot 'source-inputs.ps1')

function FetchPinned($Definition, [string]$Name) {
    $taskPath = Join-Path $taskCache $Name
    Get-PinnedVideoInput $Definition $taskPath
}
$taskSourceArchive = FetchPinned $taskPin.ffmpeg 'ffmpeg-source.tar.gz'
$taskToolArchive = FetchPinned $taskPin.toolchain 'w64devkit.7z.exe'
$taskAsmArchive = FetchPinned $taskPin.assembler 'nasm.zip'
$taskSourceParent = Join-Path $taskBase 'source'
$taskTools = Join-Path $taskBase 'tools'
New-Item -ItemType Directory -Path $taskSourceParent, $taskTools -Force | Out-Null
$taskSource = Join-Path $taskSourceParent $taskPin.ffmpeg.root
if (!(Test-Path -LiteralPath (Join-Path $taskSource 'configure'))) {
    & tar -xf $taskSourceArchive -C $taskSourceParent
    if ($LASTEXITCODE -ne 0) { throw 'FFmpeg source extraction failed' }
}
$taskCompiler = Join-Path $taskTools ($taskPin.toolchain.root + '/bin')
if (!(Test-Path -LiteralPath (Join-Path $taskCompiler 'gcc.exe'))) {
    # Windows' installed libarchive reads this SFX as data; never execute the downloaded SFX.
    & tar -xf $taskToolArchive -C $taskTools
    if ($LASTEXITCODE -ne 0) { throw 'Toolchain archive extraction failed' }
}
$taskAssembler = Join-Path $taskTools $taskPin.assembler.root
if (!(Test-Path -LiteralPath (Join-Path $taskAssembler 'nasm.exe'))) {
    Expand-Archive -LiteralPath $taskAsmArchive -DestinationPath $taskTools
}
$taskStage = Join-Path $taskBase 'stage'
New-Item -ItemType Directory -Path $taskStage -Force | Out-Null
$taskOldPath = $env:PATH
$taskOldEpoch = $env:SOURCE_DATE_EPOCH
$taskOldDirectory = Get-Location
try {
    $env:PATH = $taskCompiler + ';' + $taskAssembler + ';' + $taskOldPath
    $env:SOURCE_DATE_EPOCH = [string]$taskPin.source_date_epoch
    Set-Location -LiteralPath $taskSource
    # FFmpeg's supported release override must not inherit the parent workspace's Git tags.
    $taskRelease = (Get-Content -LiteralPath 'RELEASE' -Raw).Trim()
    if ($taskRelease -ne $taskPin.ffmpeg.version) { throw 'FFmpeg source release identity mismatch' }
    $taskVersion = $taskRelease + '-' + $taskPin.ffmpeg.commit.Substring(0, 12)
    [IO.File]::WriteAllText((Join-Path $taskSource 'VERSION'), $taskVersion + "`n", [Text.UTF8Encoding]::new($false))
    $taskConfigure = @($taskPin.configure)
    & (Join-Path $taskCompiler 'sh.exe') './configure' @taskConfigure
    if ($LASTEXITCODE -ne 0) { throw 'Source FFmpeg configure failed; inspect ffbuild/config.log' }
    # Native Windows need not grant symlink privileges. Copy build-only DLL aliases instead.
    # V=1 bypasses quiet-tag macro prefixes that native GNU Make passes literally to sh.
    $taskMake = @('-r', '-s', 'V=1', 'LN_S=cp -f')
    & (Join-Path $taskCompiler 'make.exe') @taskMake ('-j' + $Jobs)
    if ($LASTEXITCODE -ne 0) { throw 'Source FFmpeg compilation failed' }
    & (Join-Path $taskCompiler 'make.exe') @taskMake 'install' ('DESTDIR=' + $taskStage.Replace('\', '/'))
    if ($LASTEXITCODE -ne 0) { throw 'Source FFmpeg installation failed' }
    $taskSdk = Join-Path $taskStage 'mapgyver-video-sdk'
    foreach ($taskDll in Get-ChildItem -LiteralPath (Join-Path $taskSdk 'bin') -Filter '*.dll') {
        if ($taskDll.Name -notmatch '^(avcodec|avformat|avutil|swresample|swscale)-[0-9]+\.dll$') {
            throw "Unexpected source SDK library: $($taskDll.Name)"
        }
        $taskLibrary = ($taskDll.Name -split '-')[0]
        $taskDefinition = Join-Path $taskSource ('lib' + $taskLibrary + '/' + $taskDll.BaseName + '.def')
        & (Join-Path $taskCompiler 'dlltool.exe') '-m' 'i386:x86-64' '-d' $taskDefinition '-D' $taskDll.Name '-l' (Join-Path $taskSdk ('lib/' + $taskLibrary + '.lib'))
        if ($LASTEXITCODE -ne 0) { throw "MSVC import library generation failed: $taskLibrary" }
    }
    Copy-Item -LiteralPath (Join-Path $taskSource 'COPYING.LGPLv3') -Destination (Join-Path $taskSdk 'LICENSE.txt')
    Copy-Item -LiteralPath (Join-Path $taskSource 'COPYING.GPLv3') -Destination (Join-Path $taskSdk 'GPL-3.0.txt')
    Write-Output "Candidate SDK: $taskSdk"
} finally {
    Set-Location -LiteralPath $taskOldDirectory
    $env:PATH = $taskOldPath
    $env:SOURCE_DATE_EPOCH = $taskOldEpoch
}
