param([string]$Destination = (Join-Path $PSScriptRoot '..\dist\package-cpu'))
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$root = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $root 'dist\haas-cpu'
foreach ($file in @('hex.exe', 'speech.exe', 'speech.dll')) {
    if (!(Test-Path (Join-Path $dist $file))) { throw "dist\haas-cpu\$file is missing; run scripts/build-haas-cpu.ps1 first." }
}
$versionOutput = & (Join-Path $dist 'hex.exe') --version
if ($LASTEXITCODE) { throw 'CPU hex.exe --version failed.' }
$version = ($versionOutput -split '\s+') | Select-Object -Last 1
if ($version -notmatch '^[0-9A-Za-z][0-9A-Za-z.+_-]{0,127}$') { throw "Unexpected version '$version'." }

$name = "Hex-Haas-$version-windows-x64-cpu"
$stage = Join-Path $Destination $name
New-Item -ItemType Directory -Force $stage | Out-Null
foreach ($file in @('hex.exe', 'speech.exe', 'speech.dll')) {
    Copy-Item (Join-Path $dist $file) $stage -Force
}
Copy-Item (Join-Path $root 'LICENSE') $stage -Force
Copy-Item (Join-Path $root 'packaging\haas\README-Haas-CPU.txt') $stage -Force
$zip = Join-Path $Destination "$name.zip"
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
Remove-Item $stage -Recurse -Force

$sha = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
$sums = Join-Path $Destination 'SHA256SUMS'
@("$sha  $name.zip") | Set-Content $sums -Encoding ascii
@(
    "Hex Haas $version - Windows x64 CPU (unofficial fork build)",
    '',
    '- CPU inference: no Vulkan GPU driver or vulkan-1.dll required.',
    '- Requires AVX2 (Haswell / Excavator or newer); CPU inference is slower.',
    '- hex.exe with the Haas-ja-v1.0 speech.cpp backend (speech.cpp v0.10.0 included).',
    "- No code signature: verify $name.zip against SHA256SUMS before running.",
    '- SmartScreen may warn on first launch; see README-Haas-CPU.txt inside the zip.',
    '- The Haas model is not included; download it from the model picker at first run.'
) | Set-Content (Join-Path $Destination 'RELEASE-NOTES.md') -Encoding utf8NoBOM
Write-Output "$zip"
Write-Output "  sha256: $sha"
