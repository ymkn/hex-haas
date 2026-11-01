$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $root 'tools\haas'
New-Item -ItemType Directory -Force $tools | Out-Null
$cmakeZip = Join-Path $tools 'cmake.zip'
if (!(Test-Path $cmakeZip)) {
    Invoke-WebRequest 'https://github.com/Kitware/CMake/releases/download/v4.4.5/cmake-4.4.5-windows-x86_64.zip' -OutFile $cmakeZip
}
if ((Get-FileHash $cmakeZip -Algorithm SHA256).Hash -ne '326c826486d46a23cf9d5a1c9ee3a567d427bdd02c4a34071937a2a675cff004') { throw 'CMake checksum mismatch' }
if (!(Test-Path "$tools\cmake\cmake-4.4.5-windows-x86_64\bin\cmake.exe")) { Expand-Archive $cmakeZip "$tools\cmake" }
$shaderArchive = Join-Path $tools 'shaderc.zip'
if (!(Test-Path $shaderArchive)) {
    Invoke-WebRequest 'https://storage.googleapis.com/shaderc/artifacts/prod/graphics_shader_compiler/shaderc/windows-vs2022-amd64-release/continuous/53/20261028-151553/install.zip' -OutFile $shaderArchive
}
if ((Get-FileHash $shaderArchive -Algorithm SHA256).Hash -ne '2d5358873b2726a15ed9ab6fe45143a76913f4d62a8434a6f6a1a471bdfe1270') { throw 'shaderc checksum mismatch (pinned Google build attestation)' }
$shader = Join-Path $tools 'shaderc'
if (!(Test-Path $shader)) {
    Expand-Archive $shaderArchive $shader
}
$headersArchive = Join-Path $tools 'vulkan-headers.zip'
if (!(Test-Path $headersArchive)) {
    Invoke-WebRequest 'https://github.com/KhronosGroup/Vulkan-Headers/archive/6802bb4733b63ed5efd3adb308a6c885ef180ea1.zip' -OutFile $headersArchive
}
if ((Get-FileHash $headersArchive -Algorithm SHA256).Hash -ne '3760505ffbd3589de77e1b9f5ba11eaac13d3c86869588e19f3df3e0fac7e69c') { throw 'Vulkan-Headers checksum mismatch' }
if (!(Test-Path "$tools\vulkan-headers")) { Expand-Archive $headersArchive "$tools\vulkan-headers" }
$sdk = Join-Path $tools 'vulkan'
New-Item -ItemType Directory -Force "$sdk\Include", "$sdk\Lib", "$sdk\Bin" | Out-Null
$include = Get-ChildItem "$tools\vulkan-headers" -Directory | Select-Object -First 1
Copy-Item "$($include.FullName)\include\*" "$sdk\Include" -Recurse -Force
$spirvArchive = Join-Path $tools 'spirv-headers.zip'
if (!(Test-Path $spirvArchive)) {
    Invoke-WebRequest 'https://github.com/KhronosGroup/SPIRV-Headers/archive/496543121ce6419f23d6fa5d7194ba66c36212d2.zip' -OutFile $spirvArchive
}
if ((Get-FileHash $spirvArchive -Algorithm SHA256).Hash -ne 'b33782a6ccd124e6e02634f6382456b0aba2242c6ec5b68d07be20ee77a5b6bc') { throw 'SPIRV-Headers checksum mismatch' }
if (!(Test-Path "$tools\spirv-headers")) { Expand-Archive $spirvArchive "$tools\spirv-headers" }
$spirvSource = Get-ChildItem "$tools\spirv-headers" -Directory | Select-Object -First 1
$cmake = "$tools\cmake\cmake-4.4.5-windows-x86_64\bin\cmake.exe"
& $cmake -S $spirvSource.FullName -B "$tools\spirv-build" "-DCMAKE_INSTALL_PREFIX=$sdk" -DSPIRV_HEADERS_ENABLE_TESTS=OFF
if ($LASTEXITCODE) { throw 'SPIRV-Headers configure failed' }
& $cmake --install "$tools\spirv-build"
if ($LASTEXITCODE) { throw 'SPIRV-Headers install failed' }
$glslc = Get-ChildItem $shader -Filter glslc.exe -Recurse | Select-Object -First 1
if (!$glslc) { throw 'glslc.exe is missing from the shaderc package' }
Copy-Item "$($glslc.DirectoryName)\*" "$sdk\Bin" -Recurse -Force
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$version = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt").Trim()
$bin = "$vs\VC\Tools\MSVC\$version\bin\Hostx64\x64"
$exports = & "$bin\dumpbin.exe" /exports "$env:WINDIR\System32\vulkan-1.dll"
$names = @($exports | ForEach-Object { if ($_ -match '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(vk\w+)\s*$') { $Matches[1] } })
if ($names.Count -lt 100) { throw 'Could not read the installed Vulkan loader exports' }
@('LIBRARY vulkan-1', 'EXPORTS') + $names | Set-Content "$sdk\Lib\vulkan-1.def" -Encoding ascii
& "$bin\lib.exe" /nologo "/def:$sdk\Lib\vulkan-1.def" /machine:x64 "/out:$sdk\Lib\vulkan-1.lib"
if ($LASTEXITCODE) { throw 'Vulkan import library creation failed' }
Get-FileHash $cmakeZip, $shaderArchive, $headersArchive, $spirvArchive -Algorithm SHA256 | Format-List Path,Hash
& "$sdk\Bin\glslc.exe" --version
