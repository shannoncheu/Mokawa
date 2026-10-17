param([Parameter(Mandatory=$true)][string]$Executable,[int]$Seconds=15,[int]$Cycles=3)
$ErrorActionPreference='Stop'
$rows=[System.Collections.Generic.List[object]]::new()
for($cycle=1;$cycle -le $Cycles;$cycle++) {
    Write-Host "Mokawa process-tree sampling cycle $cycle / $Cycles"
    $process=Start-Process $Executable -PassThru
    try {
        Start-Sleep -Seconds 3
        $previousCpu=$null; $previousTime=$null; $previousIds=''
        for($second=1;$second -le $Seconds;$second++) {
            Start-Sleep -Seconds 1
            $process.Refresh(); if($process.HasExited){throw 'Native process exited during idle sample'}
            $all=Get-CimInstance Win32_Process
            $ids=[System.Collections.Generic.HashSet[int]]::new();$null=$ids.Add($process.Id)
            do {
                $added=$false
                foreach($entry in $all){if($ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)){$added=$true}}
            } while($added)
            $members=@(Get-Process | Where-Object {$ids.Contains($_.Id)})
            $key=($members.Id | Sort-Object) -join ','
            $cpu=($members | ForEach-Object {$_.TotalProcessorTime.TotalSeconds} | Measure-Object -Sum).Sum
            $now=[DateTime]::UtcNow
            $percent=if($previousTime -and $key -eq $previousIds){[Math]::Max(0,100*($cpu-$previousCpu)/($now-$previousTime).TotalSeconds)}else{$null}
            $rows.Add([pscustomobject]@{
                Cycle=$cycle;Second=$second;Timestamp=$now.ToString('o');Page='login';Processes=$members.Count
                WorkingSetBytes=($members | Measure-Object WorkingSet64 -Sum).Sum
                PrivateBytes=($members | Measure-Object PrivateMemorySize64 -Sum).Sum
                Handles=($members | Measure-Object HandleCount -Sum).Sum
                Threads=($members | ForEach-Object {$_.Threads.Count} | Measure-Object -Sum).Sum
                CpuOneCorePercent=$percent
            })
            if ($second % 5 -eq 0) {
                Write-Host "Mokawa cycle $cycle sample $second / $Seconds"
                $rows | Export-Csv -NoTypeInformation dist/tauri-performance.csv
            }
            $previousCpu=$cpu;$previousTime=$now;$previousIds=$key
        }
    } finally {
        if(!$process.HasExited){$null=$process.CloseMainWindow();if(!$process.WaitForExit(10000)){$process.Kill();throw 'Tauri process failed to close'}}
        $process.Dispose();$rows | Export-Csv -NoTypeInformation dist/tauri-performance.csv
    }
}
$rows | Group-Object Cycle | ForEach-Object {
    $samples=$_.Group
    [pscustomobject]@{Cycle=$_.Name;MaxWorkingSetBytes=($samples | Measure-Object WorkingSetBytes -Maximum).Maximum;FirstPrivateBytes=$samples[0].PrivateBytes;LastPrivateBytes=$samples[-1].PrivateBytes;FirstHandles=$samples[0].Handles;LastHandles=$samples[-1].Handles;MaxThreads=($samples | Measure-Object Threads -Maximum).Maximum;MaxProcesses=($samples | Measure-Object Processes -Maximum).Maximum;MeanCpuOneCorePercent=($samples | Measure-Object CpuOneCorePercent -Average).Average}
} | ConvertTo-Json | Set-Content -Encoding utf8 dist/tauri-performance.json
