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
$target = $ast.ParamBlock.Parameters | Where-Object { $_.Name.VariablePath.UserPath -eq 'RestartDeviceInstanceId' }
$validation = $target.Attributes | Where-Object { $_.TypeName.Name -eq 'ValidatePattern' }
$pattern = $validation.PositionalArguments[0].Value
if (-not $pattern) { throw 'Targeted restart requires an explicit identifier constraint' }
if ('PCI\VEN_10DE&DEV_2C59\123ABC' -notmatch $pattern) { throw 'Exact NVIDIA instance rejected' }
foreach ($value in @('*', 'PCI\VEN_8086&DEV_1234\123ABC', 'PCI\VEN_10DE&DEV_2C59\123ABC" /reboot')) {
    if ($value -match $pattern) { throw 'Broad, non-NVIDIA or extra-argument target accepted' }
}
Write-Output '6 readiness and4 target-identifier checks passed; no process or reset launched'
