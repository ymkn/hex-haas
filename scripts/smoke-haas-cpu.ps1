# Smoke checks for a built CPU variant, shared by the x64 and ARM64 jobs
# in release-haas.yml (they used to carry identical inline copies).
#   -Phase Binaries:  no Vulkan loader inside the package, and hex.exe /
#                     speech.exe start and enumerate devices.
#   -Phase Inference: download the Haas model and transcribe a synthetic
#                     WAV (a deterministic waveform, not speech).
param([ValidateSet('x64', 'arm64')][string]$Arch = 'x64',
      [ValidateSet('Binaries', 'Inference')][string]$Phase = 'Binaries')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $root 'dist\haas-cpu'
$label = 'CPU'
$smokeName = 'hex-haas-cpu-smoke'
if ($Arch -eq 'arm64') {
    $dist = Join-Path $root 'dist\haas-cpu-arm64'
    $label = 'ARM64 CPU'
    $smokeName = 'hex-haas-arm64-cpu-smoke'
}
if ($Phase -eq 'Binaries') {
    # The load-time proof is the dumpbin import scan in the build steps:
    # none of the three binaries imports vulkan-1.dll (and for ARM64 all
    # three are ARM64 machine-type binaries), so a loader on this runner
    # cannot be what makes them start. A loader shipped inside the package
    # would break that separation, so refuse only that; a system loader is
    # noted, not fatal.
    $packaged = @(Get-ChildItem $dist -Filter vulkan-1.dll -Recurse | ForEach-Object { $_.FullName })
    foreach ($loader in $packaged) {
        throw "The CPU package must not contain a Vulkan loader, but contains $loader"
    }
    $system = @("$env:WINDIR\System32\vulkan-1.dll")
    $system += ($env:PATH -split ';' | Where-Object { $_ } | ForEach-Object { Join-Path $_ 'vulkan-1.dll' })
    foreach ($loader in $system) {
        if (Test-Path $loader) { Write-Warning "This runner has a Vulkan loader at $loader; startup here does not by itself prove loader-free operation (the import scan does)." }
    }
    & (Join-Path $dist 'hex.exe') --version
    if ($LASTEXITCODE) { throw "$label Hex failed to start." }
    & (Join-Path $dist 'speech.exe') --version
    if ($LASTEXITCODE) { throw "$label speech.cpp failed to start." }
    & (Join-Path $dist 'hex.exe') model devices
    if ($LASTEXITCODE) { throw "$label device enumeration failed." }
} else {
    # Required, not continue-on-error: Hugging Face access and a real
    # model load/worker inference must succeed before uploading a zip.
    # Japanese SAPI voices are NOT assumed available on the runner. A
    # deterministic non-silent waveform exercises inference without SAPI,
    # a microphone, playback, or a Japanese accuracy claim. Empty text is
    # allowed; an inference error/nonzero exit is not. Native Japanese
    # speech, shortcut/paste, and GUI rendering need a real PC.
    $env:HEX_APPLICATION_SUPPORT_DIR = Join-Path $env:RUNNER_TEMP $smokeName
    New-Item -ItemType Directory -Force $env:HEX_APPLICATION_SUPPORT_DIR | Out-Null
    & (Join-Path $dist 'hex.exe') model install haas_ja_v1 --language ja
    if ($LASTEXITCODE) { throw 'Haas model download failed; inference was NOT verified.' }
    $wav = Join-Path $env:RUNNER_TEMP 'haas-cpu-synthetic.wav'
    $rate = 16000
    $count = 3 * $rate
    $writer = [System.IO.BinaryWriter]::new([System.IO.File]::Create($wav))
    try {
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes('RIFF'))
        $writer.Write([int](36 + 2 * $count))
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
        $writer.Write([int]16)
        $writer.Write([int16]1)
        $writer.Write([int16]1)
        $writer.Write([int]$rate)
        $writer.Write([int](2 * $rate))
        $writer.Write([int16]2)
        $writer.Write([int16]16)
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes('data'))
        $writer.Write([int](2 * $count))
        for ($i = 0; $i -lt $count; $i++) {
            $t = $i / [double]$rate
            $sample = 6000 * [Math]::Sin(2 * [Math]::PI * 220 * $t) + 3000 * [Math]::Sin(2 * [Math]::PI * 440 * $t)
            $writer.Write([int16]$sample)
        }
    } finally { $writer.Dispose() }
    & (Join-Path $dist 'hex.exe') model transcribe $wav --model haas_ja_v1 --language ja
    if ($LASTEXITCODE) { throw "Haas $label inference failed." }
}
