$ErrorActionPreference = "Stop"

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("P5 authoritative server manual validation environment")
$lines.Add("GeneratedAtUtc: $([DateTime]::UtcNow.ToString('o'))")
$lines.Add("ComputerName: $env:COMPUTERNAME")
$lines.Add("UserDomain: $env:USERDOMAIN")

try {
    $os = Get-CimInstance Win32_OperatingSystem
    $lines.Add("Windows: $($os.Caption) $($os.Version) build $($os.BuildNumber)")
    $lines.Add("MemoryGiB: $([Math]::Round($os.TotalVisibleMemorySize / 1MB, 2))")
} catch {
    $lines.Add("Windows: unavailable - $($_.Exception.Message)")
}

try {
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    $lines.Add("CPU: $($cpu.Name)")
    $lines.Add("LogicalProcessors: $($cpu.NumberOfLogicalProcessors)")
} catch {
    $lines.Add("CPU: unavailable - $($_.Exception.Message)")
}

$lines.Add("LoopbackTest: 127.0.0.1 dynamic TCP port")
$lines.Add("ExecutableSha256: $((Get-FileHash .\p5_manual_validation.exe -Algorithm SHA256).Hash)")
$lines.Add("")
$lines.Add("Manual verdict: PASS / CONDITIONAL / FAIL")
$lines.Add("Observed duplicate execution: ")
$lines.Add("Observed client divergence: ")
$lines.Add("Observed recovery rollback: ")
$lines.Add("Notes: ")

$path = Join-Path $PSScriptRoot "p5-machine-info.txt"
$lines | Set-Content -Encoding UTF8 $path
Write-Host "Wrote $path"
