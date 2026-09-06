$ErrorActionPreference = "Stop"

$output = Join-Path $PSScriptRoot "p1-machine-info.txt"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
$os = Get-CimInstance Win32_OperatingSystem
$computer = Get-CimInstance Win32_ComputerSystem
$cpus = Get-CimInstance Win32_Processor
$gpus = Get-CimInstance Win32_VideoController

$lines = @()
$lines += "P1 visual validation environment"
$lines += "CapturedAt: $timestamp"
$lines += "Computer: $($computer.Manufacturer) $($computer.Model)"
$lines += "OS: $($os.Caption) $($os.Version) build $($os.BuildNumber)"
$lines += "MemoryGiB: $([math]::Round($computer.TotalPhysicalMemory / 1GB, 2))"
$lines += "CPU: $((($cpus | ForEach-Object { $_.Name.Trim() }) -join '; '))"
$lines += "GPU: $((($gpus | ForEach-Object { $_.Name.Trim() }) -join '; '))"
$lines += "GPUDriver: $((($gpus | ForEach-Object { $_.DriverVersion }) -join '; '))"
$lines += "Resolution: $((($gpus | ForEach-Object { if ($_.CurrentHorizontalResolution -and $_.CurrentVerticalResolution) { \"$($_.CurrentHorizontalResolution)x$($_.CurrentVerticalResolution)\" } }) -join '; '))"
$lines += ""
$lines += "Manual result: NOT_RECORDED"
$lines += "Forward route recording:"
$lines += "Reverse route recording:"
$lines += "Observed camera clipping:"
$lines += "Observed complete player occlusion:"
$lines += "Observed direction ambiguity:"
$lines += "Observed discomfort or motion sickness:"
$lines += "Notes:"

$lines | Set-Content -Encoding UTF8 $output
Write-Host "Wrote $output"
