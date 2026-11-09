$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$taskPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'source-build.json') -Raw | ConvertFrom-Json
$taskBase = Join-Path $taskRoot ('target/' + $taskPin.build_directory)
$taskCache = Join-Path $taskBase 'downloads'
$taskSdk = Join-Path (Join-Path $taskBase 'stage') $taskPin.install_name
$taskKit = Join-Path $taskBase 'source-kit'
$taskSource = Join-Path $taskCache 'ffmpeg-source.tar.gz'
$taskCompilerSource = Join-Path $taskCache 'w64devkit-source.tar'
. (Join-Path $PSScriptRoot 'source-inputs.ps1')
Get-PinnedVideoInput @{
    url = $taskPin.toolchain.source_url; bytes = $taskPin.toolchain.source_bytes; sha256 = $taskPin.toolchain.source_sha256
} $taskCompilerSource | Out-Null
foreach ($taskInput in @(
    @{ path = $taskSource; bytes = $taskPin.ffmpeg.bytes; hash = $taskPin.ffmpeg.sha256 },
    @{ path = $taskCompilerSource; bytes = $taskPin.toolchain.source_bytes; hash = $taskPin.toolchain.source_sha256 }
)) {
    if ((Get-Item -LiteralPath $taskInput.path).Length -ne $taskInput.bytes -or
        (Get-FileHash -LiteralPath $taskInput.path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskInput.hash) {
        throw 'Corresponding-source archive mismatch; source kit was not changed'
    }
}
New-Item -ItemType Directory -Path $taskKit -Force | Out-Null
& python (Join-Path $PSScriptRoot 'tests/source_sdk.py') $taskSdk --output (Join-Path $taskKit 'runtime.json')
if ($LASTEXITCODE -ne 0) { throw 'Candidate library identity/configuration check failed' }
Copy-Item -LiteralPath $taskSource -Destination (Join-Path $taskKit 'ffmpeg-source.tar.gz') -Force
Copy-Item -LiteralPath $taskCompilerSource -Destination (Join-Path $taskKit 'w64devkit-source.tar') -Force
$taskRecipe = Join-Path $taskKit 'recipe/tools/mapgyver/ffmpeg'
New-Item -ItemType Directory -Path $taskRecipe -Force | Out-Null
foreach ($taskName in @('source-build.json', 'build-source.ps1', 'source-inputs.ps1', 'prepare-source-kit.ps1', 'extract-notices.py', 'SOURCE-KIT.md')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $taskName) -Destination $taskRecipe -Force
}
New-Item -ItemType Directory -Path (Join-Path $taskRecipe 'tests') -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'tests/source_sdk.py') -Destination (Join-Path $taskRecipe 'tests') -Force
$taskCompiler = Join-Path $taskBase 'tools/w64devkit'
$taskLicense = Join-Path $taskKit 'licenses'
New-Item -ItemType Directory -Path $taskLicense -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $taskSdk 'LICENSE.txt') -Destination (Join-Path $taskLicense 'FFmpeg-LGPL-3.0.txt') -Force
Copy-Item -LiteralPath (Join-Path $taskSdk 'GPL-3.0.txt') -Destination $taskLicense -Force
Copy-Item -LiteralPath (Join-Path $taskCompiler 'COPYING.MinGW-w64-runtime.txt') -Destination $taskLicense -Force
$taskToolRecipe = Join-Path $taskKit 'toolchain-recipe'
New-Item -ItemType Directory -Path $taskToolRecipe -Force | Out-Null
foreach ($taskName in @('Dockerfile', 'README.md', 'VERSION.txt')) {
    Copy-Item -LiteralPath (Join-Path $taskCompiler $taskName) -Destination $taskToolRecipe -Force
}
Copy-Item -LiteralPath (Join-Path $taskCompiler 'src') -Destination $taskToolRecipe -Recurse -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'SOURCE-KIT.md') -Destination (Join-Path $taskKit 'README.md') -Force
& python (Join-Path $PSScriptRoot 'extract-notices.py') $taskCompilerSource $taskLicense
if ($LASTEXITCODE -ne 0) { throw 'Compiler runtime license extraction failed' }
Write-Output "Corresponding-source kit: $taskKit"
