param(
    [Parameter(Mandatory = $true)]
    [string]$Installer,
    [string]$Arm64FfmpegSdk,
    [scriptblock]$AfterStartup
)

$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path -LiteralPath $Installer).ProviderPath
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$appRoot = Join-Path $workspace 'apps/mapgyver'
$config = Get-Content -LiteralPath (Join-Path $appRoot 'tauri.conf.json') -Raw | ConvertFrom-Json
$windowsConfig = Get-Content -LiteralPath (Join-Path $appRoot 'tauri.windows.conf.json') -Raw | ConvertFrom-Json
$ffmpeg = Get-Content -LiteralPath (Join-Path $workspace 'tools/mapgyver/ffmpeg/windows-x64.json') -Raw | ConvertFrom-Json
if ($Arm64FfmpegSdk) {
    . (Join-Path $workspace 'tools/mapgyver/ffmpeg/windows-pe.ps1')
    $sdkRoot = (Resolve-Path -LiteralPath $Arm64FfmpegSdk).ProviderPath
    $runtime = foreach ($resource in $windowsConfig.bundle.resources.PSObject.Properties) {
        if ($resource.Name -match '/mapgyver-ffmpeg-runtime/bin/') {
            $name = $resource.Value
            $sdkDll = Get-Item -LiteralPath (Join-Path $sdkRoot "bin/$name")
            $hash = Get-FileHash -LiteralPath $sdkDll.FullName -Algorithm SHA256
            @{ name = $name; bytes = $sdkDll.Length; sha256 = $hash.Hash.ToLowerInvariant() }
        }
    }
    if ($runtime.Count -ne 5) { throw 'Expected five ARM64 FFmpeg runtime DLLs.' }
} else {
    $runtime = $ffmpeg.runtime
}
$uninstallKey = "Software\Microsoft\Windows\CurrentVersion\Uninstall\$($config.productName)"
$userKey = "HKCU:\$uninstallKey"
if ((Test-Path -LiteralPath $userKey) -or (Test-Path -LiteralPath "HKLM:\$uninstallKey")) {
    throw "$($config.productName) is already installed for this account or machine."
}
$publisher = $config.bundle.publisher
if (!$publisher) { $publisher = $config.identifier.Split('.')[1] }
$rememberedSubkey = "Software\$publisher\$($config.productName)"
$rememberedKey = "HKCU:\$rememberedSubkey"
$previousKey = Get-Item -LiteralPath $rememberedKey -ErrorAction SilentlyContinue
$previousLocation = if ($previousKey) { $previousKey.GetValue('') }

$run = Join-Path $workspace ("target/package-smoke/" + [guid]::NewGuid())
$installRoot = Join-Path $run 'Test Install'
$binary = Join-Path $installRoot ($windowsConfig.mainBinaryName + '.exe')
$uninstaller = Join-Path $installRoot 'uninstall.exe'
$shutdown = Join-Path $run 'host.stop'
New-Item -Path $run -ItemType Directory | Out-Null
Write-Host "Installer smoke evidence: $run"

function Invoke-Setup([string]$Path, [string]$Arguments) {
    $process = Start-Process -FilePath $Path -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    if (!$process.WaitForExit(180000)) {
        $process.Kill()
        throw "Timed out: $Path"
    }
    if ($process.ExitCode -ne 0) {
        throw "$Path exited with code $($process.ExitCode)"
    }
}

$app = $null
try {
    # NSIS requires the directory argument last, with the absolute path left unquoted.
    Invoke-Setup $installerPath "/S /D=$installRoot"
    $registration = Get-ItemProperty -LiteralPath $userKey
    if ($registration.DisplayName -cne $config.productName -or
        $registration.DisplayVersion -cne $config.version -or
        $registration.InstallLocation.Trim('"') -ne $installRoot) {
        throw 'Unexpected Windows installer registration.'
    }
    if ((Get-Item -LiteralPath $binary).VersionInfo.ProductName -cne $config.productName) {
        throw 'Unexpected installed executable product name.'
    }
    $running = Get-Process -Name $windowsConfig.mainBinaryName -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $binary }
    if ($running) { throw 'Silent setup launched the app.' }
    if ($Arm64FfmpegSdk -and (Get-WindowsPeMachine $binary) -ne 0xaa64) {
        throw 'Installed MapGyver executable is not Windows ARM64.'
    }
    foreach ($dll in $runtime) {
        $path = Join-Path $installRoot $dll.name
        $hash = Get-FileHash -LiteralPath $path -Algorithm SHA256
        if ($hash.Hash -ne $dll.sha256) { throw "Unexpected installed runtime: $($dll.name)" }
        if ($Arm64FfmpegSdk -and (Get-WindowsPeMachine $path) -ne 0xaa64) {
            throw "Installed FFmpeg library is not Windows ARM64: $($dll.name)"
        }
    }

    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { $address = "127.0.0.1:$($listener.LocalEndpoint.Port)" } finally { $listener.Stop() }
    $overrides = @{
        APPDATA = (Join-Path $run 'appdata')
        LOCALAPPDATA = (Join-Path $run 'localappdata')
        GC_UI_BIND = $address
        GC_UI_FRONTEND_URL = $null
    }
    $saved = @{}
    try {
        foreach ($name in $overrides.Keys) {
            $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
            [Environment]::SetEnvironmentVariable($name, $overrides[$name], 'Process')
        }
        $app = Start-Process -FilePath $binary -WindowStyle Hidden -WorkingDirectory $installRoot -PassThru `
            -ArgumentList "--headless --no-remote --automation-shutdown-file `"$shutdown`"" `
            -RedirectStandardOutput (Join-Path $run 'app.log') -RedirectStandardError (Join-Path $run 'app-error.log')
        # Keep the process handle open so Windows PowerShell can read ExitCode after shutdown.
        $null = $app.Handle
    } finally {
        foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
    }

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if ($app.HasExited) { throw "Installed app exited during startup ($($app.ExitCode))." }
        try {
            $health = Invoke-RestMethod "http://$address/api/ui/health" -TimeoutSec 1
        } catch {
            $health = $null
            Start-Sleep -Milliseconds 100
        }
    } until (($health.backend_ready -and $health.engine_read_model_ready) -or [DateTime]::UtcNow -ge $deadline)
    if (!$health.engine_read_model_ready) { throw 'Installed app did not become ready.' }

    $discovery = Invoke-RestMethod "http://$address/.well-known/golden-ui" -TimeoutSec 5
    if ($discovery.application.display_name -cne $config.productName -or
        $discovery.application.version -cne $config.version) {
        throw 'Unexpected product name or version from the installed workbench.'
    }
    $page = Invoke-WebRequest "http://$address/" -UseBasicParsing -TimeoutSec 5
    if ($page.StatusCode -ne 200 -or $page.Content -notmatch '/_app/immutable/') {
        throw 'Installed app did not serve the embedded workbench.'
    }

    New-Item -Path $shutdown -ItemType File | Out-Null
    if (!$app.WaitForExit(15000)) { throw 'Installed app did not exit after requesting shutdown.' }
    if ($app.ExitCode -ne 0) { throw "Installed app exited with code '$($app.ExitCode)'." }
    Write-Host "Verified $($config.productName) $($config.version): silent install and packaged startup."

    if ($AfterStartup) {
        $runtimePin = Join-Path $run 'video-runtime.json'
        $runtimeJson = @{ runtime = $runtime } | ConvertTo-Json -Depth 4
        [IO.File]::WriteAllText($runtimePin, $runtimeJson)
        & $AfterStartup $binary $run $runtimePin
    }
} finally {
    if ($app -and !$app.HasExited) {
        $app.Kill()
        $app.WaitForExit()
    }
    if (Test-Path -LiteralPath $uninstaller) {
        $copy = Join-Path $run 'uninstall.exe'
        Copy-Item -LiteralPath $uninstaller -Destination $copy
        Invoke-Setup $copy "/S _?=$installRoot"
    }
    # NSIS remembers the last location after removal. Restore it only if this run set it.
    $remembered = Get-Item -LiteralPath $rememberedKey -ErrorAction SilentlyContinue
    if ($remembered -and $remembered.GetValue('') -eq $installRoot) {
        if ($null -ne $previousLocation) {
            Set-Item -LiteralPath $rememberedKey -Value $previousLocation
        } else {
            $writable = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($rememberedSubkey, $true)
            try { $writable.DeleteValue('') } finally { $writable.Dispose() }
        }
        $remembered = Get-Item -LiteralPath $rememberedKey
        if (!$previousKey -and $remembered.ValueCount -eq 0 -and $remembered.SubKeyCount -eq 0) {
            Remove-Item -LiteralPath $rememberedKey
        }
    }
}

if ((Test-Path -LiteralPath $installRoot) -or (Test-Path -LiteralPath $userKey)) {
    throw 'Silent uninstall left installed files or Windows registration.'
}
Write-Host "Verified $($config.productName) $($config.version): silent install, packaged startup, silent uninstall."
