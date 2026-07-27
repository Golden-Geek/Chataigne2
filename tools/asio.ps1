[CmdletBinding()]
param(
    [string] $SdkPath,
    [string] $CacheRoot,
    [switch] $SetupOnly,
    [switch] $PassThru,

    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]] $Command
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows
    )) {
    throw "ASIO is a Windows-only audio host."
}

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$env:CARGO_TARGET_DIR = Join-Path $repositoryRoot "target"

function Find-LibclangDirectory {
    $candidates = [System.Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($env:LIBCLANG_PATH)) {
        $candidates.Add($env:LIBCLANG_PATH)
    }

    $clang = Get-Command "clang" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $clang) {
        $candidates.Add((Split-Path -Parent $clang.Source))
    }

    $candidates.Add((Join-Path $env:ProgramFiles "LLVM\bin"))
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
        $installations = @(& $vswhere -all -products * -property installationPath 2>$null)
        foreach ($installation in $installations) {
            if (-not [string]::IsNullOrWhiteSpace($installation)) {
                $candidates.Add((Join-Path $installation "VC\Tools\Llvm\x64\bin"))
                $candidates.Add((Join-Path $installation "VC\Tools\Llvm\bin"))
            }
        }
    }

    foreach ($candidate in $candidates | Select-Object -Unique) {
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and
            (Test-Path -LiteralPath (Join-Path $candidate "libclang.dll") -PathType Leaf)) {
            return [System.IO.Path]::GetFullPath($candidate)
        }
    }

    throw "ASIO builds require LLVM/Clang with libclang.dll. Install LLVM, then rerun tools/asio.ps1."
}

function Test-VcBuildToolsInstalled {
    if (Get-Command "cl.exe" -ErrorAction SilentlyContinue) {
        return $true
    }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        return $false
    }
    $installation = & $vswhere `
        -latest `
        -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath `
        2>$null
    return ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($installation))
}

if (-not (Test-VcBuildToolsInstalled)) {
    throw "ASIO builds require Visual Studio C++ Build Tools with the VCTools workload."
}

$configureParameters = @{
    PassThru = $true
}
if (-not [string]::IsNullOrWhiteSpace($SdkPath)) {
    $configureParameters.SdkPath = $SdkPath
}
if (-not [string]::IsNullOrWhiteSpace($CacheRoot)) {
    $configureParameters.CacheRoot = $CacheRoot
}
$configuredOutput = @(
    & (Join-Path $repositoryRoot "tools\bootstrap\configure-asio-sdk.ps1") @configureParameters
)
$env:CPAL_ASIO_DIR = [string]($configuredOutput | Select-Object -Last 1)
$env:LIBCLANG_PATH = Find-LibclangDirectory

Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH"

if ($SetupOnly) {
    Write-Host "ASIO local setup is ready. tools/asio.ps1 keeps these variables scoped to each build."
    if ($PassThru) {
        [pscustomobject]@{
            sdk_path = $env:CPAL_ASIO_DIR
            libclang_path = $env:LIBCLANG_PATH
        }
    }
    return
}

if ($null -eq $Command -or $Command.Count -eq 0) {
    $Command = @(
        "cargo",
        "run",
        "-p",
        "golden_audio",
        "--example",
        "backend_probe",
        "--features",
        "asio"
    )
}

$executable = $Command[0]
$arguments = @($Command | Select-Object -Skip 1)
Write-Host "Running with ASIO: $executable $($arguments -join ' ')"
& $executable @arguments
exit $LASTEXITCODE
