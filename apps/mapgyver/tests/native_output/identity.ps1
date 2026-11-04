param(
    [Parameter(Mandatory = $true)][int]$AppProcessId,
    [Parameter(Mandatory = $true)][string]$ArtifactDirectory
)

$ErrorActionPreference = 'Stop'
$ownedProcess = Get-Process -Id $AppProcessId
if ($ownedProcess.ProcessName -notlike 'MapGyver-codex-*') {
    throw 'Identity inspection requires an isolated MapGyver test process'
}
$ownedProcess.Refresh()
$ownedWindow = $ownedProcess.MainWindowHandle
if ($ownedWindow -eq [IntPtr]::Zero) { throw 'Owned test window is unavailable' }

# Read only the owned window's icon resources; this sends no keyboard/mouse input.
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MapGyverIdentityInspection {
    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, UIntPtr parameter,
        IntPtr argument, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", EntryPoint = "GetClassLongPtrW", SetLastError = true)]
    public static extern IntPtr GetClassLongPtr(IntPtr window, int index);
}
'@

$iconResults = @()
foreach ($iconType in @(0, 1)) {
    $iconHandle = [UIntPtr]::Zero
    $messageResult = [MapGyverIdentityInspection]::SendMessageTimeoutW(
        $ownedWindow, 0x007F, [UIntPtr]::new([uint32]$iconType), [IntPtr]::Zero, 2, 1000, [ref]$iconHandle)
    if ($messageResult -eq [IntPtr]::Zero) { throw 'Owned window icon read timed out' }
    $nativeHandle = [IntPtr]([long]$iconHandle.ToUInt64())
    if ($nativeHandle -eq [IntPtr]::Zero) {
        $classIndex = if ($iconType -eq 0) { -34 } else { -14 }
        $nativeHandle = [MapGyverIdentityInspection]::GetClassLongPtr($ownedWindow, $classIndex)
    }
    $iconSource = 'window'
    if ($nativeHandle -eq [IntPtr]::Zero) {
        # When Tauri/Tao supplies no large HICON, inspect the executable resource separately.
        $iconSource = 'executable-resource-fallback'
        $borrowedIcon = [System.Drawing.Icon]::ExtractAssociatedIcon($ownedProcess.Path)
    } else {
        $borrowedIcon = [System.Drawing.Icon]::FromHandle($nativeHandle)
    }
    $bitmap = $borrowedIcon.ToBitmap()
    $artifact = Join-Path $ArtifactDirectory "window-icon-$iconType.png"
    $bitmap.Save($artifact, [System.Drawing.Imaging.ImageFormat]::Png)
    $iconResults += @{ type = $iconType; source = $iconSource; width = $bitmap.Width; height = $bitmap.Height; artifact = $artifact }
    $bitmap.Dispose()
    $borrowedIcon.Dispose()
}
@{ process = $AppProcessId; title = $ownedProcess.MainWindowTitle; icons = $iconResults } | ConvertTo-Json -Depth 4 -Compress
