param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MokawaThemeWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref Point p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint data, UIntPtr extra);
}
'@
$originalAppData = $env:LOCALAPPDATA
$isolated = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
$results = @()
try {
    # Isolate preferences only. This test never creates or exports account credentials.
    $env:LOCALAPPDATA = $isolated
    $directory = Join-Path $isolated 'Mokawa'
    New-Item -ItemType Directory -Force $directory | Out-Null
    foreach ($theme in @('light','dark','system')) {
        if ($theme -ne 'dark') {
            @{theme=$theme; compact_rows=$false; hide_details=$false} | ConvertTo-Json | Set-Content -Encoding utf8NoBOM (Join-Path $directory 'preferences.json')
        } # The dark restart must read the value saved by the previous UI click.
        # No theme override: actual persisted preferences must drive the application.
        $process = Start-Process $Executable -ArgumentList '--smoke-test','--smoke-interactive','--smoke-page=settings' -PassThru
        try {
            for ($i=0; $i -lt 50; $i++) {
                Start-Sleep -Milliseconds 100; $process.Refresh()
                if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
            }
            $handle = $process.MainWindowHandle
            if ($handle -eq [IntPtr]::Zero) { throw 'No theme test window' }
            $null = [MokawaThemeWindow]::SetForegroundWindow($handle)
            Start-Sleep -Seconds 1
            $point = New-Object MokawaThemeWindow+Point
            $point.X=400; $point.Y=22
            $null = [MokawaThemeWindow]::ClientToScreen($handle,[ref]$point)
            $bitmap = New-Object System.Drawing.Bitmap(1,1)
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.CopyFromScreen($point.X,$point.Y,0,0,$bitmap.Size)
                $pixel = $bitmap.GetPixel(0,0)
                $hex = '{0:x2}{1:x2}{2:x2}' -f $pixel.R,$pixel.G,$pixel.B
            } finally { $graphics.Dispose(); $bitmap.Dispose() }
            if ($theme -eq 'light' -and $hex -ne 'f0f3fa') { throw "Light titlebar pixel was $hex" }
            if ($theme -eq 'dark' -and $hex -ne '141621') { throw "Dark titlebar pixel was $hex" }
            if ($theme -eq 'system' -and $hex -notin @('f0f3fa','141621')) { throw "System titlebar pixel was $hex" }
            $results += @{theme=$theme; persistedStartup='passed'; titlebarPixel=$hex}
            if ($theme -eq 'light') {
                # Settings layout at the Runner's default 100% DPI. Click the actual dark button.
                $button = New-Object MokawaThemeWindow+Point
                $button.X=410; $button.Y=234
                $null = [MokawaThemeWindow]::ClientToScreen($handle,[ref]$button)
                $null = [MokawaThemeWindow]::SetCursorPos($button.X,$button.Y)
                Start-Sleep -Milliseconds 200
                [MokawaThemeWindow]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
                [MokawaThemeWindow]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
                Start-Sleep -Milliseconds 600
                $saved = Get-Content (Join-Path $directory 'preferences.json') -Raw | ConvertFrom-Json
                if ($saved.theme -ne 'dark') { throw 'Theme button did not persist dark mode' }
                $bitmap = New-Object System.Drawing.Bitmap(1,1)
                $graphics = [Drawing.Graphics]::FromImage($bitmap)
                try {
                    $graphics.CopyFromScreen($point.X,$point.Y,0,0,$bitmap.Size)
                    $pixel = $bitmap.GetPixel(0,0)
                    $actual = '{0:x2}{1:x2}{2:x2}' -f $pixel.R,$pixel.G,$pixel.B
                    if ($actual -ne '141621') { throw "Live theme switch pixel was $actual" }
                } finally { $graphics.Dispose(); $bitmap.Dispose() }
                $results += @{uiClick='passed'; immediateDarkTitlebar='passed'; automaticSave='passed'}
            }
            $null = $process.CloseMainWindow()
            if (!$process.WaitForExit(10000)) { throw 'Theme window did not close' }
        } finally {
            if (!$process.HasExited) { $process.Kill() }
            $process.Dispose()
        }
    }
} finally {
    $env:LOCALAPPDATA = $originalAppData
    Remove-Item -Recurse -Force $isolated -ErrorAction SilentlyContinue
    $results | ConvertTo-Json | Set-Content -Encoding utf8 dist/theme-results.json
}
