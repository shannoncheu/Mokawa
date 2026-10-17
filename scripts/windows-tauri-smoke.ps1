$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force dist | Out-Null
function Set-Stage([string]$name) {
    Write-Host "Mokawa native acceptance: $name"
    @{stage=$name;timestamp=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -Encoding utf8 dist/tauri-smoke-stage.json
}
function Wait-Child($child,[int]$milliseconds,[string]$name) {
    if (!$child.WaitForExit($milliseconds)) {
        $child.Kill($true)
        throw "$name timed out"
    }
    if ($child.ExitCode -ne 0) { throw "$name failed with exit code $($child.ExitCode)" }
}
$installer = Get-ChildItem src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe | Select-Object -First 1
if (!$installer) { throw 'Tauri installer missing' }
$portable = 'src-tauri/target/x86_64-pc-windows-msvc/release/mokawa.exe'
if (!(Test-Path $portable)) { throw 'Tauri portable executable missing' }
Set-Stage 'portable-executable-window-controls'
./scripts/windows-tauri-controls.ps1 -Executable $portable
$root = "$env:LOCALAPPDATA\Programs\Mokawa-Vue-Test"
Set-Stage 'install'
$setup = Start-Process $installer.FullName -ArgumentList '/S',"/D=$root" -PassThru
Wait-Child $setup 120000 'Tauri installation'
if (!(Test-Path "$root\mokawa.exe")) { throw 'Installed executable missing' }
try {
    $env:MOKAWA_TEST_EXECUTABLE = "$root\mokawa.exe"
    Set-Stage 'webview-personalization'
    $native = Start-Process (Get-Command node).Source -ArgumentList 'scripts/tauri-desktop-test.mjs' -NoNewWindow -PassThru
    Wait-Child $native 180000 'Native WebView2 acceptance'
    Set-Stage 'physical-window-controls'
    ./scripts/windows-tauri-controls.ps1 -Executable "$root/mokawa.exe"
    Set-Stage 'process-tree-performance'
    ./scripts/windows-tauri-performance.ps1 -Executable "$root/mokawa.exe"
} finally {
    Set-Stage 'uninstall'
    if (Test-Path "$root\uninstall.exe") {
        $uninstall = Start-Process "$root\uninstall.exe" -ArgumentList '/S' -PassThru
        Wait-Child $uninstall 60000 'Tauri uninstall'
    }
}
Set-Stage 'passed'
