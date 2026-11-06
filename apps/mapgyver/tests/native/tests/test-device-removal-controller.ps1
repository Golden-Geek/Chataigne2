$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot '../invoke-device-removal.ps1'
$tokens = $null
$errors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($source, [ref]$tokens, [ref]$errors)
if ($errors.Count -ne 0) { throw 'Controller has PowerShell syntax errors' }
$function = $ast.Find({
    param($node)
    $node -is [Management.Automation.Language.FunctionDefinitionAst] -and
        $node.Name -eq 'Test-OwnedFixtureReady'
}, $true)
if (-not $function) { throw 'Readiness function is missing' }
# Load only the pure predicate, never the controller or its reset branch.
. ([scriptblock]::Create($function.Extent.Text))
foreach ($text in @('', 'unrelated startup output', 'READY_EXTERNAL_DEVICE_REMOVAL pid=7: ready')) {
    $result = Test-OwnedFixtureReady -Text $text -ExpectedProcessId 42
    if ($result -isnot [bool] -or $result) { throw 'Empty/wrong-process readiness was accepted' }
}
$result = Test-OwnedFixtureReady -Text ([string]@()) -ExpectedProcessId 42
if ($result -isnot [bool] -or $result) { throw 'Empty cmdlet collection was accepted' }
$result = Test-OwnedFixtureReady -Text ([string]$null) -ExpectedProcessId 42
if ($result -isnot [bool] -or $result) { throw 'Null cmdlet output was accepted' }
$result = Test-OwnedFixtureReady -Text "startup`nREADY_EXTERNAL_DEVICE_REMOVAL pid=42: ready" -ExpectedProcessId 42
if ($result -isnot [bool] -or -not $result) { throw 'Current-process readiness was rejected' }
Write-Output '6 controller readiness checks passed; no process or reset launched'
