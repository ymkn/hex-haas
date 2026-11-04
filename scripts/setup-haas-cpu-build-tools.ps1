$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $root 'tools\haas-cpu'
New-Item -ItemType Directory -Force $tools | Out-Null

# Verify even cached archives before extracting. CPU tools never reuse GPU files.
$cmakeZip = Join-Path $tools 'cmake.zip'
if (!(Test-Path $cmakeZip)) {
    Invoke-WebRequest 'https://github.com/Kitware/CMake/releases/download/v4.4.5/cmake-4.4.5-windows-x86_64.zip' -OutFile $cmakeZip
}
if ((Get-FileHash $cmakeZip -Algorithm SHA256).Hash -ne '326c826486d46a23cf9d5a1c9ee3a567d427bdd02c4a34071937a2a675cff004') { throw 'CMake checksum mismatch' }
if (!(Test-Path "$tools\cmake\cmake-4.4.5-windows-x86_64\bin\cmake.exe")) { Expand-Archive $cmakeZip "$tools\cmake" }
$speechArchive = Join-Path $tools 'speech-source.zip'
if (!(Test-Path $speechArchive)) {
    Invoke-WebRequest 'https://github.com/nyosegawa/speech.cpp/archive/06590811a90041f935a470ba0e2b85346d2fc041.zip' -OutFile $speechArchive
}
if ((Get-FileHash $speechArchive -Algorithm SHA256).Hash -ne '111cc186abb0e20fbdfa222c0f9f5e52976da1fc51680a0b834a43606a02667e') { throw 'speech.cpp source checksum mismatch' }
if (!(Test-Path "$tools\speech-source")) { Expand-Archive $speechArchive "$tools\speech-source" }
# GitHub's zip omits submodules: obtain the exact ggml commit separately.
$ggmlArchive = Join-Path $tools 'ggml-source.zip'
if (!(Test-Path $ggmlArchive)) {
    Invoke-WebRequest 'https://github.com/nyosegawa/ggml/archive/700c9d743ed7cc583c0f1b6507afebb6c0e88bc0.zip' -OutFile $ggmlArchive
}
if ((Get-FileHash $ggmlArchive -Algorithm SHA256).Hash -ne '3b2722df7f79f02b3f9e5c0acccce2bfa8ff0c687abbd3090ac206bc60fed9b9') { throw 'ggml source checksum mismatch' }
if (!(Test-Path "$tools\ggml-source")) { Expand-Archive $ggmlArchive "$tools\ggml-source" }

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
$speechSource = "$tools\speech-source\speech.cpp-06590811a90041f935a470ba0e2b85346d2fc041"
$ggmlSource = "$tools\ggml-source\ggml-700c9d743ed7cc583c0f1b6507afebb6c0e88bc0"
# A moved checkout cannot reuse CMake caches with absolute paths. Only these
# generated CPU build/install directories are cleared, never GPU tools.
foreach ($directory in @('ggml-build', 'ggml-install', 'speech-build')) {
    Remove-Item (Join-Path $tools $directory) -Recurse -Force -ErrorAction SilentlyContinue
}
$options = @('-A', 'x64', '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded')
& $cmake -S $ggmlSource -B "$tools\ggml-build" @options "-DCMAKE_INSTALL_PREFIX=$tools\ggml-install" -DGGML_VULKAN=OFF -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DBUILD_SHARED_LIBS=OFF -DGGML_BUILD_TESTS=OFF -DGGML_BUILD_EXAMPLES=OFF
if ($LASTEXITCODE) { throw 'CPU ggml configure failed' }
& $cmake --build "$tools\ggml-build" --config Release --parallel 4
if ($LASTEXITCODE) { throw 'CPU ggml build failed' }
& $cmake --install "$tools\ggml-build" --config Release
if ($LASTEXITCODE) { throw 'CPU ggml install failed' }
# Match ggml's x64 architecture and static CRT in speech too. The speech DLL
# remains shared, but embeds ggml and the CRT; no ggml/OpenMP DLL is packaged.
& $cmake -S $speechSource -B "$tools\speech-build" @options -DSPEECH_INSTALLED_GGML=ON "-DCMAKE_PREFIX_PATH=$tools\ggml-install"
if ($LASTEXITCODE) { throw 'CPU speech.cpp configure failed' }
& $cmake --build "$tools\speech-build" --config Release --target speech-cli speech --parallel 4
if ($LASTEXITCODE) { throw 'CPU speech.cpp build failed' }
foreach ($file in @('speech.exe', 'speech.dll')) {
    $binary = "$tools\speech-build\Release\$file"
    if (!(Test-Path $binary)) { throw "CPU $file was not built." }
    $dependencies = & $dumpbin /dependents $binary
    if ($LASTEXITCODE) { throw "dumpbin failed for $binary" }
    if ($dependencies -match '(?i)\bvulkan-1\.dll\b') { throw "Unexpected Vulkan dependency in $binary" }
    $dependencies | Write-Output
}
New-Item -ItemType Directory -Force "$tools\speech" | Out-Null
Copy-Item "$tools\speech-build\Release\speech.exe", "$tools\speech-build\Release\speech.dll" "$tools\speech" -Force
Get-FileHash $cmakeZip, $speechArchive, $ggmlArchive -Algorithm SHA256 | Format-List Path,Hash
