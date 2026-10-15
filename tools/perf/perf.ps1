# Measures the release app against CLAUDE.md's performance targets on a
# generated library (see README -> Performance), and gates regressions.
# Opens and closes the app several times; run it when KB isn't open.
#
#   powershell -File tools\perf\perf.ps1 -Data <PERF_ROOT>\data [-Exe <path>] [-Runs 5]
#       [-SaveBaseline]             save this run as the baseline (tools\perf\baseline.json)
#       [-Baseline <file>]          compare with another baseline file
#
# Every run is compared with the baseline (when there is one): the script
# fails (exit 1) if any number got more than 10% worse, beyond a small
# absolute tolerance per metric for measurement noise. Results of the last
# run are in <Data>\perf-last.json.
param(
    [Parameter(Mandatory = $true)][string]$Data,
    [string]$Exe = "src-tauri\target\release\KaneegaBell.exe",
    [int]$Runs = 5,
    [int]$IdleSeconds = 30,
    [string]$Baseline = "",
    [switch]$SaveBaseline,
    # Compare the last run (<Data>\perf-last.json) with the baseline without measuring again.
    [switch]$CompareOnly
)
$ErrorActionPreference = "Stop"
# ($PSScriptRoot is empty in parameter defaults on Windows PowerShell 5.1.)
if (-not $Baseline) { $Baseline = Join-Path $PSScriptRoot "baseline.json" }
# A PATH entry with a stray quote breaks node lookups; drop such entries and
# make sure Node is reachable (this process only).
$env:Path = (($env:Path -split ';') | Where-Object { $_ -and $_ -notmatch '"' }) -join ';'
if (-not (Get-Command node -ErrorAction SilentlyContinue) -and (Test-Path "C:\Program Files\nodejs\node.exe")) { $env:Path += ";C:\Program Files\nodejs" }
$Exe = (Resolve-Path $Exe).Path
$Data = (Resolve-Path $Data).Path
if (Get-Process KaneegaBell, emu-frontend -ErrorAction SilentlyContinue) { throw "Close the app first (KaneegaBell is running)." }
$log = Join-Path $Data "perf.log"
$results = [ordered]@{}
if ($CompareOnly) {
    $last = Get-Content (Join-Path $Data "perf-last.json") -Raw | ConvertFrom-Json
    foreach ($p in $last.PSObject.Properties) { $results[$p.Name] = $p.Value }
}

# The app plus every process it started (WebView2's browser, renderer, GPU...).
function Get-Tree([int]$RootId) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
    $ids = @($RootId); $frontier = @($RootId)
    while ($frontier.Count) {
        $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId } | ForEach-Object { $_.ProcessId })
        $ids += $next; $frontier = $next
    }
    $ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
}

function Start-App([hashtable]$Env) {
    Remove-Item $log -ErrorAction SilentlyContinue
    $env:KB_PERF = "1"; $env:KB_DATA_DIR = $Data
    foreach ($k in $Env.Keys) { Set-Item "env:$k" $Env[$k] }
    $p = Start-Process -FilePath $Exe -PassThru
    foreach ($k in $Env.Keys) { Remove-Item "env:$k" }
    $deadline = (Get-Date).AddSeconds(30)
    while (-not (Test-Path $log) -or -not (Select-String -Path $log -Pattern '^interactive' -Quiet)) {
        if ((Get-Date) -gt $deadline) { Stop-Process -Id $p.Id -Force; throw "the app didn't report 'interactive' within 30 s" }
        Start-Sleep -Milliseconds 50
    }
    $ms = [int]((Select-String -Path $log -Pattern '^interactive (\d+)').Matches[0].Groups[1].Value)
    return @{ Process = $p; Ms = $ms }
}

function Stop-App($p) {
    $p.CloseMainWindow() | Out-Null
    if (-not $p.WaitForExit(5000)) { Stop-Process -Id $p.Id -Force }
    Start-Sleep -Milliseconds 800 # let WebView2 shut down before the next start
}

# Memory and CPU of the app + WebView2 only (not a running emulator).
function Measure-App($p, [string]$Label, [int]$Seconds = 0) {
    $tree = @(Get-Tree $p.Id | Where-Object { $_.ProcessName -match '^(KaneegaBell|msedgewebview2)$' })
    $cpu0 = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
    $t0 = Get-Date
    $cpu = $null
    if ($Seconds) {
        Start-Sleep -Seconds $Seconds
        $tree = @(Get-Tree $p.Id | Where-Object { $_.ProcessName -match '^(KaneegaBell|msedgewebview2)$' })
        $cpu1 = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
        $cpu = ($cpu1 - $cpu0) / ((Get-Date) - $t0).TotalMilliseconds * 100
    }
    $ws = ($tree | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
    $priv = ($tree | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB
    $tm = Get-TaskManagerMB $tree
    $cpuText = if ($null -ne $cpu) { ", CPU {0:N2}% of one core" -f $cpu } else { "" }
    Write-Host ("{0}: {1:N0} MB private, {2:N0} MB working set, {3:N0} MB in Task Manager{4}" -f $Label, $priv, $ws, $tm, $cpuText)
    return @{ PrivateMB = $priv; TaskManagerMB = $tm; Cpu = $cpu }
}

# What Task Manager's Memory column shows for these processes (their private
# working sets; memory shared between them isn't counted).
function Get-TaskManagerMB($tree) {
    $ids = @($tree | ForEach-Object { $_.Id })
    $sum = (Get-CimInstance Win32_PerfRawData_PerfProc_Process | Where-Object { $ids -contains [int]$_.IDProcess } | Measure-Object -Property WorkingSetPrivate -Sum).Sum
    return $sum / 1MB
}

if (-not $CompareOnly) {
$games = (& node (Join-Path $PSScriptRoot "db.mjs") (Join-Path $Data "library.db") count)
Write-Host "Library: $games games ($Data)"

# 1. Cold start to interactive (process start -> library grid painted).
$times = @()
for ($i = 1; $i -le $Runs; $i++) {
    $r = Start-App @{}
    $times += $r.Ms
    Write-Host ("  start {0}: {1} ms" -f $i, $r.Ms)
    if ($i -lt $Runs) { Stop-App $r.Process } else { $last = $r.Process }
}
$sorted = $times | Sort-Object
$median = $sorted[[int][math]::Floor($sorted.Count / 2)]
Write-Host ("Cold start to interactive: median {0} ms, best {1} ms, worst {2} ms (target < 1500 ms)" -f $median, $sorted[0], $sorted[-1])
$results.startMedianMs = $median

# 2. Idle CPU and memory, with the library on screen and nothing happening.
Start-Sleep -Seconds 10
# Several windows, middle value: one-off Windows activity (a scan, an update
# check of some other app) shouldn't decide the number.
$tree = Get-Tree $last.Id
$windows = @()
for ($w = 0; $w -lt 5; $w++) {
    $cpuStart = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
    $t0 = Get-Date
    Start-Sleep -Seconds ([math]::Max(2, [int]($IdleSeconds / 5)))
    $tree = Get-Tree $last.Id
    $cpuEnd = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
    $windows += ($cpuEnd - $cpuStart) / ((Get-Date) - $t0).TotalMilliseconds * 100
}
$cores = [Environment]::ProcessorCount
$pct = ($windows | Sort-Object)[2]
Write-Host ("  idle windows: {0}" -f (($windows | ForEach-Object { "{0:N2}" -f $_ }) -join ", "))
Write-Host ("Idle CPU (middle of 5 windows over {0} s): {1:N2}% of one core ({2:N3}% of all {3} cores), {4} processes" -f $IdleSeconds, $pct, ($pct / $cores), $cores, $tree.Count)
$priv = ($tree | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB
$ws = ($tree | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
$own = Get-Process -Id $last.Id
$tm = Get-TaskManagerMB $tree
Write-Host ("Memory: app process {0:N0} MB private; with WebView2 {1:N0} MB private, {2:N0} MB working set, {3:N0} MB in Task Manager" -f ($own.PrivateMemorySize64 / 1MB), $priv, $ws, $tm)
$results.idleCpuPct = [math]::Round($pct, 2)
$results.startMemMB = [math]::Round($priv)
$results.startTaskManagerMB = [math]::Round($tm)
Stop-App $last

# 3. Scrolling smoothness (DevTools protocol enabled for this run only).
$r = Start-App @{ WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9333" }
Start-Sleep -Seconds 2
$fps = & node (Join-Path $PSScriptRoot "fps.mjs") 9333
$fps | Where-Object { $_ -notmatch '^RESULT ' } | ForEach-Object { Write-Host $_ }
$scroll = ($fps | Where-Object { $_ -match '^RESULT ' } | Select-Object -First 1) -replace '^RESULT ', '' | ConvertFrom-Json
foreach ($pass in "first", "second") {
    $results["scroll${pass}P99Ms"] = $scroll.$pass.p99ms
    $results["scroll${pass}MaxMs"] = $scroll.$pass.maxMs
}
$m = Measure-App $r.Process "Memory after scrolling the library (app + WebView2)"
$results.scrollMemMB = [math]::Round($m.PrivateMB)
$results.scrollTaskManagerMB = [math]::Round($m.TaskManagerMB)

# 4. While a game runs. The generated library's PS2 "emulator" is PowerShell
# waiting 25 s; launching goes through the real path (minimize, low memory).
$gameId = (& node (Join-Path $PSScriptRoot "db.mjs") (Join-Path $Data "library.db") game)
if ($gameId) {
    & node (Join-Path $PSScriptRoot "cdp.mjs") 9333 eval "window.__TAURI_INTERNALS__.invoke('launch_game', {id: $gameId}).then(() => 'launched', (e) => String(e))" | Out-Null
    Start-Sleep -Seconds 4
    # Three 5 s windows while the stand-in game runs; the middle CPU value counts.
    $g = @(1..3 | ForEach-Object { Measure-App $r.Process "While a game runs (5 s window $_)" 5 })
    $results.gameCpuPct = [math]::Round((($g | ForEach-Object { $_.Cpu }) | Sort-Object)[1], 2)
    $results.gameMemMB = [math]::Round($g[2].PrivateMB)
    $results.gameTaskManagerMB = [math]::Round($g[2].TaskManagerMB)
    Start-Sleep -Seconds 9
    $m = Measure-App $r.Process "After the game exits"
    $results.afterGameMemMB = [math]::Round($m.PrivateMB)
    $results.afterGameTaskManagerMB = [math]::Round($m.TaskManagerMB)
} else {
    Write-Host "(no stand-in emulator in this library: skipped the game-running check)"
}
Stop-App $r.Process
$results | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $Data "perf-last.json")
}

# ------------------------------------------------------------------ gate
if ($SaveBaseline) {
    $results | ConvertTo-Json | Set-Content -Encoding utf8 $Baseline
    Write-Host "`nSaved as the baseline: $Baseline" -ForegroundColor Green
    exit 0
}
if (-not (Test-Path $Baseline)) {
    Write-Host "`nNo baseline yet; save one with -SaveBaseline." -ForegroundColor Yellow
    exit 0
}
# Lower is better for every metric. A regression is > 10% worse AND worse
# by more than the metric's noise floor: Windows counts process CPU time in
# ~15.6 ms steps (one step in a 5 s window reads as 0.31% of a core), and
# frame times jump by a few ms with the compositor and the first decode of
# thumbnails, while every frame stays far under the 16.7 ms of 60 fps.
$floor = @{ startMedianMs = 60; idleCpuPct = 1.0; startMemMB = 20; scrollfirstP99Ms = 5; scrollfirstMaxMs = 10; scrollsecondP99Ms = 5; scrollsecondMaxMs = 10; scrollMemMB = 20; gameCpuPct = 1.0; gameMemMB = 20; afterGameMemMB = 20; startTaskManagerMB = 20; scrollTaskManagerMB = 20; gameTaskManagerMB = 20; afterGameTaskManagerMB = 20 }
$base = Get-Content $Baseline -Raw | ConvertFrom-Json
$failed = @()
Write-Host "`nCompared with the baseline ($Baseline):"
foreach ($k in $results.Keys) {
    $old = $base.$k
    if ($null -eq $old) { continue }
    $new = $results[$k]
    $worse = $new -gt $old * 1.10 -and ($new - $old) -gt $floor[$k]
    $change = if ($old -ne 0) { "{0:+0;-0}%" -f (($new - $old) / $old * 100) } else { "" }
    $line = "  {0,-18} {1,8} -> {2,8} {3}" -f $k, $old, $new, $change
    if ($worse) { Write-Host "$line  REGRESSION" -ForegroundColor Red; $failed += $k } else { Write-Host $line }
}
if ($failed.Count) {
    Write-Host "`nPerf gate FAILED: $($failed -join ', ') got more than 10% worse." -ForegroundColor Red
    exit 1
}
Write-Host "`nPerf gate passed." -ForegroundColor Green
