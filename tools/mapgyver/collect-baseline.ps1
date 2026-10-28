# Read-only M00 inventory. Emit JSON to stdout; never launch a window or change display settings.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path

function Read-Inventory {
    param([scriptblock]$Read)
    try {
        $items = @(& $Read)
        return [ordered]@{ status = 'observed'; items = $items }
    } catch {
        return [ordered]@{ status = 'unavailable'; reason = $_.Exception.Message }
    }
}

function Read-ToolVersion {
    param([string]$Name, [string[]]$Arguments)
    $output = @(& $Name @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "$Name exited with $LASTEXITCODE : $($output -join ' ')"
    }
    return $output -join "`n"
}

Push-Location $workspaceRoot
try {
    $lockPath = Join-Path $workspaceRoot 'Cargo.lock'
    $lockedHost = @(
        (Get-Content $lockPath -Raw) -split '\[\[package\]\]' |
            Where-Object {
                $_ -match '(?m)^name = "(tauri|tauri-runtime-wry|wry|webview2-com|windows|windows-core|raw-window-handle)"'
            } | ForEach-Object {
                $name = [regex]::Match($_, '(?m)^name = "([^"]+)"').Groups[1].Value
                $version = [regex]::Match($_, '(?m)^version = "([^"]+)"').Groups[1].Value
                [ordered]@{ name = $name; version = $version }
            }
    )

    $inventory = [ordered]@{
        schema_version = 1
        captured_at_utc = [DateTime]::UtcNow.ToString('o')
        evidence_kind = 'read_only_inventory_not_native_qualification'
        checkout = [ordered]@{
            head = Read-ToolVersion git @('rev-parse', 'HEAD')
            branch = Read-ToolVersion git @('branch', '--show-current')
            status = @(git status --short)
            cargo_lock_sha256 = (Get-FileHash $lockPath -Algorithm SHA256).Hash.ToLowerInvariant()
            npm_lock_sha256 = (Get-FileHash 'package-lock.json' -Algorithm SHA256).Hash.ToLowerInvariant()
        }
        operating_system = Read-Inventory {
            Get-CimInstance Win32_OperatingSystem |
                Select-Object Caption, Version, BuildNumber, OSArchitecture
            Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' |
                Select-Object DisplayVersion, CurrentBuild, UBR
        }
        cpu = Read-Inventory {
            Get-CimInstance Win32_Processor |
                Select-Object Name, NumberOfCores, NumberOfLogicalProcessors
        }
        memory = Read-Inventory {
            Get-CimInstance Win32_ComputerSystem | Select-Object TotalPhysicalMemory
        }
        power = [ordered]@{
            active_scheme = Read-Inventory { Read-ToolVersion powercfg @('/getactivescheme') }
            battery = Read-Inventory {
                Get-CimInstance Win32_Battery | Select-Object BatteryStatus, EstimatedChargeRemaining
            }
        }
        video_controllers = Read-Inventory {
            Get-CimInstance Win32_VideoController |
                Select-Object Name, DriverVersion,
                    @{Name = 'DriverDate'; Expression = { $_.DriverDate.ToString('yyyy-MM-dd') }},
                    CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate
        }
        monitors = Read-Inventory {
            Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorConnectionParams |
                Select-Object Active, VideoOutputTechnology
        }
        webview2 = Read-Inventory {
            $registryPaths = @(
                'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\*'
                'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\*'
                'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\*'
            )
            # Missing registry hives are normal; no matching registration is an explicit failure.
            $runtimes = @(Get-ItemProperty $registryPaths -ErrorAction SilentlyContinue |
                Where-Object { $_.name -like '*WebView*' } | Select-Object name, pv -Unique)
            if ($runtimes.Count -eq 0) { throw 'No WebView2 Runtime registration found' }
            $runtimes
        }
        toolchain = [ordered]@{
            rustc = Read-Inventory { Read-ToolVersion rustc @('-Vv') }
            cargo = Read-Inventory { Read-ToolVersion cargo @('-V') }
            rustup = Read-Inventory { Read-ToolVersion rustup @('show', 'active-toolchain') }
            node = Read-Inventory { Read-ToolVersion node @('--version') }
            npm = Read-Inventory { Read-ToolVersion npm @('--version') }
            cmake = Read-Inventory { Read-ToolVersion cmake @('--version') }
            visual_studio = Read-Inventory {
                $vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
                $installations = Read-ToolVersion $vswherePath @(
                    '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-format', 'json'
                ) | ConvertFrom-Json
                if (@($installations).Count -eq 0) { throw 'No x64 MSVC tool installation found by vswhere' }
                $installations | Select-Object displayName, installationVersion
            }
        }
        locked_host_packages = $lockedHost
        spout_related_processes = Read-Inventory {
            # Process names are only a hint, not SDK enumeration or evidence of a usable sender.
            Get-Process | Where-Object { $_.ProcessName -match 'spout|mapgyver' } | Select-Object ProcessName
        }
        not_measured = @(
            'DXGI adapter LUID, dedicated/shared memory budget and D3D11 feature level'
            'DisplayConfig identity, physical origins, per-monitor DPI and WebView rasterization scale'
            'WebView underlay/DOM layering, pointer capture and layout alignment'
            'Spout SDK enumeration, GPU sharing, resize/restart and adapter compatibility'
            'Frame timing, readback/copy behavior, resource retirement and device recovery'
        )
    }
    $inventory | ConvertTo-Json -Depth 8
} finally {
    Pop-Location
}
