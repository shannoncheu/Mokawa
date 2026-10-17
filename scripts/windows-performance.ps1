param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [ValidateRange(10,86400)][int]$Seconds = 60,
    [ValidateRange(1,100)][int]$Cycles = 3,
    [string]$Output = 'dist/performance.csv'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force (Split-Path $Output) | Out-Null
$samples = [System.Collections.Generic.List[object]]::new()
for ($cycle = 1; $cycle -le $Cycles; $cycle++) {
    # Open the real task page, with no synthetic account or transfer data.
    $process = Start-Process $Executable -ArgumentList '--smoke-page=tasks' -PassThru
    try {
        Start-Sleep -Seconds 3
        $process.Refresh()
        if ($process.HasExited) { throw 'Mokawa exited before measurement' }
        $previousCpu = $process.TotalProcessorTime.TotalSeconds
        $previousTime = [DateTime]::UtcNow
        for ($second = 1; $second -le $Seconds; $second++) {
            Start-Sleep -Seconds 1
            $process.Refresh()
            if ($process.HasExited) { throw 'Mokawa exited while measuring' }
            $now = [DateTime]::UtcNow
            $cpu = $process.TotalProcessorTime.TotalSeconds
            $samples.Add([pscustomobject]@{
                Cycle = $cycle; Second = $second; Timestamp = $now.ToString('o')
                WorkingSetBytes = $process.WorkingSet64
                PrivateBytes = $process.PrivateMemorySize64
                CpuOneCorePercent = 100 * ($cpu - $previousCpu) / ($now - $previousTime).TotalSeconds
                Threads = $process.Threads.Count; Handles = $process.HandleCount
            })
            $previousCpu = $cpu; $previousTime = $now
        }
    } finally {
        if (!$process.HasExited) {
            $null = $process.CloseMainWindow()
            if (!$process.WaitForExit(10000)) {
                $process.Kill()
                throw 'Mokawa did not close within 10 seconds'
            }
        }
        $process.Dispose()
        $samples | Export-Csv -NoTypeInformation -Path $Output
    }
}
$samples | Group-Object Cycle | ForEach-Object {
    $rows = $_.Group
    [pscustomobject]@{
        Cycle = $_.Name
        MeanCpuOneCorePercent = ($rows | Measure-Object CpuOneCorePercent -Average).Average
        FirstPrivateBytes = $rows[0].PrivateBytes; LastPrivateBytes = $rows[-1].PrivateBytes
        MaxWorkingSetBytes = ($rows | Measure-Object WorkingSetBytes -Maximum).Maximum
        MaxThreads = ($rows | Measure-Object Threads -Maximum).Maximum
        FirstHandles = $rows[0].Handles; LastHandles = $rows[-1].Handles
    }
} | ConvertTo-Json | Set-Content -Encoding utf8 ($Output -replace '\.csv$', '.json')
