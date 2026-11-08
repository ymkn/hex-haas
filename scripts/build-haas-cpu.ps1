param([switch]$Check, [switch]$Lint, [switch]$BuildOnly, [string]$TestFilter, [string[]]$TestArguments, [ValidateSet('x64', 'arm64')][string]$Arch = 'x64')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $root 'tools\haas-cpu'
$cmake = Join-Path $tools 'cmake\cmake-4.4.5-windows-x86_64\bin'
$targetDir = Join-Path $root 'target-cpu'
$releaseDir = Join-Path $targetDir 'release'
if ($Arch -eq 'arm64') {
    if ($env:PROCESSOR_ARCHITECTURE -ne 'ARM64') {
        throw 'The ARM64 CPU build needs an ARM64 Windows host: the pinned clang-cl is an ARM64 binary. Use a Surface-class PC or a windows-11-arm runner.'
    }
    $tools = Join-Path $root 'tools\haas-cpu-arm64'
    $cmake = Join-Path $tools 'cmake\cmake-4.4.5-windows-arm64\bin'
    $targetDir = Join-Path $root 'target-cpu-arm64'
    # CARGO_BUILD_TARGET keeps every cargo entry point (build, check, clippy,
    # scripts/check) on the aarch64 toolchain and its target subdirectory.
    $env:CARGO_BUILD_TARGET = 'aarch64-pc-windows-msvc'
    $releaseDir = Join-Path $targetDir 'aarch64-pc-windows-msvc\release'
    & rustup target add aarch64-pc-windows-msvc
    if ($LASTEXITCODE) { throw 'rustup could not install the aarch64-pc-windows-msvc target.' }
}
if ($Arch -eq 'arm64') {
    if (!(Test-Path "$cmake\cmake.exe")) { throw 'Run scripts/setup-haas-cpu-build-tools.ps1 -Arch arm64 first.' }
} elseif (!(Test-Path "$cmake\cmake.exe")) {
    throw 'Run scripts/setup-haas-cpu-build-tools.ps1 first.'
}
# Never inherit a GPU prefix or configure arguments, even for checks/tests.
# No TRANSCRIBE_DIR reuse: all native rebuilds stay inside target-cpu(-arm64).
Remove-Item Env:VULKAN_SDK -ErrorAction SilentlyContinue
Remove-Item Env:TRANSCRIBE_DIR -ErrorAction SilentlyContinue
$env:TRANSCRIBE_CMAKE_ARGS = '-DGGML_NATIVE=OFF -DTRANSCRIBE_VULKAN=OFF'
$env:CMAKE_ARGS = '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded'
$env:CARGO_TARGET_DIR = $targetDir
$env:PATH = "$cmake;$env:PATH"
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if ($Arch -eq 'arm64') {
    # clang-cl + Ninja, mirroring upstream's Windows ARM64 build and the
    # setup script's native core configuration.
    $env:PATH = "$tools\ninja;$tools\llvm\bin;$env:PATH"
    $env:CC = "$tools\llvm\bin\clang-cl.exe"
    $env:CXX = "$tools\llvm\bin\clang-cl.exe"
    $env:CMAKE_C_COMPILER = 'clang-cl'
    $env:CMAKE_CXX_COMPILER = 'clang-cl'
    $env:CMAKE_C_COMPILER_TARGET = 'aarch64-pc-windows-msvc'
    $env:CMAKE_CXX_COMPILER_TARGET = 'aarch64-pc-windows-msvc'
    Remove-Item Env:CMAKE_GENERATOR -ErrorAction SilentlyContinue
    $env:CMAKE_GENERATOR = 'Ninja'
    $env:TRANSCRIBE_CMAKE_ARGS = '-DGGML_NATIVE=OFF -DTRANSCRIBE_VULKAN=OFF -DGGML_OPENMP=OFF -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER_TARGET=aarch64-pc-windows-msvc -DCMAKE_CXX_COMPILER_TARGET=aarch64-pc-windows-msvc'
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationPath
    if ($LASTEXITCODE -or !$vs) {
        $vs = & $vswhere -latest -products '*' -property installationPath
    }
    if ($LASTEXITCODE -or !$vs) { throw 'Visual Studio with ARM64 C++ tools were not found.' }
    $installationVersion = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationVersion
    $msvcVersion = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt").Trim()
    $dumpbin = @("$vs\VC\Tools\MSVC\$msvcVersion\bin\Hostarm64\arm64\dumpbin.exe", "$vs\VC\Tools\MSVC\$msvcVersion\bin\Hostx64\arm64\dumpbin.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if (!$dumpbin) { throw 'MSVC ARM64 dumpbin.exe was not found.' }
} else {
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -or !$vs) { throw 'Visual Studio x64 C++ tools were not found.' }
    $installationVersion = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
    if ($LASTEXITCODE) { throw 'Could not query the Visual Studio version.' }
    if ($installationVersion -match '^18\.') {
        $env:CMAKE_GENERATOR = 'Visual Studio 18 2026'
    } else {
        Remove-Item Env:CMAKE_GENERATOR -ErrorAction SilentlyContinue
    }
}
$env:CMAKE_BUILD_PARALLEL_LEVEL = '4'
$env:CFLAGS = '/utf-8'
$env:CXXFLAGS = '/utf-8'
if ($Arch -eq 'arm64') {
    # clang-cl C++ targets need an explicit exceptions flag; MSVC's cl.exe
    # enables them by default, so the x64 configuration never sets this.
    $env:CL = '/EHsc'
}
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
        $binaries = @("$releaseDir\hex.exe", "$tools\speech\speech.exe", "$tools\speech\speech.dll")
        foreach ($binary in $binaries) {
            if (!(Test-Path $binary)) { throw "Missing CPU binary: $binary. Run setup-haas-cpu-build-tools.ps1 first." }
            $dependencies = & $dumpbin /dependents $binary
            if ($LASTEXITCODE) { throw "dumpbin failed for $binary" }
            if ($dependencies -match '(?i)\bvulkan-1\.dll\b') { throw "Unexpected Vulkan dependency in $binary" }
            if ($Arch -eq 'arm64') {
                $headers = & $dumpbin /headers $binary
                if ($LASTEXITCODE) { throw "dumpbin headers failed for $binary" }
                if (!($headers | Select-String -SimpleMatch 'machine (ARM64)')) { throw "$binary is not an ARM64 binary" }
            }
            $dependencies | Write-Output
        }
        if (!$BuildOnly) {
            $dist = Join-Path $root 'dist\haas-cpu'
            if ($Arch -eq 'arm64') { $dist = Join-Path $root 'dist\haas-cpu-arm64' }
            New-Item -ItemType Directory -Force $dist | Out-Null
            Copy-Item $binaries $dist -Force
        }
    }
} finally { Pop-Location }
