# Helpers for driving the real JMeter UI window: start it, click, type, read its state, take screenshots.
# Dot-source this file from a test script: . "$PSScriptRoot\ui-lib.ps1"

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
if (-not ([System.Management.Automation.PSTypeName]'UiNative').Type) {
Add-Type @"
using System; using System.Runtime.InteropServices;
public class UiNative {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  public struct RECT { public int L, T, R, B; }
  public static void Click(int x, int y) { SetCursorPos(x, y); mouse_event(2,0,0,0,UIntPtr.Zero); mouse_event(4,0,0,0,UIntPtr.Zero); }
  public static void RightClick(int x, int y) { SetCursorPos(x, y); mouse_event(8,0,0,0,UIntPtr.Zero); mouse_event(16,0,0,0,UIntPtr.Zero); }
  public static void Down(int x, int y) { SetCursorPos(x, y); mouse_event(2,0,0,0,UIntPtr.Zero); }
  public static void Up() { mouse_event(4,0,0,0,UIntPtr.Zero); }
}
"@
}

$script:Results = @()
$script:Failures = 0

function Start-Ui {
    param([string]$Exe, [string[]]$Arguments, [int]$WaitSeconds = 4)
    $p = Start-Process $Exe -ArgumentList $Arguments -PassThru
    Start-Sleep -Seconds $WaitSeconds
    $p.Refresh()
    if ($p.HasExited) { throw "The app exited at startup (exit code $($p.ExitCode))" }
    [UiNative]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 400
    return $p
}

function Get-WindowRect($p) {
    $p.Refresh(); $r = New-Object UiNative+RECT
    [UiNative]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
    return $r
}

function Click-At($p, [int]$x, [int]$y) {
    $r = Get-WindowRect $p
    [UiNative]::Click($r.L + $x, $r.T + $y); Start-Sleep -Milliseconds 350
}

function RightClick-At($p, [int]$x, [int]$y) {
    $r = Get-WindowRect $p
    [UiNative]::RightClick($r.L + $x, $r.T + $y); Start-Sleep -Milliseconds 350
}

function Send-Keys([string]$keys) {
    [System.Windows.Forms.SendKeys]::SendWait($keys); Start-Sleep -Milliseconds 350
}

function Save-Shot($p, [string]$path) {
    $r = Get-WindowRect $p
    $w = $r.R - $r.L; $h = $r.B - $r.T
    $b = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($b)
    $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size)
    $b.Save($path); $g.Dispose(); $b.Dispose()
}

# Reads the state file the app writes. If the file is caught while it is being replaced it is read again.
function Read-State([string]$statePath) {
    for ($try = 0; $try -lt 20; $try++) {
        try { return Get-Content $statePath -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop }
        catch { Start-Sleep -Milliseconds 100 }
    }
    return Get-Content $statePath -Raw | ConvertFrom-Json
}

function Get-State([string]$statePath) {
    Start-Sleep -Milliseconds 250
    return Read-State $statePath
}

function Check([string]$name, [bool]$ok, $detail = "") {
    $script:Results += [pscustomobject]@{ Name = $name; Passed = $ok; Detail = "$detail" }
    if (-not $ok) { $script:Failures++ }
    $mark = if ($ok) { "PASS" } else { "FAIL" }
    Write-Host ("  [{0}] {1} {2}" -f $mark, $name, $(if ($ok) { "" } else { "- $detail" }))
}

function Stop-Ui($p) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }

# Type into the command palette and wait until the app has taken in every character (the debug build filters
# its list after each key, so it can fall behind the keyboard).
function Send-Query([string]$text, [string]$stateFile) {
    $want = $text -replace '\{([+^%~(){}\[\]])\}', '$1'
    [System.Windows.Forms.SendKeys]::SendWait($text)
    $deadline = (Get-Date).AddSeconds(8)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $s = Read-State $stateFile
        if ($s.palette -and $s.palette.query -eq $want) { break }
    }
    Start-Sleep -Milliseconds 250
}
