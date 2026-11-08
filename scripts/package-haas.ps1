param([string]$Destination = (Join-Path $PSScriptRoot '..\dist\package'))
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$root = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $root 'dist\haas'
foreach ($file in @('hex.exe', 'speech.exe', 'speech.dll')) {
    if (!(Test-Path (Join-Path $dist $file))) { throw "dist\haas\$file is missing; run scripts/build-haas.ps1 first." }
}
$versionOutput = (& (Join-Path $dist 'hex.exe') --version 2>&1 | Out-String)
if ($LASTEXITCODE) { throw 'hex.exe --version failed.' }
# Drop whitespace-only tokens: the version is the last non-empty one
# ("hex 3.3.1", or "hex 0.0.0-dev-<run>" on a dev build).
$version = (@($versionOutput -split '\s+') | Where-Object { $_ } | Select-Object -Last 1)
if (!$version -or ($version -notmatch '^[0-9A-Za-z][0-9A-Za-z.+_-]{0,127}$')) { throw "Unexpected version '$version'." }

$name = "Hex-Haas-$version-windows-x64"
$stage = Join-Path $Destination $name
New-Item -ItemType Directory -Force $stage | Out-Null
foreach ($file in @('hex.exe', 'speech.exe', 'speech.dll')) {
    Copy-Item (Join-Path $dist $file) $stage -Force
}
Copy-Item (Join-Path $root 'LICENSE') $stage -Force
Copy-Item (Join-Path $root 'packaging\haas\README-Haas.txt') $stage -Force
$zip = Join-Path $Destination "$name.zip"
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
Remove-Item $stage -Recurse -Force

$sha = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
$sums = Join-Path $Destination 'SHA256SUMS'
@("$sha  $name.zip") | Set-Content $sums -Encoding ascii
@(
    "Hex Haas $version - Windows x64 (unofficial fork build)",
    ''
    "- hex.exe with the Haas-ja-v1.0 speech.cpp backend (speech.cpp v0.10.0 included)."
    "- No code signature: verify $name.zip against SHA256SUMS before running."
    "- SmartScreen may warn on first launch; see README-Haas.txt inside the zip."
    "- The Haas model is not included; download it from the model picker at first run."
) | Set-Content (Join-Path $Destination 'RELEASE-NOTES.md') -Encoding utf8NoBOM
Write-Output "$zip"
Write-Output "  sha256: $sha"
