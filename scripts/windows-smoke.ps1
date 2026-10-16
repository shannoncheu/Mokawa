param([Parameter(Mandatory=$true)][string]$Executable, [string]$OutputDirectory = 'dist')
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MokawaWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
}
'@
$results = @()
foreach ($scenario in @('account', 'files', 'tasks', 'settings', 'files-small')) {
    $page = if ($scenario -eq 'files-small') { 'files' } else { $scenario }
    $arguments = @('--smoke-test', "--smoke-page=$page")
    if ($scenario -eq 'files-small') { $arguments += '--smoke-small' }
    $process = $null
    try {
        $process = Start-Process -FilePath $Executable -ArgumentList $arguments -PassThru
        $window = [IntPtr]::Zero
        for ($attempt = 0; $attempt -lt 30; $attempt++) {
            Start-Sleep -Milliseconds 100
            $process.Refresh()
            if ($process.HasExited) { break }
            if ($process.MainWindowHandle -ne [IntPtr]::Zero) { $window = $process.MainWindowHandle; break }
        }
        if ($window -eq [IntPtr]::Zero) { throw 'No native window was observed; the runner may lack a usable GPU desktop.' }
        if ($process.MainWindowTitle -ne 'Mokawa') { throw "Unexpected window title: $($process.MainWindowTitle)" }
        Start-Sleep -Milliseconds 400
        $rect = New-Object MokawaWindow+Rect
        if (![MokawaWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Could not measure native window.' }
        $work = [System.Windows.Forms.Screen]::FromHandle($window).WorkingArea
        if ($rect.Left -lt ($work.Left - 8) -or $rect.Top -lt ($work.Top - 8) -or $rect.Right -gt ($work.Right + 8) -or $rect.Bottom -gt ($work.Bottom + 8)) {
            throw 'Native window extends outside the usable desktop.'
        }
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $bitmap = New-Object System.Drawing.Bitmap($width, $height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
            $bitmap.Save((Join-Path (Resolve-Path $OutputDirectory) "$scenario.png"))
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
        if (!$process.WaitForExit(20000)) { throw 'Smoke process did not exit in time.' }
        if ($process.ExitCode -ne 0) { throw "Smoke process failed: $($process.ExitCode)" }
        $results += @{ scenario=$scenario; status='passed'; width=$width; height=$height; exit_code=$process.ExitCode }
    } catch {
        $results += @{ scenario=$scenario; status='failed'; reason=$_.Exception.Message }
        if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force }
    }
}
$results | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $OutputDirectory 'smoke-results.json')
if (@($results | Where-Object { $_.status -ne 'passed' }).Count -gt 0) { throw 'Some native-window smoke scenarios failed. See smoke-results.json.' }
