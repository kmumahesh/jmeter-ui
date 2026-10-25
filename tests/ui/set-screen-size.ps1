# Raises the screen to the largest mode the machine offers (shared CI runners start at 1024x768, which is too
# small for the 1280x800 window the real-window checks are written for). Prints every mode it finds.
param([int]$MinWidth = 1400, [int]$MinHeight = 900)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Display {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
  public struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
    public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
    public int dmFields;
    public int dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
    public short dmLogPixels;
    public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
    public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
  }
  [DllImport("user32.dll", CharSet = CharSet.Ansi, SetLastError = true)] public static extern bool EnumDisplaySettings(string device, int mode, ref DEVMODE dm);
  [DllImport("user32.dll", CharSet = CharSet.Ansi)] public static extern int ChangeDisplaySettings(ref DEVMODE dm, int flags);
}
"@

$ErrorActionPreference = "Stop"

function New-Mode {
    $dm = New-Object Display+DEVMODE
    $dm.dmSize = [int16][System.Runtime.InteropServices.Marshal]::SizeOf([type]"Display+DEVMODE")
    return $dm
}

$cur = New-Mode
$ok = [Display]::EnumDisplaySettings($null, -1, [ref]$cur)
"Struct size $($cur.dmSize); current mode read: $ok; $($cur.dmPelsWidth)x$($cur.dmPelsHeight) at $($cur.dmBitsPerPel) bits (last error $([System.Runtime.InteropServices.Marshal]::GetLastWin32Error()))"

# Enumeration may be refused on a virtual display, so also try the usual sizes directly (flag 2 = test only).
$candidates = @(@(1920, 1080), @(1680, 1050), @(1600, 900), @(1440, 900), @(1400, 1050), @(1366, 768), @(1280, 1024), @(1280, 960), @(1280, 800))
$chosen = $null
foreach ($c in $candidates) {
    if ($c[0] -lt $MinWidth -or $c[1] -lt $MinHeight) { continue }
    $dm = New-Mode
    [void][Display]::EnumDisplaySettings($null, -1, [ref]$dm)
    $dm.dmPelsWidth = $c[0]; $dm.dmPelsHeight = $c[1]
    $dm.dmFields = 0x80000 -bor 0x100000
    $t = [Display]::ChangeDisplaySettings([ref]$dm, 2)
    "  test $($c[0])x$($c[1]): $t (0 is accepted)"
    if ($t -eq 0 -and -not $chosen) { $chosen = $c }
}
if (-not $chosen) { $chosen = @(1280, 800) }
$dm = New-Mode
[void][Display]::EnumDisplaySettings($null, -1, [ref]$dm)
$dm.dmPelsWidth = $chosen[0]; $dm.dmPelsHeight = $chosen[1]
$dm.dmFields = 0x80000 -bor 0x100000
$r = [Display]::ChangeDisplaySettings([ref]$dm, 0)
Start-Sleep -Seconds 2
"Wanted $($chosen[0])x$($chosen[1]); change result $r (0 is success)"
Add-Type -AssemblyName System.Windows.Forms
$s = [System.Windows.Forms.Screen]::PrimaryScreen
"Screen is now $($s.Bounds.Width)x$($s.Bounds.Height), working area $($s.WorkingArea.Width)x$($s.WorkingArea.Height)"
