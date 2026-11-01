$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'dist\haas\hex.exe'
if (!(Test-Path $exe)) { throw 'The experimental Hex binary is not built yet.' }
$other = Get-Process hex -ErrorAction SilentlyContinue
if ($other) { throw 'Quit the currently running Hex from its tray menu before starting Hex Haas. No process has been stopped.' }
$data = Join-Path $root 'haas-data'
$config = Join-Path $root 'haas-config'
New-Item -ItemType Directory -Force $data, $config | Out-Null
$settingsFile = Join-Path $data 'settings.json'
if (!(Test-Path $settingsFile)) {
    $original = Join-Path $env:APPDATA 'hex\settings.json'
    $settings = if (Test-Path $original) {
        Get-Content $original -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    } else { @{} }
    $settings.transcription = @{model='haas_ja_v1'; language='ja'; recognition_hints=''}
    $settings.transcription_recents = @()
    $settings.enable_on_launch = $true
    $settings | ConvertTo-Json -Depth 100 | Set-Content $settingsFile -Encoding utf8NoBOM
    $marker = Join-Path $env:APPDATA 'hex\onboarding-complete'
    if (Test-Path $marker) { Copy-Item $marker (Join-Path $data 'onboarding-complete') }
}
$keys = @('HEX_APPLICATION_SUPPORT_DIR','HEX_USER_CONFIG_DIR','HEX_SPEECH_CPP_EXE','HEX_EXPERIMENTS')
$previous = @{}
foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
try {
    $env:HEX_APPLICATION_SUPPORT_DIR = $data
    $env:HEX_USER_CONFIG_DIR = $config
    $env:HEX_SPEECH_CPP_EXE = Join-Path $root 'dist\haas\speech.exe'
    $env:HEX_EXPERIMENTS = 'haas-ja'
    Start-Process -FilePath $exe -ArgumentList 'app' -WorkingDirectory (Split-Path $exe)
} finally {
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
}
