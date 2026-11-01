$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'dist\haas\hex.exe'
if (!(Test-Path $exe)) { throw 'Hex Haas is not built yet. Run scripts/build-haas.ps1 first.' }
Start-Process -FilePath $exe -ArgumentList 'app' -WorkingDirectory (Split-Path $exe)
