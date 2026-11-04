param([switch]$Check, [switch]$Lint, [switch]$BuildOnly, [string]$TestFilter, [string[]]$TestArguments)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $root 'tools\haas-cpu'
$cmake = Join-Path $tools 'cmake\cmake-4.4.5-windows-x86_64\bin'
if (!(Test-Path "$cmake\cmake.exe")) { throw 'Run scripts/setup-haas-cpu-build-tools.ps1 first.' }
# Never inherit a GPU prefix or configure arguments, even for checks/tests.
# No TRANSCRIBE_DIR reuse: all native rebuilds stay inside target-cpu.
Remove-Item Env:VULKAN_SDK -ErrorAction SilentlyContinue
Remove-Item Env:TRANSCRIBE_DIR -ErrorAction SilentlyContinue
$env:TRANSCRIBE_CMAKE_ARGS = '-DGGML_NATIVE=OFF -DTRANSCRIBE_VULKAN=OFF'
$env:CMAKE_ARGS = '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded'
$env:CARGO_TARGET_DIR = Join-Path $root 'target-cpu'
$env:PATH = "$cmake;$env:PATH"
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -or !$vs) { throw 'Visual Studio x64 C++ tools were not found.' }
$installationVersion = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
if ($LASTEXITCODE) { throw 'Could not query the Visual Studio version.' }
if ($installationVersion -match '^18\.') {
    $env:CMAKE_GENERATOR = 'Visual Studio 18 2026'
} else {
    Remove-Item Env:CMAKE_GENERATOR -ErrorAction SilentlyContinue
}
$env:CMAKE_BUILD_PARALLEL_LEVEL = '4'
$env:CFLAGS = '/utf-8'
$env:CXXFLAGS = '/utf-8'
$env:HEX_LOCAL_BUILD = '1'
$env:HEX_HAAS_BUILD = '1'
$env:CARGO_BUILD_JOBS = '4'
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
    if (!$Check -and !$Lint -and !$TestFilter) {
        $version = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt").Trim()
        $dumpbin = "$vs\VC\Tools\MSVC\$version\bin\Hostx64\x64\dumpbin.exe"
        if (!(Test-Path $dumpbin)) { throw 'MSVC dumpbin.exe was not found.' }
        $binaries = @("$root\target-cpu\release\hex.exe", "$tools\speech\speech.exe", "$tools\speech\speech.dll")
        foreach ($binary in $binaries) {
            if (!(Test-Path $binary)) { throw "Missing CPU binary: $binary. Run setup-haas-cpu-build-tools.ps1 first." }
            $dependencies = & $dumpbin /dependents $binary
            if ($LASTEXITCODE) { throw "dumpbin failed for $binary" }
            if ($dependencies -match '(?i)\bvulkan-1\.dll\b') { throw "Unexpected Vulkan dependency in $binary" }
            $dependencies | Write-Output
        }
        if (!$BuildOnly) {
            $dist = Join-Path $root 'dist\haas-cpu'
            New-Item -ItemType Directory -Force $dist | Out-Null
            Copy-Item $binaries $dist -Force
        }
    }
} finally { Pop-Location }
