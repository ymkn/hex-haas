param([ValidateSet('x64', 'arm64')][string]$Arch = 'x64')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'haas-common.ps1')
$root = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $root 'tools\haas-cpu'
if ($Arch -eq 'arm64') { $tools = Join-Path $root 'tools\haas-cpu-arm64' }
New-Item -ItemType Directory -Force $tools | Out-Null

# Verify even cached archives before extracting. CPU tools never reuse GPU files.
if ($Arch -eq 'x64') {
    $cmakeZip = Join-Path $tools 'cmake.zip'
    if (!(Test-Path $cmakeZip)) {
        Get-PinnedDownload 'https://github.com/Kitware/CMake/releases/download/v4.4.5/cmake-4.4.5-windows-x86_64.zip' -OutFile $cmakeZip
    }
    if ((Get-FileHash $cmakeZip -Algorithm SHA256).Hash -ne '326c826486d46a23cf9d5a1c9ee3a567d427bdd02c4a34071937a2a675cff004') { throw 'CMake checksum mismatch' }
    if (!(Test-Path "$tools\cmake\cmake-4.4.5-windows-x86_64\bin\cmake.exe")) { Expand-Archive $cmakeZip "$tools\cmake" }
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -or !$vs) { throw 'Visual Studio x64 C++ tools were not found.' }
    $version = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt").Trim()
    $dumpbin = "$vs\VC\Tools\MSVC\$version\bin\Hostx64\x64\dumpbin.exe"
    if (!(Test-Path $dumpbin)) { throw 'MSVC dumpbin.exe was not found.' }
    $installationVersion = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
    if ($LASTEXITCODE) { throw 'Could not query the Visual Studio version.' }
    if ($installationVersion -match '^18\.') {
        $env:CMAKE_GENERATOR = 'Visual Studio 18 2026'
    } else {
        Remove-Item Env:CMAKE_GENERATOR -ErrorAction SilentlyContinue
    }
    $env:CFLAGS = '/utf-8'
    $env:CXXFLAGS = '/utf-8'
    $cmake = "$tools\cmake\cmake-4.4.5-windows-x86_64\bin\cmake.exe"
    $generatorOptions = @('-A', 'x64')
    $speechOutput = "$tools\speech-build\Release"
} else {
    # ARM64 builds run on an ARM64 Windows host (this PC, or GitHub's
    # windows-11-arm runner): the pinned CMake, Ninja, and LLVM releases are
    # ARM64 Windows binaries. ggml's ARM CPU backend refuses MSVC, so the
    # native core builds with clang-cl through Ninja, mirroring upstream's
    # Windows ARM64 build (GGML_NATIVE=OFF, ggml's own threadpool instead of
    # OpenMP).
    $cmakeZip = Join-Path $tools 'cmake.zip'
    if (!(Test-Path $cmakeZip)) {
        Get-PinnedDownload 'https://github.com/Kitware/CMake/releases/download/v4.4.5/cmake-4.4.5-windows-arm64.zip' -OutFile $cmakeZip
    }
    if ((Get-FileHash $cmakeZip -Algorithm SHA256).Hash -ne 'B0B54EBDD3E7B9F5AD733D865541D6E0A3B774182E4318B26890221432835359') { throw 'CMake checksum mismatch' }
    if (!(Test-Path "$tools\cmake\cmake-4.4.5-windows-arm64\bin\cmake.exe")) {
        New-Item -ItemType Directory -Force "$tools\cmake" | Out-Null
        tar -xf $cmakeZip -C "$tools\cmake"
        if ($LASTEXITCODE) { throw 'CMake extraction failed.' }
    }
    $ninjaZip = Join-Path $tools 'ninja.zip'
    if (!(Test-Path $ninjaZip)) {
        Get-PinnedDownload 'https://github.com/ninja-build/ninja/releases/download/v1.12.1/ninja-winarm64.zip' -OutFile $ninjaZip
    }
    if ((Get-FileHash $ninjaZip -Algorithm SHA256).Hash -ne '79C96A50E0DEAFEC212CFA85AA57C6B74003F52D9D1673DDCD1EAB1C958C5900') { throw 'Ninja checksum mismatch' }
    if (!(Test-Path "$tools\ninja\ninja.exe")) {
        New-Item -ItemType Directory -Force "$tools\ninja" | Out-Null
        tar -xf $ninjaZip -C "$tools\ninja"
        if ($LASTEXITCODE) { throw 'Ninja extraction failed.' }
        if (!(Test-Path "$tools\ninja\ninja.exe")) { throw 'ninja.exe was not extracted.' }
    }
    $llvmArchive = Join-Path $tools 'llvm.tar.xz'
    if (!(Test-Path $llvmArchive)) {
        Get-PinnedDownload 'https://github.com/llvm/llvm-project/releases/download/llvmorg-22.1.8/clang+llvm-22.1.8-aarch64-pc-windows-msvc.tar.xz' -OutFile $llvmArchive
    }
    if ((Get-FileHash $llvmArchive -Algorithm SHA256).Hash -ne 'DE718C58EBBC5F61D58C17B90457FCF42983BC2C4A4ABA3E010D108713BFD7F1') { throw 'LLVM checksum mismatch' }
    if (!(Test-Path "$tools\llvm\bin\clang-cl.exe")) {
        New-Item -ItemType Directory -Force "$tools\llvm" | Out-Null
        tar -xJf $llvmArchive -C "$tools\llvm" --strip-components 1
        if ($LASTEXITCODE) { throw 'LLVM extraction failed.' }
        if (!(Test-Path "$tools\llvm\bin\clang-cl.exe")) { throw 'clang-cl.exe was not extracted.' }
    }
    # Locate an MSVC ARM64 toolset: clang-cl uses its link.exe, import
    # libraries, and the Windows SDK headers. The component query comes
    # first; any install that actually ships the ARM64 binaries also works.
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationPath
    if ($LASTEXITCODE -or !$vs) {
        $vs = & $vswhere -latest -products '*' -property installationPath
    }
    if ($LASTEXITCODE -or !$vs) { throw 'Visual Studio with ARM64 C++ tools was not found.' }
    $version = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt").Trim()
    $dumpbin = @("$vs\VC\Tools\MSVC\$version\bin\Hostarm64\arm64\dumpbin.exe", "$vs\VC\Tools\MSVC\$version\bin\Hostx64\arm64\dumpbin.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if (!$dumpbin) { throw 'MSVC ARM64 dumpbin.exe was not found.' }
    Remove-Item Env:CMAKE_GENERATOR -ErrorAction SilentlyContinue
    $env:CMAKE_GENERATOR = 'Ninja'
    $env:CMAKE_C_COMPILER_TARGET = 'aarch64-pc-windows-msvc'
    $env:CMAKE_CXX_COMPILER_TARGET = 'aarch64-pc-windows-msvc'
    # MSVC otherwise reads UTF-8 upstream C++ comments as the Japanese system
    # code page, which can swallow source lines after a multibyte character.
    $env:CFLAGS = '/utf-8'
    $env:CXXFLAGS = '/utf-8 /EHsc'
    $env:CL = '/EHsc'
    $cmake = "$tools\cmake\cmake-4.4.5-windows-arm64\bin\cmake.exe"
    $env:PATH = "$tools\ninja;$tools\llvm\bin;$env:PATH"
    $generatorOptions = @('-G', 'Ninja',
        '-DCMAKE_BUILD_TYPE=Release',
        '-DCMAKE_C_COMPILER=clang-cl',
        '-DCMAKE_CXX_COMPILER=clang-cl',
        '-DCMAKE_C_COMPILER_TARGET=aarch64-pc-windows-msvc',
        '-DCMAKE_CXX_COMPILER_TARGET=aarch64-pc-windows-msvc')
}
$speechArchive = Join-Path $tools 'speech-source.zip'
if (!(Test-Path $speechArchive)) {
    Get-PinnedDownload 'https://github.com/nyosegawa/speech.cpp/archive/06590811a90041f935a470ba0e2b85346d2fc041.zip' -OutFile $speechArchive
}
if ((Get-FileHash $speechArchive -Algorithm SHA256).Hash -ne '111cc186abb0e20fbdfa222c0f9f5e52976da1fc51680a0b834a43606a02667e') { throw 'speech.cpp source checksum mismatch' }
if (!(Test-Path "$tools\speech-source")) {
    New-Item -ItemType Directory -Force "$tools\speech-source" | Out-Null
    tar -xf $speechArchive -C "$tools\speech-source"
    if ($LASTEXITCODE) { throw 'speech.cpp source extraction failed.' }
}
# GitHub's zip omits submodules: obtain the exact ggml commit separately.
$ggmlArchive = Join-Path $tools 'ggml-source.zip'
if (!(Test-Path $ggmlArchive)) {
    Get-PinnedDownload 'https://github.com/nyosegawa/ggml/archive/700c9d743ed7cc583c0f1b6507afebb6c0e88bc0.zip' -OutFile $ggmlArchive
}
if ((Get-FileHash $ggmlArchive -Algorithm SHA256).Hash -ne '3b2722df7f79f02b3f9e5c0acccce2bfa8ff0c687abbd3090ac206bc60fed9b9') { throw 'ggml source checksum mismatch' }
if (!(Test-Path "$tools\ggml-source")) {
    New-Item -ItemType Directory -Force "$tools\ggml-source" | Out-Null
    tar -xf $ggmlArchive -C "$tools\ggml-source"
    if ($LASTEXITCODE) { throw 'ggml source extraction failed.' }
}

$speechSource = "$tools\speech-source\speech.cpp-06590811a90041f935a470ba0e2b85346d2fc041"
$ggmlSource = "$tools\ggml-source\ggml-700c9d743ed7cc583c0f1b6507afebb6c0e88bc0"
# A moved checkout cannot reuse CMake caches with absolute paths. Only these
# generated CPU build/install directories are cleared, never GPU tools.
foreach ($directory in @('ggml-build', 'ggml-install', 'speech-build')) {
    Remove-Item (Join-Path $tools $directory) -Recurse -Force -ErrorAction SilentlyContinue
}
$options = $generatorOptions + @('-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded')
& $cmake -S $ggmlSource -B "$tools\ggml-build" @options "-DCMAKE_INSTALL_PREFIX=$tools\ggml-install" -DGGML_VULKAN=OFF -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DBUILD_SHARED_LIBS=OFF -DGGML_BUILD_TESTS=OFF -DGGML_BUILD_EXAMPLES=OFF
if ($LASTEXITCODE) { throw 'CPU ggml configure failed' }
& $cmake --build "$tools\ggml-build" --config Release --parallel 4
if ($LASTEXITCODE) { throw 'CPU ggml build failed' }
& $cmake --install "$tools\ggml-build" --config Release
if ($LASTEXITCODE) { throw 'CPU ggml install failed' }
# Match ggml's architecture and static CRT in speech too. The speech DLL
# remains shared, but embeds ggml and the CRT; no ggml/OpenMP DLL is packaged.
& $cmake -S $speechSource -B "$tools\speech-build" @options -DSPEECH_INSTALLED_GGML=ON "-DCMAKE_PREFIX_PATH=$tools\ggml-install"
if ($LASTEXITCODE) { throw 'CPU speech.cpp configure failed' }
& $cmake --build "$tools\speech-build" --config Release --target speech-cli speech --parallel 4
if ($LASTEXITCODE) { throw 'CPU speech.cpp build failed' }
New-Item -ItemType Directory -Force "$tools\speech" | Out-Null
if ($Arch -eq 'arm64') {
    # The Ninja generator writes binaries into the build directory root, not
    # a Release subdirectory.
    $speechOutput = "$tools\speech-build"
    if (!(Test-Path "$speechOutput\speech.exe") -or !(Test-Path "$speechOutput\speech.dll")) {
        throw 'ARM64 speech.exe / speech.dll were not built.'
    }
}
foreach ($file in @('speech.exe', 'speech.dll')) {
    $binary = Join-Path $speechOutput $file
    if (!(Test-Path $binary)) { throw "CPU $file was not built." }
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
Copy-Item (Join-Path $speechOutput 'speech.exe'), (Join-Path $speechOutput 'speech.dll') "$tools\speech" -Force
Get-FileHash $cmakeZip, $speechArchive, $ggmlArchive -Algorithm SHA256 | Format-List Path,Hash
if ($Arch -eq 'arm64') {
    Get-FileHash (Join-Path $tools 'ninja.zip'), (Join-Path $tools 'llvm.tar.xz') -Algorithm SHA256 | Format-List Path,Hash
}
