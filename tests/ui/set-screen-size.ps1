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
  [DllImport("user32.dll", CharSet = CharSet.Ansi)] public static extern bool EnumDisplaySettings(string device, int mode, ref DEVMODE dm);
  [DllImport("user32.dll", CharSet = CharSet.Ansi)] public static extern int ChangeDisplaySettings(ref DEVMODE dm, int flags);
}
"@

$modes = @()
$i = 0
while ($true) {
    $dm = New-Object Display+DEVMODE
    $dm.dmSize = [int][System.Runtime.InteropServices.Marshal]::SizeOf($dm)
    if (-not [Display]::EnumDisplaySettings($null, $i, [ref]$dm)) { break }
    if ($dm.dmBitsPerPel -ge 24) { $modes += [pscustomobject]@{ W = $dm.dmPelsWidth; H = $dm.dmPelsHeight; Mode = $dm } }
    $i++
}
"Modes offered: " + (($modes | Sort-Object W, H -Unique | ForEach-Object { "$($_.W)x$($_.H)" }) -join ", ")
$best = $modes | Where-Object { $_.W -ge $MinWidth -and $_.H -ge $MinHeight } | Sort-Object { $_.W * $_.H } | Select-Object -First 1
if (-not $best) { $best = $modes | Sort-Object { $_.W * $_.H } -Descending | Select-Object -First 1 }
$dm = $best.Mode
$dm.dmFields = 0x80000 -bor 0x100000   # width and height
$r = [Display]::ChangeDisplaySettings([ref]$dm, 0)
Start-Sleep -Seconds 2
Add-Type -AssemblyName System.Windows.Forms
$s = [System.Windows.Forms.Screen]::PrimaryScreen
"Wanted $($best.W)x$($best.H); change result $r; screen is now $($s.Bounds.Width)x$($s.Bounds.Height), working area $($s.WorkingArea.Width)x$($s.WorkingArea.Height)"
