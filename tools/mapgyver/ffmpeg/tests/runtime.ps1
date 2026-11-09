param([Parameter(Mandatory = $true)][string]$Install)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$taskHelper = Join-Path $PSScriptRoot '../prepare-runtime.ps1'
$taskPin = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../windows-x64.json') -Raw | ConvertFrom-Json
$taskStage = Join-Path $taskRoot 'target/mapgyver-ffmpeg-runtime'
function StageSnapshot {
    @(Get-ChildItem -LiteralPath $taskStage -Recurse -File | Sort-Object FullName | ForEach-Object {
        [ordered]@{
            path = $_.FullName.Substring($taskStage.Length)
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            written = $_.LastWriteTimeUtc.Ticks
        }
    }) | ConvertTo-Json -Compress
}
& $taskHelper -Install $Install
$taskBefore = StageSnapshot
& $taskHelper -Install $Install
if ((StageSnapshot) -ne $taskBefore) { throw 'Unchanged runtime staging must preserve hashes and timestamps' }

$taskTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$taskFixture = Join-Path $taskTemp ('MapGyverVideoRuntime-' + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path (Join-Path $taskFixture 'bin') -Force | Out-Null
    foreach ($taskDll in $taskPin.runtime) {
        Copy-Item -LiteralPath (Join-Path $Install ('bin/' + $taskDll.name)) -Destination (Join-Path $taskFixture 'bin')
    }
    Copy-Item -LiteralPath (Join-Path $Install 'LICENSE.txt') -Destination $taskFixture
    $taskLast = Join-Path $taskFixture ('bin/' + $taskPin.runtime[-1].name)
    [IO.File]::WriteAllBytes($taskLast, [byte[]]@(0))
    $taskRejected = $false
    try { & $taskHelper -Install $taskFixture } catch {
        if ($_.Exception.Message -notlike 'Pinned FFmpeg runtime mismatch:*') { throw }
        $taskRejected = $true
    }
    if (!$taskRejected -or (StageSnapshot) -ne $taskBefore) { throw 'Corrupt last DLL must reject before staging changes' }
    Copy-Item -LiteralPath (Join-Path $Install ('bin/' + $taskPin.runtime[-1].name)) -Destination $taskLast -Force
    [IO.File]::WriteAllBytes((Join-Path $taskFixture 'LICENSE.txt'), [byte[]]@(0))
    $taskRejected = $false
    try { & $taskHelper -Install $taskFixture } catch {
        if ($_.Exception.Message -ne 'Pinned FFmpeg license mismatch') { throw }
        $taskRejected = $true
    }
    if (!$taskRejected -or (StageSnapshot) -ne $taskBefore) { throw 'Corrupt license must reject before staging changes' }
    Write-Output 'Runtime staging: idempotent; corrupt extracted DLL/license rejected; stage unchanged.'
} finally {
    $taskResolved = [IO.Path]::GetFullPath($taskFixture)
    if ([IO.Path]::GetDirectoryName($taskResolved) -ne $taskTemp.TrimEnd('\') -or
        [IO.Path]::GetFileName($taskResolved) -notmatch '^MapGyverVideoRuntime-[a-f0-9]{32}$') {
        throw 'Refusing cleanup outside the owned runtime test directory'
    }
    Remove-Item -LiteralPath $taskResolved -Recurse -Force
}
