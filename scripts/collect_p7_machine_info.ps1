$ErrorActionPreference = 'Stop'
$out = Join-Path $PSScriptRoot 'p7-machine-info.txt'
$computer = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$memoryGiB = [math]::Round($computer.TotalPhysicalMemory / 1GB, 2)
@"
P7 multiplayer interest and region authority manual validation machine information
Generated: $((Get-Date).ToString('o'))
Computer: $($computer.Manufacturer) $($computer.Model)
Windows: $($os.Caption) $($os.Version)
CPU: $($cpu.Name)
Logical processors: $($computer.NumberOfLogicalProcessors)
Physical memory GiB: $memoryGiB
PowerShell: $($PSVersionTable.PSVersion)

Manual observations
-------------------
Any firewall prompt for loopback execution:
Any long unresponsive period:
Any continuously growing memory:
All 15 checks shown as PASS:
Final verdict: PASS / CONDITIONAL PASS / FAIL
Notes:
"@ | Set-Content -Encoding UTF8 $out
Write-Host "Wrote $out"
