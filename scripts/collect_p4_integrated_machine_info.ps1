$ErrorActionPreference = "Stop"

$OutFile = Join-Path $PSScriptRoot "p4-integrated-machine-info.txt"
$Computer = Get-CimInstance Win32_ComputerSystem
$OS = Get-CimInstance Win32_OperatingSystem
$CPU = Get-CimInstance Win32_Processor
$GPU = Get-CimInstance Win32_VideoController

$Lines = @()
$Lines += "P4 Integrated Atlas-to-Scroll GPU Validation"
$Lines += "CollectedAt=$(Get-Date -Format o)"
$Lines += "ComputerManufacturer=$($Computer.Manufacturer)"
$Lines += "ComputerModel=$($Computer.Model)"
$Lines += "MemoryGB=$([math]::Round($Computer.TotalPhysicalMemory / 1GB, 2))"
$Lines += "Windows=$($OS.Caption) $($OS.Version) build $($OS.BuildNumber)"
$Lines += "CPU=$($CPU.Name -join '; ')"
$Lines += "LogicalProcessors=$($Computer.NumberOfLogicalProcessors)"
$Lines += "GPU=$($GPU.Name -join '; ')"
$Lines += "GPUDriver=$($GPU.DriverVersion -join '; ')"
$Lines += "Display=$($GPU.CurrentHorizontalResolution)x$($GPU.CurrentVerticalResolution)"
$Lines += ""
$Lines += "WorkflowRun="
$Lines += "SourceCommit="
$Lines += "WindowResolution="
$Lines += "AtlasIndependentMapLayer=PASS/FAIL/NOT_TESTED"
$Lines += "DetailedGeographyReadable=PASS/FAIL/NOT_TESTED"
$Lines += "HistoryExplainsSettlement=PASS/FAIL/NOT_TESTED"
$Lines += "RoutesDerivedFromWorld=PASS/FAIL/NOT_TESTED"
$Lines += "CrossScaleIdentity=PASS/FAIL/NOT_TESTED"
$Lines += "BuildingLodStable=PASS/FAIL/NOT_TESTED"
$Lines += "VisibleCracksOrFlicker=NONE/DESCRIBE"
$Lines += "MotionDiscomfort=NONE/DESCRIBE"
$Lines += "FinalConclusion=PASS/CONDITIONAL/FAIL"
$Lines += "Notes="

$Lines | Set-Content -Encoding UTF8 $OutFile
Write-Host "Wrote $OutFile"
