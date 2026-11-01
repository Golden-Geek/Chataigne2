# Produces a test artifact only. Neither this script nor the test requests elevation.
param([string]$ManifestTool)

$ErrorActionPreference = 'Stop'
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
$callerElevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$source = Join-Path $workspace 'target/debug/application_process_fixture.exe'
$output = Join-Path $workspace 'target/debug/application_elevation_required_fixture.exe'
$extracted = Join-Path $workspace 'target/debug/application_elevation_required_fixture.manifest'
if (!(Test-Path -LiteralPath $source -PathType Leaf)) {
    throw 'Build application_process_fixture with application-process-fixture first.'
}
if (!$ManifestTool) {
    $sdkBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
    $ManifestTool = Get-ChildItem -LiteralPath $sdkBin -Directory |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { Join-Path $_.FullName 'x64/mt.exe' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
}
if (!$ManifestTool -or !(Test-Path -LiteralPath $ManifestTool -PathType Leaf)) {
    throw 'Windows SDK mt.exe is required; pass -ManifestTool if it is not installed in the SDK root.'
}
Copy-Item -LiteralPath $source -Destination $output -Force
& $ManifestTool -nologo -manifest (Join-Path $PSScriptRoot 'elevation-required.manifest') "-outputresource:$output;#1"
if ($LASTEXITCODE -ne 0) { throw "Manifest embedding failed: $LASTEXITCODE" }
& $ManifestTool -nologo "-inputresource:$output;#1" "-out:$extracted"
if ($LASTEXITCODE -ne 0) { throw "Manifest extraction failed: $LASTEXITCODE" }
[xml]$manifest = Get-Content -LiteralPath $extracted -Raw
if ($manifest.assembly.trustInfo.security.requestedPrivileges.requestedExecutionLevel.level -ne 'requireAdministrator') {
    throw 'The embedded fixture manifest does not requireAdministrator.'
}
[pscustomobject]@{
    executable = $output
    sha256 = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
    embedded_manifest = $extracted
    execution_level = 'requireAdministrator'
    caller_elevated = $callerElevated
    manifest_tool = $ManifestTool
} | ConvertTo-Json -Compress
