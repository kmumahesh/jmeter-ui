# Real-window smoke test: starts the built app, drives it with real mouse and keyboard input, and checks
# the state file the app writes (--state) plus settings saved on disk. Screenshots go to the output folder.
#
#   powershell -File tests\ui\smoke.ps1 [-Exe path\to\jmeter-ui.exe] [-Out folder]
#
# Coordinates are measured from the top-left of the window (the default 1280x800 layout).

param(
    [string]$Exe = "$PSScriptRoot\..\..\target\debug\jmeter-ui.exe",
    [string]$Out = "$PSScriptRoot\..\..\target\ui-smoke"
)

. "$PSScriptRoot\ui-lib.ps1"
# The app records the pages it is asked to open instead of starting a browser during the checks.
$env:JMETER_UI_NO_OPEN = "1"
$ErrorActionPreference = "Stop"
$clock = [System.Diagnostics.Stopwatch]::StartNew()
$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$plan = (Resolve-Path "$PSScriptRoot\..\..\crates\plan-core\tests\fixtures\build-adv-web-test-plan.jmx").Path

function New-Run([string]$name) {
    $dir = Join-Path $Out $name
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
    New-Item -ItemType Directory -Force $dir | Out-Null
    return [pscustomobject]@{ Dir = $dir; Settings = "$dir\settings.json"; State = "$dir\state.json" }
}

function Row-Y([int]$i) { return 86 + 25 * $i }   # y of tree row i

# ---------------------------------------------------------------------------------------------
Write-Host "1. Open a plan, select by mouse, navigate by keyboard"
$r = New-Run "tree"
$p = Start-Ui $Exe @($plan, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
$s = Get-State $r.State
Check "plan loads" ($s.elements -eq 23) "elements = $($s.elements)"
Check "tree shows 15 rows" ($s.visible_rows -eq 15) "rows = $($s.visible_rows)"
Click-At $p 150 (Row-Y 3)
$s = Get-State $r.State
Check "clicking a row selects it" ($s.selected -eq "HTTP Cookie Manager") "selected = $($s.selected)"
Check "and the editor shows it" ($s.editor.name -eq "HTTP Cookie Manager") "editor = $($s.editor.name)"
Check "without opening a tab" ($s.tabs.Count -eq 1) (($s.tabs | ForEach-Object { $_.title }) -join ", ")
Send-Keys "{DOWN}"
$s = Get-State $r.State
Check "Down moves the selection and the editor follows" ($s.selected -eq "HTTP Header Manager" -and $s.editor.name -eq "HTTP Header Manager") "selected = $($s.selected), editor = $($s.editor.name)"
Send-Keys "{END}"
$s = Get-State $r.State
Check "End selects the last row" ($s.selected -eq "View Results Tree") "selected = $($s.selected)"
Save-Shot $p "$($r.Dir)\tree.png"

Write-Host "2. Command palette"
Send-Keys "^k"
$s = Get-State $r.State
Check "Ctrl+K opens the palette" ($s.palette -ne $null) ""
Send-Query ">theme" $r.State
$s = Get-State $r.State
Check "typing filters commands" (($s.palette.results | Where-Object { $_ -notlike "*Theme*" }).Count -eq 0 -and $s.palette.results.Count -eq 3) ($s.palette.results -join "; ")
Save-Shot $p "$($r.Dir)\palette.png"
Send-Keys "{ESC}"
$s = Get-State $r.State
Check "Escape closes it" ($s.palette -eq $null) ""
Send-Keys "^p"
Send-Query "bugdetail" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "Go to element selects it" ($s.selected -eq "BugDetail Page") "selected = $($s.selected)"
Check "and the editor shows it, still in one tab" ($s.editor.name -eq "BugDetail Page" -and $s.tabs.Count -eq 1) "editor = $($s.editor.name), tabs = $($s.tabs.Count)"

Write-Host "3. Inspector"
$s = Get-State $r.State
Check "the inspector starts hidden" ($s.inspector -eq $false) ""
Send-Keys "^%b"
$s = Get-State $r.State
Check "Ctrl+Alt+B shows it" ($s.inspector -eq $true) ""
Save-Shot $p "$($r.Dir)\inspector.png"
Send-Keys "^%b"
Click-At $p 1000 19      # empty title bar area: should not toggle anything
$s = Get-State $r.State
Check "an empty title bar click changes nothing" ($s.tabs.Count -eq 1 -and $s.inspector -eq $false) ""

Write-Host "4. Menus"
Click-At $p 212 19        # View
Save-Shot $p "$($r.Dir)\menu-view.png"
Send-Keys "{ESC}"
Send-Keys "^j"
$s = Get-State $r.State
Check "Ctrl+J shows the results panel" ($s.bottom -eq $true) ""
Send-Keys "^b"
$s = Get-State $r.State
Check "Ctrl+B hides the test plan panel" ($s.sidebar -eq $false) ""
Send-Keys "^b"

Write-Host "5. Themes"
Send-Keys "^k"
Send-Query ">theme: light" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "light theme applies" ($s.dark -eq $false) ""
Save-Shot $p "$($r.Dir)\light.png"
Send-Keys "^k"
Send-Query ">theme: dark" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "dark theme applies" ($s.dark -eq $true) ""

Write-Host "6. Resizing a panel and remembering it"
$before = (Get-Content $r.Settings -Raw | ConvertFrom-Json).sidebar_width
$rect = Get-WindowRect $p
[UiNative]::Down($rect.L + 333, $rect.T + 400); Start-Sleep -Milliseconds 150
[UiNative]::SetCursorPos($rect.L + 380, $rect.T + 400); Start-Sleep -Milliseconds 150
[UiNative]::SetCursorPos($rect.L + 433, $rect.T + 400); Start-Sleep -Milliseconds 150
[UiNative]::Up(); Start-Sleep -Milliseconds 500
$after = (Get-Content $r.Settings -Raw | ConvertFrom-Json).sidebar_width
Check "dragging the handle widens the panel" ($after -gt $before + 50) "$before -> $after"

Write-Host "7. Window controls and remembered size"
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
Click-At $p 1218 19       # maximize
Start-Sleep -Milliseconds 700
$m = Get-WindowRect $p
Check "maximize fills the screen" (($m.R - $m.L) -ge $screen.Width) "width $($m.R - $m.L) vs $($screen.Width)"
Click-At $p ([int]($m.R - $m.L) - 31) 19   # close button (23px from the right edge plus the 8px window border)
Start-Sleep -Milliseconds 1200
$p.Refresh()
Check "close button ends the app" ($p.HasExited) ""
$saved = Get-Content $r.Settings -Raw | ConvertFrom-Json
Check "window state is saved" ($saved.window.maximized -eq $true) "maximized = $($saved.window.maximized)"

# ---------------------------------------------------------------------------------------------
Write-Host "8. Settings tab and drop-down"
$r = New-Run "settings"
$p = Start-Ui $Exe @("--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^,"
$s = Get-State $r.State
Check "Ctrl+, opens settings" (@($s.tabs | Where-Object { $_.title -eq "Settings" }).Count -eq 1) ""
Click-At $p 456 252       # the startup drop-down
Start-Sleep -Milliseconds 700
Save-Shot $p "$($r.Dir)\combo-open.png"
Send-Keys "{DOWN}"
Send-Keys "{ENTER}"
$saved = Get-Content $r.Settings -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json
Check "choosing from the drop-down with the keyboard saves it" ($saved.startup -eq "lastplan") "startup = $($saved.startup)"
Click-At $p 456 252
Start-Sleep -Milliseconds 700
Click-At $p 413 276       # first option: back to the welcome screen
$saved = Get-Content $r.Settings -Raw | ConvertFrom-Json
Check "choosing with the mouse saves it" ($saved.startup -eq "welcome") "startup = $($saved.startup)"
# Heap field: type an invalid value, apply, and expect an error notice and no change.
Click-At $p 460 500
Send-Keys "^a"
Send-Keys "10"
Click-At $p 417 558       # Apply and check
$s = Get-State $r.State
Check "an invalid heap size is rejected" ($s.notice -like "*not valid*") "notice = $($s.notice)"
Save-Shot $p "$($r.Dir)\settings-invalid.png"
Send-Keys "^a"
Send-Keys "2048"
Click-At $p 417 558
$saved = Get-Content $r.Settings -Raw | ConvertFrom-Json
Check "a valid heap size is saved" ($saved.heap_mb -eq 2048) "heap_mb = $($saved.heap_mb)"
Stop-Ui $p

Write-Host "9. Open dialog"
$r = New-Run "dialog"
$p = Start-Ui $Exe @("--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^o"
Start-Sleep -Milliseconds 2500
Send-Keys $plan
Send-Keys "{ENTER}"
Start-Sleep -Milliseconds 1800
$s = Get-State $r.State
Check "Ctrl+O opens a file dialog and loads the chosen plan" ($s.elements -eq 23) "elements = $($s.elements)"
$second = (Resolve-Path "$PSScriptRoot\..\..\crates\plan-core\tests\fixtures\Test.jmx").Path
Send-Keys "^o"
Start-Sleep -Milliseconds 2500
Send-Keys $second
Send-Keys "{ENTER}"
Start-Sleep -Milliseconds 1800
$s = Get-State $r.State
Check "a second plan gets its own tab" ($s.tabs.Count -eq 2 -and $s.active_tab -eq "Test.jmx") (($s.tabs | ForEach-Object { $_.title }) -join ", ")
Check "and the tree shows the second plan" ($s.elements -ne 23) "elements = $($s.elements)"
Save-Shot $p "$($r.Dir)\two-plans.png"
Send-Keys "^{TAB}"
$s = Get-State $r.State
Check "Ctrl+Tab switches back to the first plan" ($s.active_tab -eq "build-adv-web-test-plan.jmx" -and $s.elements -eq 23) "active = $($s.active_tab)"
Stop-Ui $p

# ---------------------------------------------------------------------------------------------
Write-Host "10. Editing: forms, add, delete, undo, drag, save, recovery"
$r = New-Run "editing"
Copy-Item $plan "$($r.Dir)\plan.jmx"
$work = "$($r.Dir)\plan.jmx"
$p = Start-Ui $Exe @($work, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^p"
Send-Query "bugdetail" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "go to element shows its form" ($s.editor.name -eq "BugDetail Page") "editor = $($s.editor.name)"
Click-At $p 880 487        # the Path box
Send-Keys "^a"
Send-Keys "/new/path"
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "typing in a box and pressing Enter changes the plan" ($s.dirty -eq $true -and $s.tabs[0].dirty -eq $true) "dirty = $($s.dirty)"
Click-At $p 623 559        # Redirect automatically
$s = Get-State $r.State
Check "clicking a checkbox is another change" ($s.can_undo -eq $true) ""
Save-Shot $p "$($r.Dir)\form-edited.png"
Click-At $p 180 386        # the tree row, so the tree has the focus
Send-Keys "^z"
$s = Get-State $r.State
Check "Ctrl+Z undoes the last change" ($s.can_redo -eq $true) ""
Send-Keys "^y"
Send-Keys "^s"
$s = Get-State $r.State
$saved = Get-Content $work -Raw
Check "Ctrl+S saves" ($s.dirty -eq $false -and $saved.Contains("/new/path") -and $saved.Contains('HTTPSampler.auto_redirects">true')) "dirty = $($s.dirty)"
Check "the file keeps its original formatting elsewhere" ($saved.Contains('jmeter="3.1-SNAPSHOT.20161018"')) ""

Send-Keys "^k"
Send-Query "{+}constant timer" $r.State
Start-Sleep -Milliseconds 900   # the debug build filters the list after every key
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "adding from the palette selects the new element" ($s.selected -eq "Constant Timer" -and $s.elements -eq 24) "selected = $($s.selected), elements = $($s.elements)"
Click-At $p 180 436        # focus the tree again
Send-Keys "^p"
Send-Query "constant timer" $r.State
Start-Sleep -Milliseconds 700
Send-Keys "{ENTER}"
Send-Keys "{DELETE}"
Start-Sleep -Milliseconds 600
$s = Get-State $r.State
Check "Delete removes the selected element" ($s.elements -eq 23) "elements = $($s.elements)"
Send-Keys "^z"
$s = Get-State $r.State
Check "and undo brings it back" ($s.elements -eq 24 -and $s.selected -eq "Constant Timer") ""

Send-Keys "^k"
Send-Query ">expand all" $r.State
Start-Sleep -Milliseconds 700
Send-Keys "{ENTER}"
$s = Get-State $r.State
$lastRow = 86 + 25 * ($s.visible_rows - 1)     # View Results Tree is the last row
$w = Get-WindowRect $p
[UiNative]::Down($w.L + 180, $w.T + $lastRow); Start-Sleep -Milliseconds 200
foreach ($y in ($lastRow - 20), ($lastRow - 80), ($lastRow - 200), 330, 280, 245, 239, 236) { [UiNative]::SetCursorPos($w.L + 180, $w.T + $y); Start-Sleep -Milliseconds 120 }
[UiNative]::Up(); Start-Sleep -Milliseconds 500
$s = Get-State $r.State
Check "dragging a row onto another moves the element" ($s.selected -eq "View Results Tree" -and $s.can_undo -eq $true) "selected = $($s.selected)"
Save-Shot $p "$($r.Dir)\dropped.png"

RightClick-At $p 150 236
Save-Shot $p "$($r.Dir)\context-menu.png"
Send-Keys "{ESC}"

# Unsaved work survives a crash: wait for the recovery copy, kill the app, open the plan again.
Send-Keys "^k"
Send-Query "{+}debug sampler" $r.State
Start-Sleep -Milliseconds 900
Send-Keys "{ENTER}"
Start-Sleep -Seconds 4
Check "a recovery copy is written next to the plan after a pause" (Test-Path "$work.bak") ""
Stop-Process -Id $p.Id -Force
Start-Sleep -Seconds 1
$p = Start-Ui $Exe @($work, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
$s = Get-State $r.State
Check "opening the plan again offers the unsaved work" ($s.dialog.kind -eq "recover") "dialog = $($s.dialog | ConvertTo-Json -Compress)"
Save-Shot $p "$($r.Dir)\recover-dialog.png"
Click-At $p 650 437        # Recover Changes
$s = Get-State $r.State
Check "recovering brings the changes back, unsaved" ($s.dirty -eq $true -and $s.dialog -eq $null) "dirty = $($s.dirty)"
Click-At $p 1264 19        # the window's own close button
Start-Sleep -Milliseconds 800
$s = Get-State $r.State
if ($s.dialog -eq $null) { Click-At $p 1264 19; Start-Sleep -Milliseconds 800; $s = Get-State $r.State }   # the first click can only focus the window
Check "closing the window with unsaved changes asks first" ($s.dialog.kind -eq "unsaved" -and $s.dialog.then -eq "Quit") "dialog = $($s.dialog | ConvertTo-Json -Compress)"
Click-At $p 819 437        # Cancel
$s = Get-State $r.State
Check "Cancel keeps the app open with the work" ($s.dialog -eq $null -and $s.dirty -eq $true) ""
Send-Keys "^k"
Send-Query ">quit" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "quitting with unsaved changes asks first" ($s.dialog.kind -eq "unsaved") ""
Click-At $p 748 437        # Don't Save
Start-Sleep -Milliseconds 800
$p.Refresh()
Check "Don't Save quits" ($p.HasExited) ""
Check "and removes the recovery copy" (-not (Test-Path "$work.bak")) ""

# ---------------------------------------------------------------------------------------------
Write-Host "11. Running: F5, live numbers, history, Validate, Send once"
$jmHome = if ($env:JMETER_HOME) { $env:JMETER_HOME } else { Join-Path $env:USERPROFILE "Downloads\apache-jmeter-5.6.3" }
if (-not (Test-Path "$jmHome\bin\ApacheJMeter.jar") -or -not (Get-Command java -ErrorAction SilentlyContinue)) {
    Write-Host "  (skipped: Java or JMeter is not installed)"
} else {
    $r = New-Run "running"
    # A small web server on a free local port; it answers 500 for /fail and 200 for the rest.
    $portFile = "$($r.Dir)\port.txt"
    $server = Start-Job -ArgumentList $portFile -ScriptBlock {
        param($portFile)
        $l = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 0)
        $l.Start()
        Set-Content $portFile ($l.LocalEndpoint.Port)
        while ($true) {
            $c = $l.AcceptTcpClient()
            try {
                $st = $c.GetStream(); $buf = New-Object byte[] 8192; $n = $st.Read($buf, 0, 8192)
                $req = [System.Text.Encoding]::ASCII.GetString($buf, 0, $n)
                $status = if ($req -match "^GET /fail") { "500 Internal Server Error" } else { "200 OK" }
                $resp = [System.Text.Encoding]::ASCII.GetBytes("HTTP/1.1 $status`r`nContent-Length: 2`r`nConnection: close`r`n`r`nok")
                $st.Write($resp, 0, $resp.Length)
            } catch {} finally { $c.Close() }
        }
    }
    $p = $null
    try {
    $deadline = (Get-Date).AddSeconds(10)
    while (-not (Test-Path $portFile) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
    $port = [int](Get-Content $portFile -Raw)
    function Sampler([string]$name, [string]$path) {
        return @"
        <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="$name" enabled="true">
          <elementProp name="HTTPsampler.Arguments" elementType="Arguments" guiclass="HTTPArgumentsPanel" testclass="Arguments" testname="User Defined Variables" enabled="true"><collectionProp name="Arguments.arguments"/></elementProp>
          <stringProp name="HTTPSampler.domain">127.0.0.1</stringProp>
          <stringProp name="HTTPSampler.port">$port</stringProp>
          <stringProp name="HTTPSampler.protocol">http</stringProp>
          <stringProp name="HTTPSampler.path">$path</stringProp>
          <stringProp name="HTTPSampler.method">GET</stringProp>
        </HTTPSamplerProxy>
        <hashTree/>
"@
    }
    $planText = @"
<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3"><hashTree>
<TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Shop" enabled="true"><boolProp name="TestPlan.functional_mode">false</boolProp><elementProp name="TestPlan.user_defined_variables" elementType="Arguments" guiclass="ArgumentsPanel" testclass="Arguments" testname="User Defined Variables" enabled="true"><collectionProp name="Arguments.arguments"/></elementProp></TestPlan>
<hashTree>
  <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users" enabled="true">
    <stringProp name="ThreadGroup.on_sample_error">continue</stringProp>
    <elementProp name="ThreadGroup.main_controller" elementType="LoopController" guiclass="LoopControlPanel" testclass="LoopController" testname="Loop Controller" enabled="true"><boolProp name="LoopController.continue_forever">false</boolProp><stringProp name="LoopController.loops">3</stringProp></elementProp>
    <stringProp name="ThreadGroup.num_threads">2</stringProp><stringProp name="ThreadGroup.ramp_time">0</stringProp>
  </ThreadGroup>
  <hashTree>
$(Sampler "Home" "/")$(Sampler "Broken" "/fail")  </hashTree>
</hashTree></hashTree></jmeterTestPlan>
"@
    $shop = "$($r.Dir)\shop.jmx"
    [System.IO.File]::WriteAllText($shop, $planText)
    $before = Get-Content $shop -Raw
    Set-Content $r.Settings (@{ jmeter_home = $jmHome } | ConvertTo-Json) -Encoding UTF8
    $p = Start-Ui $Exe @($shop, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark")
    $deadline = (Get-Date).AddSeconds(30)
    do { Start-Sleep -Milliseconds 500; $s = Get-State $r.State } while (-not $s.runtime_ready -and (Get-Date) -lt $deadline)
    Check "Java and JMeter are found in the background" ($s.runtime_ready -eq $true) "java = $($s.java), jmeter = $($s.jmeter)"

    function Wait-RunEnd([string]$state, [int]$seconds = 90) {
        $until = (Get-Date).AddSeconds($seconds)
        do { Start-Sleep -Milliseconds 500; $x = Read-State $state } while ($x.run -and @("Starting", "Running", "Stopping") -contains $x.run.phase -and (Get-Date) -lt $until)
        return $x
    }

    Send-Keys "{F5}"
    Start-Sleep -Milliseconds 800
    $s = Get-State $r.State
    Check "F5 starts the run and opens the lower panel on the Run tab" ($s.run -and $s.bottom -eq $true -and $s.bottom_tab -eq "Run") "run = $($s.run | ConvertTo-Json -Compress)"
    Save-Shot $p "$($r.Dir)\run-started.png"
    $s = Wait-RunEnd $r.State
    Save-Shot $p "$($r.Dir)\run-finished.png"
    Check "the run finishes" ($s.run.phase -eq "Finished") "phase = $($s.run.phase); error = $($s.run.error)"
    Check "all 12 samples were counted, 6 of them errors" ($s.run.samples -eq 12 -and $s.run.errors -eq 6) "samples = $($s.run.samples), errors = $($s.run.errors)"
    Check "the plan file was not changed by the run" ((Get-Content $shop -Raw) -eq $before) ""
    $dir = $s.run.dir
    Check "the run has its own folder inside runs next to the plan" ((Split-Path $dir -Parent) -eq "$($r.Dir)\runs") "dir = $dir"
    Check "with results, log, the plan as run and run.json" ((Test-Path "$dir\results.jtl") -and (Test-Path "$dir\jmeter.log") -and (Test-Path "$dir\plan-as-run.jmx") -and (Test-Path "$dir\run.json")) ""
    Check "the history lists the run" ($s.history.Count -eq 1 -and $s.history[0].outcome -eq "Finished") ""

    Check "the six failed requests were kept with what was sent and received" ($s.run.details -eq 6 -and $s.run.details_failed -eq 6) "details = $($s.run.details)"
    Send-Keys "^+r"
    Start-Sleep -Milliseconds 400
    $s = Get-State $r.State
    Check "Ctrl+Shift+R opens the request viewer on the first failure" ($s.bottom_tab -eq "Requests" -and $s.requests.selected -eq 0) "tab = $($s.bottom_tab)"
    Check "it shows the request that was sent and the answer that came back" ($s.requests.request -like "*GET*/fail*" -and $s.requests.response -like "*500*") "request = $($s.requests.request); response = $($s.requests.response)"
    Send-Keys "{DOWN}"
    $s = Get-State $r.State
    Check "the Down key moves to the next request" ($s.requests.selected -eq 1) "selected = $($s.requests.selected)"
    Save-Shot $p "$($r.Dir)\request-viewer.png"
    Send-Keys "^+t"
    $s = Get-State $r.State
    Check "Ctrl+Shift+T opens the expression dialog with the response of the chosen request" ($s.dialog -and $s.dialog.kind -eq "expression" -and $s.dialog.sample -eq "ok") "dialog = $($s.dialog | ConvertTo-Json -Compress)"
    Send-Keys '$.a'
    Send-Keys "{ENTER}"
    $until = (Get-Date).AddSeconds(60)
    do { Start-Sleep -Milliseconds 500; $s = Get-State $r.State } while ($s.dialog -and $s.dialog.busy -ne $false -and (Get-Date) -lt $until)
    Check "the real JMeter libraries answer, and say that the text is not JSON" ($s.dialog.error -eq $true -and $s.dialog.status -like "*not a JSON object*") "status = $($s.dialog.status)"
    Save-Shot $p "$($r.Dir)\expression-dialog.png"
    Send-Keys "{ESC}"
    $s = Get-State $r.State
    Check "Escape closes the dialog" ($null -eq $s.dialog) ""

    Send-Keys "^{F5}"
    $s = Wait-RunEnd $r.State
    Check "Ctrl+F5 validates: one user once, so each request once" ($s.run.kind -eq "Validate" -and $s.run.samples -eq 2) "kind = $($s.run.kind), samples = $($s.run.samples)"

    Send-Keys "^p"
    Send-Query "home" $r.State
    Send-Keys "{ENTER}"
    Send-Keys "{F6}"
    $s = Wait-RunEnd $r.State
    Check "F6 sends the selected request once" ($s.run.kind -eq "Send once" -and $s.run.samples -eq 1) "kind = $($s.run.kind), samples = $($s.run.samples)"
    Check "all three runs are in the history" ($s.history.Count -eq 3) "count = $($s.history.Count)"
    Send-Keys "^+h"
    $until = (Get-Date).AddSeconds(120)
    do { Start-Sleep -Milliseconds 500; $s = Get-State $r.State } while ((-not $s.report.exists -or $s.report.busy) -and (Get-Date) -lt $until)
    Check "Ctrl+Shift+H makes the HTML report with JMeter's generator and opens its page" ($s.report.exists -eq $true -and @($s.opened).Count -eq 1 -and (Test-Path "$($s.run.dir)\report\index.html")) "report = $($s.report | ConvertTo-Json -Compress); opened = $($s.opened -join ', ')"
    Save-Shot $p "$($r.Dir)\report-made.png"
    Save-Shot $p "$($r.Dir)\run-history.png"

    } finally {
        # Whatever happened above, leave nothing running.
        if ($p) { Stop-Ui $p }
        Stop-Job $server -ErrorAction SilentlyContinue; Remove-Job $server -Force -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------------------------------------
Write-Host "12. New plan"
$r = New-Run "newplan"
$p = Start-Ui $Exe @("--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^n"
$s = Get-State $r.State
Check "Ctrl+N asks which kind of plan to start" ($s.dialog -and $s.dialog.kind -eq "new_plan") "dialog = $($s.dialog | ConvertTo-Json -Compress)"
Save-Shot $p "$($r.Dir)\new-plan.png"
Stop-Ui $p

# ---------------------------------------------------------------------------------------------
Write-Host "13. Text area for a script"
$r = New-Run "script"
$scriptPlan = "$($r.Dir)\scripts.jmx"
[System.IO.File]::WriteAllText($scriptPlan, @'
<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3"><hashTree>
<TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Scripts" enabled="true"><boolProp name="TestPlan.functional_mode">false</boolProp><elementProp name="TestPlan.user_defined_variables" elementType="Arguments" guiclass="ArgumentsPanel" testclass="Arguments" testname="User Defined Variables" enabled="true"><collectionProp name="Arguments.arguments"/></elementProp></TestPlan>
<hashTree><ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users" enabled="true"><stringProp name="ThreadGroup.on_sample_error">continue</stringProp><elementProp name="ThreadGroup.main_controller" elementType="LoopController" guiclass="LoopControlPanel" testclass="LoopController" testname="Loop Controller" enabled="true"><boolProp name="LoopController.continue_forever">false</boolProp><stringProp name="LoopController.loops">1</stringProp></elementProp><stringProp name="ThreadGroup.num_threads">1</stringProp><stringProp name="ThreadGroup.ramp_time">0</stringProp></ThreadGroup>
<hashTree><JSR223Sampler guiclass="TestBeanGUI" testclass="JSR223Sampler" testname="Calc" enabled="true"><stringProp name="scriptLanguage">groovy</stringProp><stringProp name="script">// start
def a = 1</stringProp></JSR223Sampler><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>
'@)
$p = Start-Ui $Exe @($scriptPlan, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^p"
Send-Query "calc" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
Check "the script element is selected" ($s.selected -eq "Calc") "selected = $($s.selected)"
Click-At $p 800 600          # inside the script text area
Send-Keys "^{END}"
Send-Keys "{ENTER}b = 2"
Start-Sleep -Milliseconds 800
Save-Shot $p "$($r.Dir)\script-area.png"
$s = Get-State $r.State
Check "typing in the area marks the plan as changed" ($s.dirty -eq $true) ""
Send-Keys "^s"
Start-Sleep -Milliseconds 600
$saved = [System.IO.File]::ReadAllText($scriptPlan)
Check "the saved script keeps its lines" ($saved -match "// start\r?\ndef a = 1\r?\nb = 2") ""
$s = Get-State $r.State
Check "and nothing is left to save" ($s.dirty -eq $false) ""
Stop-Ui $p

# ---------------------------------------------------------------------------------------------
Write-Host "14. Copy and paste of elements"
$r = New-Run "copypaste"
$p = Start-Ui $Exe @($plan, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^p"
Send-Query "bugdetail" $r.State
Send-Keys "{ENTER}"
$s = Get-State $r.State
$before = $s.elements
Check "a request is selected in the tree" ($s.selected -eq "BugDetail Page") "selected = $($s.selected)"
Send-Keys "^c"
Send-Keys "^v"
$s = Get-State $r.State
Check "Ctrl+C then Ctrl+V adds a copy with what is under it" ($s.elements -gt $before -and $s.dirty -eq $true) "elements $before -> $($s.elements)"
Save-Shot $p "$($r.Dir)\pasted.png"
Send-Keys "^z"
$s = Get-State $r.State
Check "one Ctrl+Z takes the whole paste away" ($s.elements -eq $before -and $s.dirty -eq $false) "elements = $($s.elements), dirty = $($s.dirty)"
Send-Keys "^x"
$s = Get-State $r.State
Check "Ctrl+X removes the element" ($s.elements -lt $before) "elements = $($s.elements)"
Send-Keys "^z"
Stop-Ui $p

Write-Host "15. Import from cURL"
$r = New-Run "curl"
$p = Start-Ui $Exe @($plan, "--settings", $r.Settings, "--state", $r.State, "--theme", "dark", "--no-detect")
Send-Keys "^+i"
$s = Get-State $r.State
Check "Ctrl+Shift+I opens the cURL dialog" ($s.dialog -and $s.dialog.kind -eq "curl") "dialog = $($s.dialog | ConvertTo-Json -Compress)"
Send-Keys "curl https://x.org/a"
# The window shows the typed text a moment later on a slow machine, so look for a few seconds.
$until = (Get-Date).AddSeconds(5)
do { Start-Sleep -Milliseconds 250; $s = Get-State $r.State } while ($s.dialog.text -ne "curl https://x.org/a" -and (Get-Date) -lt $until)
Check "typing goes into the dialog's text area" ($s.dialog.text -eq "curl https://x.org/a") "text = $($s.dialog.text)"
Save-Shot $p "$($r.Dir)\curl-dialog.png"
Stop-Ui $p

# ---------------------------------------------------------------------------------------------
$passed = ($script:Results | Where-Object { $_.Passed }).Count
Write-Host ""
Write-Host "$passed of $($script:Results.Count) checks passed. Screenshots: $Out"
Write-Host ("Total time: {0:N1} s" -f $clock.Elapsed.TotalSeconds)
if ($script:Failures -gt 0) { exit 1 }
