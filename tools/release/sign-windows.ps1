param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$ArtifactPath
)

$ErrorActionPreference = 'Stop'
$thumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT
if ([string]::IsNullOrWhiteSpace($thumbprint)) {
    if ($env:GC_REQUIRE_SIGNING -eq '1') {
        throw 'WINDOWS_CERTIFICATE_THUMBPRINT is required for a signed Windows package'
    }
    Write-Host "[release] leaving unsigned local artifact: $ArtifactPath"
    exit 0
}

$timestampUrl = $env:WINDOWS_TIMESTAMP_URL
if ([string]::IsNullOrWhiteSpace($timestampUrl)) {
    throw 'WINDOWS_TIMESTAMP_URL is required when Windows signing is enabled'
}

$signTool = $env:TAURI_WINDOWS_SIGNTOOL_PATH
if ([string]::IsNullOrWhiteSpace($signTool)) {
    $signTool = (Get-Command signtool.exe -ErrorAction Stop).Source
}

& $signTool sign /sha1 $thumbprint /fd SHA256 /tr $timestampUrl /td SHA256 $ArtifactPath
if ($LASTEXITCODE -ne 0) {
    throw "signtool failed with exit code $LASTEXITCODE"
}
