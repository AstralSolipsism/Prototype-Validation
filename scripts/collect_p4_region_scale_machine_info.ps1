$ErrorActionPreference = "Stop"

$output = Join-Path $PSScriptRoot "p4-region-scale-machine-info.txt"
$gpu = Get-CimInstance Win32_VideoController |
    Select-Object Name, DriverVersion, AdapterRAM, CurrentHorizontalResolution, CurrentVerticalResolution
$cpu = Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors
$system = Get-CimInstance Win32_ComputerSystem | Select-Object Manufacturer, Model, TotalPhysicalMemory
$os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber

@"
# P4 region-scale manual validation machine info

Generated: $(Get-Date -Format o)

## OS
$($os | Format-List | Out-String)

## System
$($system | Format-List | Out-String)

## CPU
$($cpu | Format-List | Out-String)

## GPU
$($gpu | Format-List | Out-String)

## Manual result
- Atlas/local scale separation:
- Selected-cell activation:
- Neighbor proxy continuity:
- Building grounding:
- Surface-conforming routes:
- Bridge:
- Cross-cell handoff:
- Camera/visual stability:
- Final verdict:
- Notes:
"@ | Set-Content -Encoding UTF8 $output

Write-Host "Wrote $output"
