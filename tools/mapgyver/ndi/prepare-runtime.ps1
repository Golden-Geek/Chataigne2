param(
    [string]$SdkDirectory = $env:NDI_SDK_DIR,
    [string]$InstallerLicense = $env:MAPGYVER_NDI_INSTALLER_LICENSE
)

$ErrorActionPreference = 'Stop'
$runtimeDll = 'Processing.NDI.Lib.x64.dll'
$runtimeNotices = 'Processing.NDI.Lib.Licenses.txt'

if (!$SdkDirectory -or !(Test-Path -LiteralPath $SdkDirectory -PathType Container)) {
    throw 'Set NDI_SDK_DIR to the current standard NDI SDK installation to bundle MapGyver with NDI.'
}
$sdkBin = Join-Path $SdkDirectory 'Bin/x64'
foreach ($file in @($runtimeDll, $runtimeNotices)) {
    if (!(Test-Path -LiteralPath (Join-Path $sdkBin $file) -PathType Leaf)) {
        throw "Missing $file in the NDI SDK x64 Bin directory ($sdkBin)."
    }
}
if (!$InstallerLicense -or !(Test-Path -LiteralPath $InstallerLicense -PathType Leaf) -or
    [IO.Path]::GetExtension($InstallerLicense) -ne '.txt' -or
    (Get-Item -LiteralPath $InstallerLicense).Length -eq 0) {
    throw 'Set MAPGYVER_NDI_INSTALLER_LICENSE to the approved plain-text installer agreement for distributing this SDK.'
}

$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$stage = Join-Path $workspace 'target/mapgyver-ndi-runtime'
New-Item -Path $stage -ItemType Directory -Force | Out-Null
foreach ($file in @($runtimeDll, $runtimeNotices)) {
    Copy-Item -LiteralPath (Join-Path $sdkBin $file) -Destination (Join-Path $stage $file)
}
Copy-Item -LiteralPath $InstallerLicense -Destination (Join-Path $stage 'installer-license.txt')
