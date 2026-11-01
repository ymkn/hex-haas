$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'dist\haas\hex.exe'
$wav = Join-Path $root 'tools\haas\fixtures\japanese.wav'
if (!(Test-Path $exe) -or !(Test-Path $wav)) { throw 'Build Hex and generate the synthetic native fixture first.' }
# Exercise ordinary direct launch: no special data/config/worker overrides.
foreach ($key in @('HEX_APPLICATION_SUPPORT_DIR','HEX_USER_CONFIG_DIR','HEX_EXPERIMENTS','HEX_SPEECH_CPP_EXE')) {
    if ([Environment]::GetEnvironmentVariable($key, 'Process')) { throw "Unset $key before testing the ordinary Haas package." }
}
$experiments = & $exe experiments list --json | ConvertFrom-Json
if ($LASTEXITCODE -or !($experiments.experiments | Where-Object { $_.name -eq 'haas-ja' -and $_.enabled -and 'build' -in $_.sources })) {
    throw 'The Haas backend is not available by default in this build.'
}
& $exe model check --model haas_ja_v1 --language ja
if ($LASTEXITCODE) { throw 'Haas model check failed.' }
foreach ($model in @('haas_ja_v1', 'whisper_large_v3_turbo')) {
    $output = & $exe model transcribe $wav --model $model --language ja
    if ($LASTEXITCODE) { throw "$model package inference failed." }
    if (![regex]::IsMatch(($output -join ''), '[\p{IsHiragana}\p{IsKatakana}\p{IsCJKUnifiedIdeographs}]')) {
        throw "$model produced no Japanese transcript."
    }
    Write-Output "$model : $output"
}
foreach ($theme in @('light','dark')) {
    $path = Join-Path $root "tools\haas\fixtures\picker-$theme.png"
    & $exe app -- --snap $path --scene picker-japanese --theme $theme --size 1100x1600
    if ($LASTEXITCODE -or !(Test-Path $path)) { throw "Picker render failed: $theme" }
}
Write-Output 'Package smoke checks passed. No microphone, shortcut listener, or clipboard paste was started.'
