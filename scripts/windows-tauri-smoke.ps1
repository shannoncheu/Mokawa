$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force dist | Out-Null
$installer = Get-ChildItem src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe | Select-Object -First 1
if (!$installer) { throw 'Tauri installer missing' }
$root = "$env:LOCALAPPDATA\Programs\Mokawa-Vue-Test"
$setup = Start-Process $installer.FullName -ArgumentList '/S',"/D=$root" -Wait -PassThru
if ($setup.ExitCode -ne 0) { throw 'Tauri installer failed' }
if (!(Test-Path "$root\mokawa.exe")) { throw 'Installed executable missing' }
try {
    $env:MOKAWA_TEST_EXECUTABLE = "$root\mokawa.exe"
    node scripts/tauri-desktop-test.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Native WebView2 acceptance failed' }
} finally {
    if (Test-Path "$root\uninstall.exe") {
        $uninstall = Start-Process "$root\uninstall.exe" -ArgumentList '/S' -Wait -PassThru
        if ($uninstall.ExitCode -ne 0) { throw 'Tauri uninstall failed' }
    }
}
