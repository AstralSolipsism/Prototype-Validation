$ErrorActionPreference = "Stop"

$output = Join-Path $PSScriptRoot "p3-machine-info.txt"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
$os = Get-CimInstance Win32_OperatingSystem
$computer = Get-CimInstance Win32_ComputerSystem
$cpus = Get-CimInstance Win32_Processor
$gpus = Get-CimInstance Win32_VideoController

$cpuNames = $cpus | ForEach-Object { $_.Name.Trim() }
$gpuNames = $gpus | ForEach-Object { $_.Name.Trim() }
$gpuDrivers = $gpus | ForEach-Object { $_.DriverVersion }
$resolutions = $gpus | ForEach-Object {
    if ($_.CurrentHorizontalResolution -and $_.CurrentVerticalResolution) {
        "{0}x{1}" -f $_.CurrentHorizontalResolution, $_.CurrentVerticalResolution
    }
}

$lines = @(
    "P3 semantic building compiler visual validation environment",
    "CapturedAt: $timestamp",
    "Computer: $($computer.Manufacturer) $($computer.Model)",
    "OS: $($os.Caption) $($os.Version) build $($os.BuildNumber)",
    "MemoryGiB: $([math]::Round($computer.TotalPhysicalMemory / 1GB, 2))",
    "CPU: $(($cpuNames) -join '; ')",
    "GPU: $(($gpuNames) -join '; ')",
    "GPUDriver: $(($gpuDrivers) -join '; ')",
    "Resolution: $(($resolutions) -join '; ')",
    "",
    "Workflow run:",
    "Commit SHA:",
    "Manual result: NOT_RECORDED",
    "Complete all-side geometry:",
    "Interior and exterior continuity:",
    "Full/shell/massing anchor consistency:",
    "Cutaway behavior:",
    "Localized window rebuild:",
    "Static/mobile instance consistency:",
    "Observed clipping or flicker:",
    "Observed performance issue:",
    "Notes:"
)

$lines | Set-Content -Encoding UTF8 $output
Write-Host "Wrote $output"
