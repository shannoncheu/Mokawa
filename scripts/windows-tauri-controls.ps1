param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MokawaTauriControls {
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref Point p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
}
'@
function Move-ToClient($handle, $x, $y) {
    $point = New-Object MokawaTauriControls+Point
    $point.X = $x; $point.Y = $y
    $null = [MokawaTauriControls]::ClientToScreen($handle, [ref]$point)
    $null = [MokawaTauriControls]::SetCursorPos($point.X, $point.Y)
    Start-Sleep -Milliseconds 250
    return $point
}
function Click-Pointer {
    [MokawaTauriControls]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    [MokawaTauriControls]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
}
$process = Start-Process $Executable -PassThru
try {
    for ($i=0; $i -lt 50; $i++) {
        Start-Sleep -Milliseconds 100; $process.Refresh()
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
    }
    $handle = $process.MainWindowHandle
    if ($handle -eq [IntPtr]::Zero) { throw 'No Mokawa window' }
    $null = [MokawaTauriControls]::SetForegroundWindow($handle)
    Start-Sleep -Seconds 2
    $rect = New-Object MokawaTauriControls+Rect
    $null = [MokawaTauriControls]::GetClientRect($handle,[ref]$rect)
    $width = $rect.Right
    # Tauri buttons are HTML hit targets; test actual mouse actions, not GPUI HT codes.
    $null = Move-ToClient $handle ($width-67) 22
    Click-Pointer
    Start-Sleep -Milliseconds 500
    if (![MokawaTauriControls]::IsZoomed($handle)) { throw 'Maximize button did not maximize' }
    $null = [MokawaTauriControls]::GetClientRect($handle,[ref]$rect)
    $null = Move-ToClient $handle ($rect.Right-67) 22
    Click-Pointer
    Start-Sleep -Milliseconds 500
    if ([MokawaTauriControls]::IsZoomed($handle)) { throw 'Restore button did not restore' }
    # Double-click the drag area, then restore again.
    $null = Move-ToClient $handle 140 22
    Click-Pointer; Start-Sleep -Milliseconds 80; Click-Pointer
    Start-Sleep -Milliseconds 500
    if (![MokawaTauriControls]::IsZoomed($handle)) { throw 'Caption double click did not maximize' }
    $null = [MokawaTauriControls]::ShowWindow($handle,9)
    Start-Sleep -Milliseconds 300
    $before = New-Object MokawaTauriControls+Rect
    $null = [MokawaTauriControls]::GetWindowRect($handle,[ref]$before)
    $point = Move-ToClient $handle 140 22
    [MokawaTauriControls]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    $null = [MokawaTauriControls]::SetCursorPos($point.X+24,$point.Y+20)
    Start-Sleep -Milliseconds 300
    [MokawaTauriControls]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 300
    $after = New-Object MokawaTauriControls+Rect
    $null = [MokawaTauriControls]::GetWindowRect($handle,[ref]$after)
    if ($after.Left -eq $before.Left -and $after.Top -eq $before.Top) { throw 'Caption drag did not move window' }
    $null = [MokawaTauriControls]::GetClientRect($handle,[ref]$rect)
    $null = Move-ToClient $handle ($rect.Right-112) 22
    Click-Pointer
    Start-Sleep -Milliseconds 500
    if (![MokawaTauriControls]::IsIconic($handle)) { throw 'Minimize did not minimize' }
    $null = [MokawaTauriControls]::ShowWindow($handle,9)
    Start-Sleep -Milliseconds 300
    $null = Move-ToClient $handle ($rect.Right-22) 22
    Click-Pointer
    if (!$process.WaitForExit(10000)) { throw 'Close button did not exit' }
    if ($process.ExitCode -ne 0) { throw 'Window controls process exited with error' }
    @{status='passed';  maximize=$true; restore=$true; doubleClick=$true; drag=$true; minimize=$true; close=$true} | ConvertTo-Json | Set-Content -Encoding utf8 dist/tauri-window-controls.json
} finally {
    if (!$process.HasExited) { $process.Kill() }
    $process.Dispose()
}
