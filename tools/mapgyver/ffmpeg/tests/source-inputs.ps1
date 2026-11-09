$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../source-inputs.ps1')
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../../target'))
$taskTest = Join-Path $taskRoot ('source-input-check-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskTest -Force | Out-Null
try {
    $taskExpected = Join-Path $taskTest 'expected'
    [IO.File]::WriteAllBytes($taskExpected, [byte[]]@(1, 2, 3, 4))
    $taskDefinition = @{
        url = 'https://example.invalid/pinned'; bytes = 4
        sha256 = (Get-FileHash -LiteralPath $taskExpected -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    $global:mapgyverSourceInputCalls = 0
    function Invoke-WebRequest($Uri, $OutFile, [switch]$UseBasicParsing) {
        $global:mapgyverSourceInputCalls++
        if ($global:mapgyverSourceInputCalls -lt 3) {
            [IO.File]::WriteAllBytes($OutFile, [byte[]]@(0))
            throw 'Simulated transient transfer failure'
        }
        [IO.File]::WriteAllBytes($OutFile, [byte[]]@(1, 2, 3, 4))
    }
    $taskDestination = Join-Path $taskTest 'download'
    Get-PinnedVideoInput $taskDefinition $taskDestination | Out-Null
    if ($global:mapgyverSourceInputCalls -ne 3) { throw 'Transient input transfer did not recover' }
    Get-PinnedVideoInput $taskDefinition $taskDestination | Out-Null
    if ($global:mapgyverSourceInputCalls -ne 3) { throw 'Valid cached input unexpectedly downloaded' }
    [IO.File]::WriteAllBytes($taskDestination, [byte[]]@(0))
    Get-PinnedVideoInput $taskDefinition $taskDestination | Out-Null
    if ($global:mapgyverSourceInputCalls -ne 4 -or
        @(Get-ChildItem -LiteralPath $taskTest -Filter 'download.rejected-*').Count -ne 1 -or
        @(Get-ChildItem -LiteralPath $taskTest -Filter '*.partial-*').Count -ne 0) {
        throw 'Corrupt cache input was not preserved/recovered atomically'
    }
    Write-Output 'Source input recovery: two transient failures, valid-cache reuse and corrupt-cache recovery pass.'
} finally {
    $taskResolved = [IO.Path]::GetFullPath($taskTest)
    if ([IO.Path]::GetDirectoryName($taskResolved) -ne $taskRoot -or
        [IO.Path]::GetFileName($taskResolved) -notmatch '^source-input-check-[a-f0-9]{32}$') {
        throw 'Refusing cleanup outside the owned source-input test directory'
    }
    Remove-Item -LiteralPath $taskResolved -Recurse -Force
}
