# Checks that a Windows program is a window program (no console window of its own).
#   powershell -File tools\check-subsystem.ps1 [-Exe target\release\jmeter-ui.exe]
# Reads the "subsystem" field of the PE header: 2 is a window program, 3 is a console program.
param([string]$Exe = "target\release\jmeter-ui.exe")
$ErrorActionPreference = "Stop"
$bytes = [IO.File]::ReadAllBytes((Resolve-Path $Exe))
if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw "$Exe is not a Windows program file." }
$pe = [BitConverter]::ToInt32($bytes, 0x3C)
if ($pe -lt 0 -or $pe + 100 -gt $bytes.Length) { throw "$Exe has a damaged header." }
$subsystem = [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
switch ($subsystem) {
    2 { Write-Host "OK: $Exe is a window program (no console window)." }
    3 { throw "$Exe is a console program: starting it opens a terminal window." }
    default { throw "$Exe has an unexpected subsystem value $subsystem." }
}
