param([switch]$Check, [switch]$Lint, [switch]$BuildOnly, [string]$TestFilter, [string[]]$TestArguments)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$cmake = Join-Path $root 'tools\haas\cmake\cmake-4.4.5-windows-x86_64\bin'
$sdk = Join-Path $root 'tools\haas\vulkan'
if (!(Test-Path "$cmake\cmake.exe") -or !(Test-Path "$sdk\Bin\glslc.exe")) {
    throw 'Run scripts/setup-haas-build-tools.ps1 first.'
}
# Build the exact native fork pinned by Hex: upstream 0.4.1's binary lacks
# its Parakeet vocabulary extension and is not an ABI-compatible substitute.
Remove-Item Env:TRANSCRIBE_DIR -ErrorAction SilentlyContinue
$env:PATH = "$cmake;$sdk\Bin;$env:PATH"
$env:VULKAN_SDK = $sdk
$env:CMAKE_GENERATOR = 'Visual Studio 18 2026'
$env:TRANSCRIBE_CMAKE_ARGS = '-DGGML_NATIVE=OFF'
$env:CMAKE_BUILD_PARALLEL_LEVEL = '4'
# MSVC otherwise reads UTF-8 upstream C++ comments as the Japanese system
# code page, which can swallow source lines after a multibyte character.
$env:CFLAGS = '/utf-8'
$env:CXXFLAGS = '/utf-8'
$env:HEX_LOCAL_BUILD = '1'
$env:HEX_HAAS_BUILD = '1'
$env:CARGO_BUILD_JOBS = '4'
if ($Check -or $Lint -or $TestFilter) {
    $built = Get-ChildItem "$root\target\release\build\transcribe-cpp-sys-*\out\lib\transcribe-link.json" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($built) { $env:TRANSCRIBE_DIR = Split-Path (Split-Path $built.FullName) }
}
Push-Location $root
try {
    if ($TestFilter) {
        & 'C:\Program Files\Git\bin\bash.exe' scripts/check $TestFilter @TestArguments
    } elseif ($Lint) {
        cargo clippy --locked --all-targets -- -D warnings
    } elseif ($Check) {
        cargo check --locked --all-targets
    } else {
        cargo build --release --locked
    }
    if ($LASTEXITCODE) { throw "Cargo failed with exit code $LASTEXITCODE." }
    if (!$Check -and !$Lint -and !$TestFilter -and !$BuildOnly) {
        $dist = Join-Path $root 'dist\haas'
        New-Item -ItemType Directory -Force $dist | Out-Null
        Copy-Item "$root\target\release\hex.exe" $dist -Force
        Copy-Item "$root\tools\haas\speech\speech.exe", "$root\tools\haas\speech\speech.dll" $dist -Force
    }
} finally { Pop-Location }
