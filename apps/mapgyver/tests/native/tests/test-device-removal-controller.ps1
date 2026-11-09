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

$cycle = $ast.Find({
    param($node)
    $node -is [Management.Automation.Language.FunctionDefinitionAst] -and
        $node.Name -eq 'Invoke-TargetedDeviceCycle'
}, $true)
if (-not $cycle) { throw 'Targeted cycle function is missing' }
. ([scriptblock]::Create($cycle.Extent.Text))
# Mock the three operating-system boundaries; loading this function cannot run a utility.
function Invoke-PnpDeviceAction {
    param($Action, $InstanceId, $Directory)
    if ($InstanceId -ne 'exact-nvidia') { throw 'Cycle changed its admitted target' }
    $script:actions.Add([string]$Action)
    if ($Action -eq 'disable' -and $script:throwDisable) { throw 'disable interrupted' }
    if ($Action -eq 'disable') { return $script:disableCode }
    return $script:enableCode
}
function Get-PnpDeviceProperty {
    param($InstanceId, $KeyName)
    if ($script:throwProperty) { throw 'status unavailable' }
    return @{ Data = $script:problemCode }
}
function Get-PnpDevice {
    param([switch]$PresentOnly, $InstanceId, $ErrorAction)
    return @{ Status = 'OK' }
}
function Start-Sleep {
    param($Seconds, $Milliseconds)
    if ($Seconds -eq 7) { $script:dwelt = $true }
    if ($script:interrupt) { throw 'interrupted dwell' }
}
$cases = @(
    'success', 'disable-failure', 'disable-throws', 'property-throws',
    'not-disabled', 'interrupted', 'enable-failure'
)
foreach ($case in $cases) {
    $script:actions = [Collections.Generic.List[string]]::new()
    $script:disableCode = if ($case -eq 'disable-failure') { 1 } else { 0 }
    $script:enableCode = if ($case -eq 'enable-failure') { 1 } else { 0 }
    $script:problemCode = if ($case -eq 'not-disabled') { 0 } else { 22 }
    $script:interrupt = $case -eq 'interrupted'
    $script:throwDisable = $case -eq 'disable-throws'
    $script:throwProperty = $case -eq 'property-throws'
    $script:dwelt = $false
    $metadata = @{}
    $failed = $false
    try { Invoke-TargetedDeviceCycle -InstanceId exact-nvidia -Directory unused -Metadata $metadata }
    catch { $failed = $true }
    if (($script:actions -join ',') -ne 'disable,enable') { throw "$case did not restore its exact target" }
    if ($failed -ne ($case -ne 'success')) { throw "$case returned an incorrect terminal outcome" }
    if ($case -eq 'success' -and (-not $script:dwelt -or $metadata.disabled_problem_code -ne 22 -or -not $metadata.reenabled_utc)) {
        throw 'Successful cycle lacks disabled/dwell/restored witnesses'
    }
    if ($case -eq 'enable-failure' -and $metadata.reenabled_utc) { throw 'Failed restore claimed success' }
}
Write-Output '7 device-cycle restoration/terminal checks passed with mocked OS boundaries'

$missingBinary = Join-Path $PSScriptRoot ('missing-' + [guid]::NewGuid().ToString('N') + '.exe')
$validId = 'PCI\VEN_10DE&DEV_2C59\123ABC'
$invalidCycles = @(
    @{ CycleDevice = $true; UseCurrentToken = $true },
    @{ CycleDevice = $true; RestartDeviceInstanceId = $validId },
    @{ CycleDevice = $true; UseCurrentToken = $true; RestartDeviceInstanceId = $validId }
)
for ($index = 0; $index -lt $invalidCycles.Count; $index++) {
    $arguments = $invalidCycles[$index]
    $message = ''
    try { & $source -TestExecutable $missingBinary @arguments }
    catch { $message = $_.Exception.Message }
    $expected = if ($index -lt 2) {
        'Device cycle requires an elevated current token and an exact NVIDIA target'
    } else {
        'Targeted device restart requires explicit reset authorization'
    }
    if ($message -ne $expected) { throw 'Invalid cycle arguments reached later controller work' }
}
Write-Output '3 cycle admission checks passed before filesystem/process/device work'
