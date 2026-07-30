$ErrorActionPreference = "Continue"

function Run-Test {
    param(
        [string]$Name,
        [scriptblock]$Action
    )
    Write-Host "  -> $Name ... " -NoNewline
    $time = Measure-Command { & $Action 2>&1 | Out-Null }
    $seconds = [Math]::Round($time.TotalSeconds, 3)
    Write-Host "${seconds}s" -ForegroundColor Cyan
    return $seconds
}

function Print-Line {
    Write-Host "------------------------------------------------------------" -ForegroundColor DarkGray
}

Write-Host "`n TURBO vs SCCACHE vs CARGO BENCHMARK" -ForegroundColor Magenta
Print-Line

$results = @{}

# Isolated cache directory so cargo clean does not delete it!
$globalBenchCache = "$PWD\.bench_cache"
if (!(Test-Path $globalBenchCache)) { New-Item -ItemType Directory -Path $globalBenchCache | Out-Null }

# ==========================================
# 1. NATIVE CARGO
# ==========================================
Write-Host "`n[1/3] Testing Native Cargo" -ForegroundColor Yellow

cmd /c "cargo clean >nul 2>&1"
$cargoCold = Run-Test "Cold Build    (Full compilation)" { cmd /c "cargo build -q >nul 2>&1" }

$cargoWarm = Run-Test "Warm / No-op  (No changes)" { cmd /c "cargo build -q >nul 2>&1" }

cmd /c "cargo clean >nul 2>&1"
$cargoRestore = Run-Test "Cache Restore (Empty target dir)" { cmd /c "cargo build -q >nul 2>&1" }

$results["Cargo"] = @{ Cold = $cargoCold; Restore = $cargoRestore; Warm = $cargoWarm }

# ==========================================
# 2. SCCACHE
# ==========================================
if (Get-Command sccache -ErrorAction SilentlyContinue) {
    Write-Host "`n[2/3] Testing Sccache" -ForegroundColor Yellow
    
    $env:RUSTC_WRAPPER = "sccache"
    $env:SCCACHE_DIR = "$globalBenchCache\sccache"
    cmd /c "sccache --stop-server >nul 2>&1"

    cmd /c "cargo clean >nul 2>&1"
    if (Test-Path $env:SCCACHE_DIR) { Remove-Item -Recurse -Force $env:SCCACHE_DIR }
    $sccacheCold = Run-Test "Cold Build    (Empty sccache dir)" { cmd /c "cargo build -q >nul 2>&1" }

    $sccacheWarm = Run-Test "Warm / No-op  (No changes)" { cmd /c "cargo build -q >nul 2>&1" }

    cmd /c "cargo clean >nul 2>&1"
    $sccacheRestore = Run-Test "Cache Restore (Extract from cache)" { cmd /c "cargo build -q >nul 2>&1" }

    $results["Sccache"] = @{ Cold = $sccacheCold; Restore = $sccacheRestore; Warm = $sccacheWarm }
    
    cmd /c "sccache --stop-server >nul 2>&1"
    Remove-Item Env:RUSTC_WRAPPER
    Remove-Item Env:SCCACHE_DIR
} else {
    Write-Host "`n[!] Sccache is not installed. Run 'cargo install sccache' to test it." -ForegroundColor Red
}

# ==========================================
# 3. TURBO
# ==========================================
Write-Host "`n[3/3] Testing Turbo" -ForegroundColor Yellow

$env:TURBO_CACHE_DIR = "$globalBenchCache\turbo"

cmd /c "cargo clean >nul 2>&1"
if (Test-Path $env:TURBO_CACHE_DIR) { Remove-Item -Recurse -Force $env:TURBO_CACHE_DIR }
$turboCold = Run-Test "Cold Build    (Empty turbo cache)" { cmd /c "cargo turbo build -q >nul 2>&1" }

$turboWarm = Run-Test "Warm / No-op  (No changes)" { cmd /c "cargo turbo build -q >nul 2>&1" }

cmd /c "cargo clean >nul 2>&1"
$turboRestore = Run-Test "Cache Restore (Extract from cache)" { cmd /c "cargo turbo build -q >nul 2>&1" }

$results["Turbo"] = @{ Cold = $turboCold; Restore = $turboRestore; Warm = $turboWarm }

Remove-Item Env:TURBO_CACHE_DIR
cmd /c "cargo clean >nul 2>&1"

# Clean up the global isolated cache directory after tests
if (Test-Path $globalBenchCache) { Remove-Item -Recurse -Force $globalBenchCache }

# ==========================================
# TABLE OF RESULTS
# ==========================================
Write-Host ""
Print-Line
Write-Host "RESULTS (in seconds)" -ForegroundColor Green
Print-Line

Write-Host ("{0,-12} | {1,10} | {2,10} | {3,10}" -f "Tool","Cold Build","Restore","Warm (No-op)")
Print-Line

foreach($name in $results.Keys) {
    Write-Host ("{0,-12} | {1,10} | {2,10} | {3,10}" -f $name, "$($results[$name].Cold)s", "$($results[$name].Restore)s", "$($results[$name].Warm)s")
}

# ==========================================
# WINNERS
# ==========================================
Write-Host ""
Print-Line
Write-Host "CATEGORY WINNERS" -ForegroundColor Yellow
Print-Line

$winnerCold = $results.GetEnumerator() | Sort-Object {$_.Value.Cold} | Select-Object -First 1
$winnerRestore = $results.GetEnumerator() | Sort-Object {$_.Value.Restore} | Select-Object -First 1
$winnerWarm = $results.GetEnumerator() | Sort-Object {$_.Value.Warm} | Select-Object -First 1

Write-Host "Fastest Cold Build : $($winnerCold.Key) ($($winnerCold.Value.Cold)s)" -ForegroundColor Cyan
Write-Host "Fastest Restore    : $($winnerRestore.Key) ($($winnerRestore.Value.Restore)s)" -ForegroundColor Cyan
Write-Host "Fastest Warm Build : $($winnerWarm.Key) ($($winnerWarm.Value.Warm)s)" -ForegroundColor Cyan

# ==========================================
# OVERALL RANKING
# ==========================================
$overall = $results.GetEnumerator() | ForEach-Object {
    [PSCustomObject]@{
        Name = $_.Key
        Score = $_.Value.Cold + $_.Value.Restore + $_.Value.Warm
    }
} | Sort-Object Score

Write-Host ""
Print-Line
Write-Host "OVERALL RANKING (Lower time = Better)" -ForegroundColor Green
Print-Line

$place = 1
foreach($entry in $overall) {
    $color = if ($place -eq 1) { "Green" } elseif ($place -eq 2) { "Yellow" } else { "Gray" }
    Write-Host "$place. $($entry.Name) - $([Math]::Round($entry.Score,3)) sec (Total)" -ForegroundColor $color
    $place++
}
Write-Host ""