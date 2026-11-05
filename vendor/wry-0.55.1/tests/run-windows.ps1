param(
    [Parameter(Mandatory = $true)]
    [string]$ManifestTool,
    [switch]$Native
)

$ErrorActionPreference = 'Stop'
$packageRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$workspaceRoot = (Resolve-Path -LiteralPath (Join-Path $packageRoot '../..')).Path
$targetDirectory = Join-Path $workspaceRoot 'target'
$cargoManifest = Join-Path $packageRoot 'Cargo.toml'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Cargo.lock') -Destination (Join-Path $packageRoot 'Cargo.lock')
$savedPreference = $ErrorActionPreference
try {
    # Windows PowerShell treats redirected native stderr as error records, including
    # Cargo's successful progress messages. Its exit code remains the success signal.
    $ErrorActionPreference = 'Continue'
    $records = & cargo test --manifest-path $cargoManifest --locked `
        --target-dir $targetDirectory --lib --no-run --message-format=json
    $cargoExit = $LASTEXITCODE
} finally {
    $ErrorActionPreference = $savedPreference
}
if ($cargoExit -ne 0) { throw 'Wry regression compile failed' }
$artifacts = @($records | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object {
    $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'wry' -and
    $_.profile.test -and $_.executable
})
if ($artifacts.Count -ne 1) { throw 'Expected exactly one compiled Wry test executable' }
$testExecutable = $artifacts[0].executable

# GetWindowSubclass's named export requires Common Controls v6, as the Tauri host does.
# Embed that supported assembly dependency in this standalone unit-test artifact too.
$resourceArgument = '-outputresource:' + $testExecutable + ';#1'
& $ManifestTool -manifest (Join-Path $PSScriptRoot 'windows.manifest') $resourceArgument
if ($LASTEXITCODE -ne 0) { throw 'Wry test manifest embedding failed' }
if ($Native) {
    & $testExecutable hardware_parent_subclass_is_removed --ignored --nocapture
} else {
    & $testExecutable --skip hardware_
}
exit $LASTEXITCODE
