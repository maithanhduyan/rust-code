# Apex Benchmark Script for Windows
# ==================================

param(
    [int]$Duration = 30,
    [int]$Threads = 4,
    [int]$Connections = 100,
    [string]$Target = "http://localhost:8080/"
)

$ErrorActionPreference = "Stop"

function Write-Header {
    param([string]$Text)
    Write-Host ""
    Write-Host "=" * 60 -ForegroundColor Cyan
    Write-Host " $Text" -ForegroundColor Cyan
    Write-Host "=" * 60 -ForegroundColor Cyan
}

function Test-Command {
    param([string]$Command)
    $null = Get-Command $Command -ErrorAction SilentlyContinue
    return $?
}

# Check for benchmarking tools
$hasWrk = Test-Command "wrk"
$hasHey = Test-Command "hey"
$hasCurl = Test-Command "curl"

if (-not $hasWrk -and -not $hasHey) {
    Write-Host "Warning: Neither 'wrk' nor 'hey' found. Install one for proper benchmarking." -ForegroundColor Yellow
    Write-Host "  - wrk: https://github.com/wg/wrk"
    Write-Host "  - hey: https://github.com/rakyll/hey"
    
    if ($hasCurl) {
        Write-Host ""
        Write-Host "Using curl for basic connectivity test..." -ForegroundColor Yellow
    }
}

Write-Header "Apex Benchmark"
Write-Host "Target:      $Target"
Write-Host "Duration:    ${Duration}s"
Write-Host "Threads:     $Threads"
Write-Host "Connections: $Connections"

# Connectivity test
Write-Header "Connectivity Test"
if ($hasCurl) {
    curl -s -o $null -w "HTTP Status: %{http_code}`nTime: %{time_total}s`n" $Target
} else {
    try {
        $response = Invoke-WebRequest -Uri $Target -UseBasicParsing -TimeoutSec 5
        Write-Host "HTTP Status: $($response.StatusCode)"
    } catch {
        Write-Host "Error: Could not connect to $Target" -ForegroundColor Red
        exit 1
    }
}

# Run benchmark
if ($hasWrk) {
    Write-Header "Running wrk benchmark"
    wrk -t$Threads -c$Connections -d"${Duration}s" --latency $Target
} elseif ($hasHey) {
    Write-Header "Running hey benchmark"
    hey -c $Connections -z "${Duration}s" $Target
} else {
    Write-Header "Basic Load Test (using curl)"
    Write-Host "Running $Connections parallel requests..."
    
    $jobs = @()
    $start = Get-Date
    
    for ($i = 0; $i -lt $Connections; $i++) {
        $jobs += Start-Job -ScriptBlock {
            param($url, $count)
            for ($j = 0; $j -lt $count; $j++) {
                $null = Invoke-WebRequest -Uri $url -UseBasicParsing
            }
        } -ArgumentList $Target, 100
    }
    
    $jobs | Wait-Job | Out-Null
    $elapsed = (Get-Date) - $start
    
    $totalRequests = $Connections * 100
    $rps = [math]::Round($totalRequests / $elapsed.TotalSeconds, 2)
    
    Write-Host "Total Requests: $totalRequests"
    Write-Host "Time: $($elapsed.TotalSeconds)s"
    Write-Host "Requests/sec: $rps"
    
    $jobs | Remove-Job
}

Write-Header "Benchmark Complete"
