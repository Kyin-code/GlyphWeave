# Native Windows process measurement. Fresh paths only; no deletes or shell indirection.
param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Manifest,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [Parameter(Mandatory=$true)][string]$ReportDirectory,
    [ValidateSet('generate-world','quality-report','scale-audit','world-quality-gate','validate-world','export-preview-stream')][string]$Operation='generate-world',
    [ValidateRange(1,86400)][int]$TimeoutSeconds=600,
    [ValidateRange(256,32768)][int]$MaxPrivateMiB=8192
)
$ErrorActionPreference='Stop'
$exe=(Resolve-Path -LiteralPath $Executable).Path
$inputFile=(Resolve-Path -LiteralPath $Manifest).Path
$out=[IO.Path]::GetFullPath($OutputDirectory)
$reports=[IO.Path]::GetFullPath($ReportDirectory)
if(Test-Path -LiteralPath $reports){throw 'Benchmark requires a fresh report directory'}
if($Operation -eq 'generate-world' -and (Test-Path -LiteralPath $out)){throw 'Bake output must be fresh'}
if($Operation -ne 'generate-world' -and -not (Test-Path -LiteralPath (Join-Path $out 'world.json'))){throw 'Audit requires a completed baked world'}
if($out.Contains('"') -or $inputFile.Contains('"')){throw 'Unsupported quote in benchmark path'}
New-Item -ItemType Directory -Path $reports | Out-Null
$stdout=Join-Path $reports 'stdout.log'
$stderr=Join-Path $reports 'stderr.log'
$measure=Join-Path $reports 'process.json'
$manifestData=Get-Content -LiteralPath $inputFile -Raw | ConvertFrom-Json
$os=Get-CimInstance Win32_OperatingSystem
$previousTrace=$env:GLYPHWEAVE_BAKE_TRACE
$env:GLYPHWEAVE_BAKE_TRACE='1'
$clock=[Diagnostics.Stopwatch]::StartNew()
$process=$null
$reason=$null
$lastStage='not-started'
$peakPrivate=0L
$peakWorking=0L
$cpuSeconds=0.0
$sampleCount=0
$lastWrite=-10.0
function Save-Measurement([string]$status,$code) {
    $report=[ordered]@{
        format='glyphweave.process-benchmark';version=1;status=$status;operation=$Operation
        timestampUtc=[DateTime]::UtcNow.ToString('o');pid=if($process){$process.Id}else{$null}
        executable=$exe;executableSha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        manifest=$inputFile;manifestSha256=(Get-FileHash -LiteralPath $inputFile -Algorithm SHA256).Hash
        output=$out;scenes=$manifestData.scenes
        timeoutSeconds=$TimeoutSeconds;maxPrivateMiB=$MaxPrivateMiB
        elapsedSeconds=$clock.Elapsed.TotalSeconds;sampledPeakPrivateBytes=$peakPrivate
        peakWorkingSetBytes=$peakWorking;cpuSeconds=$cpuSeconds;sampleCount=$sampleCount
        lastTraceStage=$lastStage;stopReason=$reason;exitCode=$code
        osTotalMemoryKiB=$os.TotalVisibleMemorySize;osFreeMemoryAtStartKiB=$os.FreePhysicalMemory
        memoryScope='child process only; private bytes sampled every 500ms, working set includes OS peak; excludes renderer and other processes'
    }
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $measure -Encoding utf8
}
try {
    # Quoted native arguments, never cmd /c or an interpolated command shell.
    $arguments=if($Operation -eq 'generate-world'){@('generate-world',('"'+$inputFile+'"'),('"'+$out+'"'))}else{@($Operation,('"'+$out+'"'))}
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    # Force open a process handle while live, so exit code remains readable.
    $null=$process.Handle
    Save-Measurement 'running' $null
    while(-not $process.HasExited){
        $process.Refresh()
        if($process.HasExited){break}
        $sampleCount++
        $peakPrivate=[Math]::Max($peakPrivate,$process.PrivateMemorySize64)
        $peakWorking=[Math]::Max($peakWorking,$process.PeakWorkingSet64)
        $cpuSeconds=$process.TotalProcessorTime.TotalSeconds
        if($clock.Elapsed.TotalSeconds-$lastWrite -ge 5){
            if(Test-Path -LiteralPath $stderr){
                foreach($line in Get-Content -LiteralPath $stderr -Tail 16){
                    if($line.StartsWith('{')){try{$trace=$line|ConvertFrom-Json;if($trace.format -eq 'glyphweave.bake-trace'){$lastStage=$trace.stage}}catch{}}
                }
            }
            Save-Measurement 'running' $null
            $lastWrite=$clock.Elapsed.TotalSeconds
        }
        if($peakPrivate -gt $MaxPrivateMiB*1MB){$reason='private-memory-budget-exceeded'}
        elseif($clock.Elapsed.TotalSeconds -gt $TimeoutSeconds){$reason='elapsed-time-budget-exceeded'}
        if($reason){
            # This exact Process object belongs to this invocation. Never kill by name.
            $process.Kill();$process.WaitForExit();break
        }
        Start-Sleep -Milliseconds 500
    }
    $process.WaitForExit();$clock.Stop()
    $code=$process.ExitCode
    $status=if($reason){'budget-stopped'}elseif($code -eq 0){if($Operation -eq 'generate-world'){'baked'}else{'completed'}}else{'failed'}
    if(Test-Path -LiteralPath $stderr){foreach($line in Get-Content -LiteralPath $stderr -Tail 30){if($line.StartsWith('{')){try{$trace=$line|ConvertFrom-Json;if($trace.format -eq 'glyphweave.bake-trace'){$lastStage=$trace.stage}}catch{}}}}
    Save-Measurement $status $code
    $files=if(Test-Path -LiteralPath $out){@(Get-ChildItem -LiteralPath $out -Recurse -File)}else{@()}
    $disk=[ordered]@{format='glyphweave.bake-disk-evidence';files=$files.Count;bytes=($files|Measure-Object Length -Sum).Sum;naturalChunks=@($files|Where-Object Name -like '*.natural.height.bin').Count;finalChunks=@($files|Where-Object Name -match '^chunk-.*(?<!natural)\.height\.bin$').Count;worldIndexPresent=(Test-Path -LiteralPath (Join-Path $out 'world.json'));generationReportPresent=(Test-Path -LiteralPath (Join-Path $out 'generation-report.json'))}
    $disk|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $reports 'disk.json') -Encoding utf8
    Get-Content -LiteralPath $measure
    Get-Content -LiteralPath (Join-Path $reports 'disk.json')
    if($status -notin @('baked','completed')){exit 1}
    exit 0
 } catch {
    $reason='measurement-error: '+$_.Exception.Message
    if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()}
    $clock.Stop()
    Save-Measurement 'measurement-failed' $null
    throw
} finally {
    if($null -eq $previousTrace){Remove-Item Env:GLYPHWEAVE_BAKE_TRACE -ErrorAction SilentlyContinue}else{$env:GLYPHWEAVE_BAKE_TRACE=$previousTrace}
    if($process){$process.Dispose()}
}
