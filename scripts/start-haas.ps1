$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
# Pick the most specific built variant: GPU, then x64 CPU, then ARM64 CPU.
$candidates = @('dist\haas', 'dist\haas-cpu', 'dist\haas-cpu-arm64')
$exe = $null
foreach ($relative in $candidates) {
    $path = Join-Path $root "$relative\hex.exe"
    if (Test-Path $path) { $exe = $path; break }
}
if (!$exe) { throw 'Hex Haas is not built yet. Run scripts/build-haas.ps1 or scripts/build-haas-cpu.ps1 (-Arch arm64 on an ARM64 PC) first.' }
Start-Process -FilePath $exe -ArgumentList 'app' -WorkingDirectory (Split-Path $exe)
