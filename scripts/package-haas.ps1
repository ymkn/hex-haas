param([string]$Destination = (Join-Path $PSScriptRoot '..\dist\package'),
      [string]$Channel = $(if ($env:HAAS_CHANNEL) { $env:HAAS_CHANNEL } else { 'stable' }),
      [string]$Build = $(if ($env:HAAS_BUILD) { $env:HAAS_BUILD } else { Get-Date -Format 'yyyyMMdd-HHmm' }),
      [string]$Source = $(if ($env:HAAS_SOURCE) { $env:HAAS_SOURCE } else { '' }))
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
# Build marker for `hex haas-upgrade`: the client reads haas-build.json next
# to hex.exe to learn its variant, channel and build; the publish job reads
# the sidecar copy left in $Destination to compose the release manifest.json.
$marker = [ordered]@{
    product = 'hex-haas'
    variant = 'gpu'
    channel = $Channel
    version = $version
    build   = $Build
    source  = $Source
} | ConvertTo-Json
Set-Content -Path (Join-Path $stage 'haas-build.json') -Value $marker -Encoding utf8NoBOM
Set-Content -Path (Join-Path $Destination 'haas-build.json') -Value $marker -Encoding utf8NoBOM
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
