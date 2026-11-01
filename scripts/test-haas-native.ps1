$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$fixtures = Join-Path $root 'tools\haas\fixtures'
New-Item -ItemType Directory -Force $fixtures | Out-Null
$voice = New-Object -ComObject SAPI.SpVoice
$voices = $voice.GetVoices()
$japanese = $null
for ($i=0; $i -lt $voices.Count; $i++) {
    $token = $voices.Item($i)
    if ($token.GetAttribute('Language') -match '(^|;)411(;|$)') { $japanese = $token; break }
}
if (!$japanese) { throw 'No Japanese SAPI voice is installed. Supply a WAV fixture instead; no microphone is opened.' }
$voice.Voice = $japanese
$file = Join-Path $fixtures 'japanese.wav'
$stream = New-Object -ComObject SAPI.SpFileStream
$stream.Format.Type = 22 # 22.05 kHz, 16-bit mono; Hex resamples to 16 kHz.
$stream.Open($file, 3, $false)
$voice.AudioOutputStream = $stream
$text = '今日は音声認識のテストをしています。日本語の文章を正しく入力できるか確認します。'
try { [void]$voice.Speak($text) } finally { $stream.Close() }
Write-Output "Fixture: $file"
Write-Output "Expected: $text"
& "$root\tools\haas\speech\speech.exe" asr "$env:APPDATA\hex-haas\models\Haas-616M-ja-v1.0-Q8_0.gguf" $file
if ($LASTEXITCODE) { throw "speech.cpp failed with exit code $LASTEXITCODE" }
