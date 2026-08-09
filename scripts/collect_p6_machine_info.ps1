$ErrorActionPreference = 'Stop'
$out = Join-Path $PSScriptRoot 'p6-machine-info.txt'
$os = Get-CimInstance Win32_OperatingSystem
$computer = Get-CimInstance Win32_ComputerSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$memoryGiB = [math]::Round($computer.TotalPhysicalMemory / 1GB, 2)
@"
P6 simulation scale manual validation machine information
Generated: $(Get-Date -Format o)
Computer: $($computer.Manufacturer) $($computer.Model)
Windows: $($os.Caption) $($os.Version)
CPU: $($cpu.Name)
Logical processors: $($computer.NumberOfLogicalProcessors)
Physical memory GiB: $memoryGiB
PowerShell: $($PSVersionTable.PSVersion)

Manual observations
-------------------
Peak working set (optional):
First run elapsed shown by program:
Repeat run elapsed shown by program:
Any long unresponsive period:
Any continuously growing memory:
Final verdict: PASS / CONDITIONAL PASS / FAIL
Notes:
"@ | Set-Content -Encoding UTF8 $out
Write-Host "Wrote $out"
