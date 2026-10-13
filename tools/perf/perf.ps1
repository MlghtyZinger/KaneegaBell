# Measures the release app against CLAUDE.md's performance targets on a
# generated library (see README -> Performance). Opens and closes the app
# several times; run it when the app isn't already open.
#
#   powershell -File tools\perf\perf.ps1 -Data <PERF_ROOT>\data [-Exe <path>] [-Runs 5]
param(
    [Parameter(Mandatory = $true)][string]$Data,
    [string]$Exe = "src-tauri\target\release\emu-frontend.exe",
    [int]$Runs = 5,
    [int]$IdleSeconds = 30
)
$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$Data = (Resolve-Path $Data).Path
if (Get-Process emu-frontend -ErrorAction SilentlyContinue) { throw "Close the app first (emu-frontend is running)." }
$log = Join-Path $Data "perf.log"

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
    $env:EMU_PERF = "1"; $env:EMU_FRONTEND_DATA_DIR = $Data
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
Write-Host ("Cold start to interactive: median {0} ms, best {1} ms, worst {2} ms (target < 1500 ms)" -f $sorted[[int][math]::Floor($sorted.Count / 2)], $sorted[0], $sorted[-1])

# 2. Idle CPU and memory, with the library on screen and nothing happening.
Start-Sleep -Seconds 10
$tree = Get-Tree $last.Id
$cpuStart = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
$t0 = Get-Date
Start-Sleep -Seconds $IdleSeconds
$tree = Get-Tree $last.Id
$cpuEnd = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
$wall = ((Get-Date) - $t0).TotalMilliseconds
$cores = [Environment]::ProcessorCount
$pct = ($cpuEnd - $cpuStart) / $wall * 100
Write-Host ("Idle CPU over {0} s: {1:N2}% of one core ({2:N3}% of all {3} cores), {4} processes" -f $IdleSeconds, $pct, ($pct / $cores), $cores, $tree.Count)
$ws = ($tree | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
$priv = ($tree | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB
$own = Get-Process -Id $last.Id
Write-Host ("Memory: app process {0:N0} MB private; with WebView2 {1:N0} MB private, {2:N0} MB working set" -f ($own.PrivateMemorySize64 / 1MB), $priv, $ws)
Stop-App $last

# 3. Scrolling smoothness (DevTools protocol enabled for this run only).
$r = Start-App @{ WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9333" }
Start-Sleep -Seconds 2
& node (Join-Path $PSScriptRoot "fps.mjs") 9333

# Memory and CPU of the app + WebView2 only (not a running emulator).
function Measure-App($p, [string]$Label, [int]$Seconds = 0) {
    $tree = @(Get-Tree $p.Id | Where-Object { $_.ProcessName -match '^(emu-frontend|msedgewebview2)$' })
    $cpu0 = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
    $t0 = Get-Date
    if ($Seconds) {
        Start-Sleep -Seconds $Seconds
        $tree = @(Get-Tree $p.Id | Where-Object { $_.ProcessName -match '^(emu-frontend|msedgewebview2)$' })
    }
    $cpu = if ($Seconds) {
        $cpu1 = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
        ", CPU {0:N2}% of one core" -f (($cpu1 - $cpu0) / ((Get-Date) - $t0).TotalMilliseconds * 100)
    } else { "" }
    $ws = ($tree | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
    $priv = ($tree | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB
    Write-Host ("{0}: {1:N0} MB private, {2:N0} MB working set{3}" -f $Label, $priv, $ws, $cpu)
}
Measure-App $r.Process "Memory after scrolling the library (app + WebView2)"

# 4. While a game runs. The generated library's PS2 "emulator" is PowerShell
# waiting 25 s; launching goes through the real path (minimize, low memory).
$gameId = (& node (Join-Path $PSScriptRoot "db.mjs") (Join-Path $Data "library.db") game)
if ($gameId) {
    & node (Join-Path $PSScriptRoot "cdp.mjs") 9333 eval "window.__TAURI_INTERNALS__.invoke('launch_game', {id: $gameId}).then(() => 'launched', (e) => String(e))" | Out-Null
    Start-Sleep -Seconds 5
    Measure-App $r.Process "While a game runs (10 s)" 10
    Start-Sleep -Seconds 14
    Measure-App $r.Process "After the game exits"
} else {
    Write-Host "(no stand-in emulator in this library: skipped the game-running check)"
}
Stop-App $r.Process
