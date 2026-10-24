# Runs every kind of check and records how long each took in docs\TEST-TIMINGS.md.
#
#   powershell -ExecutionPolicy Bypass -File tools\run-tests.ps1 [-Smoke] [-NoRecord]
#
# -Smoke     also runs the real-window script tests\ui\smoke.ps1 (it takes over the mouse and keyboard)
# -NoRecord  prints the table but does not touch docs\TEST-TIMINGS.md

param(
    [switch]$Smoke,
    [switch]$NoRecord
)

# cargo prints progress on stderr, which Windows PowerShell 5.1 would treat as an error.
$ErrorActionPreference = "Continue"
$root = (Resolve-Path "$PSScriptRoot\..").Path
Set-Location $root
$rows = New-Object System.Collections.ArrayList
$failed = $false
$all = [System.Diagnostics.Stopwatch]::StartNew()

function Add-Row([string]$group, [string]$what, [double]$secs, [string]$result) {
    [void]$rows.Add([pscustomobject]@{ Group = $group; What = $what; Seconds = [math]::Round($secs, 1); Result = $result })
}

function Time-Step([string]$group, [string]$what, [scriptblock]$body) {
    Write-Host "== $group ..."
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = & $body 2>&1 | ForEach-Object { "$_" } | Out-String
    $ok = ($LASTEXITCODE -eq 0)
    $sw.Stop()
    if (-not $ok) { $script:failed = $true; Write-Host $out }
    Add-Row $group $what $sw.Elapsed.TotalSeconds $(if ($ok) { "passed" } else { "FAILED" })
    return $out
}

# --- unit and whole-window tests: one row per test program, from cargo's own "finished in" lines ---
Write-Host "== Tests ..."
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$text = cmd /c "cargo test --workspace --exclude gpui-spike --exclude zed-ui-spike 2>&1" | Out-String
$code = $LASTEXITCODE
$sw.Stop()
if ($code -ne 0) { $failed = $true; Write-Host $text }
$current = ""
foreach ($line in ($text -split "`n")) {
    if ($line -match "Running (unittests )?\S+ \(([^)]+)\)") {
        $exe = ($Matches[2] -replace "\\", "/" -split "/")[-1] -replace "\.exe$", "" -replace "-[0-9a-f]{16}$", ""
        $current = if ($Matches[1]) { "$exe (unit tests)" } else { $exe }
    }
    elseif ($line -match "test result: (\w+)\. (\d+) passed; (\d+) failed.*finished in ([\d.]+)s" -and $Matches[2] -ne "0") {
        Add-Row "Tests" "$current, $($Matches[2]) tests" ([double]$Matches[4]) $(if ($Matches[3] -eq "0") { "passed" } else { "FAILED" })
    }
}
Add-Row "Tests" "all test programs, with building" $sw.Elapsed.TotalSeconds $(if ($code -eq 0) { "passed" } else { "FAILED" })

[void](Time-Step "Lints" "cargo clippy, warnings as errors" { cargo clippy --workspace --exclude gpui-spike --exclude zed-ui-spike --all-targets -- -D warnings })
if (Get-Command cargo-audit -ErrorAction SilentlyContinue) { [void](Time-Step "Supply chain" "cargo audit" { cargo audit }) }
if (Get-Command cargo-deny -ErrorAction SilentlyContinue) { [void](Time-Step "Supply chain" "cargo deny check" { cargo deny check }) }

if ($Smoke) {
    Write-Host "== Real window (do not touch the mouse or keyboard) ..."
    $exe = Join-Path $root "target\debug\jmeter-ui.exe"
    cmd /c "cargo build -p jmeter-ui 2>&1" | Out-Null
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = & powershell -ExecutionPolicy Bypass -File "$root\tests\ui\smoke.ps1" 2>&1 | Out-String
    $ok = ($LASTEXITCODE -eq 0)
    $sw.Stop()
    if (-not $ok) { $failed = $true; Write-Host $out }
    $summary = ($out -split "`n" | Where-Object { $_ -match "checks passed" } | Select-Object -First 1)
    Add-Row "Real window" "smoke.ps1 ($($summary.Trim()))" $sw.Elapsed.TotalSeconds $(if ($ok) { "passed" } else { "FAILED" })
}

$all.Stop()
Add-Row "Total" "everything above" $all.Elapsed.TotalSeconds $(if ($failed) { "FAILED" } else { "passed" })

Write-Host ""
$rows | Format-Table -AutoSize | Out-String | Write-Host

if (-not $NoRecord) {
    $file = Join-Path $root "docs\TEST-TIMINGS.md"
    $stamp = Get-Date -Format "yyyy-MM-dd HH:mm"
    $git = (& git rev-parse --short HEAD 2>$null)
    $section = New-Object System.Text.StringBuilder
    [void]$section.AppendLine("## $stamp" + $(if ($git) { " (after commit $git)" } else { "" }) + $(if ($Smoke) { "" } else { ", without the real-window script" }))
    [void]$section.AppendLine("")
    [void]$section.AppendLine("| Group | What | Seconds | Result |")
    [void]$section.AppendLine("|---|---|---:|---|")
    foreach ($r in $rows) { [void]$section.AppendLine("| $($r.Group) | $($r.What) | $($r.Seconds) | $($r.Result) |") }
    [void]$section.AppendLine("")
    $header = "# Test timings`n`nHow long each kind of check took, one section per run of ``tools\run-tests.ps1``, newest first. Times are wall-clock seconds on the developer's machine in a debug build; they show trends, not guarantees. The runs that start real JMeter processes are the slow ones.`n`n"
    $old = ""
    if (Test-Path $file) {
        $old = Get-Content $file -Raw
        $idx = $old.IndexOf("## ")
        $old = if ($idx -ge 0) { $old.Substring($idx) } else { "" }
    }
    # Keep the 15 newest runs.
    $parts = [regex]::Split($old, "(?m)^(?=## )") | Where-Object { $_.Trim() }
    $keep = @($section.ToString()) + @($parts | Select-Object -First 14)
    [System.IO.File]::WriteAllText($file, ($header + ($keep -join "")), (New-Object System.Text.UTF8Encoding $false))
    Write-Host "Timings recorded in docs\TEST-TIMINGS.md"
}

if ($failed) { exit 1 }
