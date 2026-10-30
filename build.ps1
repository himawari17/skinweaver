# Release build for Windows x64. Run from this folder in PowerShell:
#   powershell -ExecutionPolicy Bypass -File .\build.ps1
# Needs Rust stable 1.95+ and Visual Studio Build Tools (Desktop C++ + Windows SDK).
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

cargo test --locked --release
if ($LASTEXITCODE -ne 0) { throw "Tests failed ($LASTEXITCODE). Release package was not updated." }
cargo build --locked --release --bin skinweaver
if ($LASTEXITCODE -ne 0) { throw "Build failed ($LASTEXITCODE). Release package was not updated." }

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\skinweaver.exe dist\ -Force
Copy-Item README.md, LICENSE, ASSETS.md, LOCALIZATION.md, THIRD_PARTY_LICENSES.txt dist\ -Force
New-Item -ItemType Directory -Force dist\assets\fonts, dist\assets\icons, dist\docs\screenshots | Out-Null
Copy-Item assets\fonts\OFL*.txt dist\assets\fonts\ -Force
Copy-Item assets\icons\LICENSE.txt dist\assets\icons\ -Force
Copy-Item docs\screenshots dist\docs\ -Recurse -Force
Compress-Archive -Force -Path dist\skinweaver.exe, dist\README.md, dist\LICENSE, dist\ASSETS.md, `
    dist\LOCALIZATION.md, dist\THIRD_PARTY_LICENSES.txt, dist\assets, dist\docs `
    -DestinationPath dist\skinweaver-windows-x64.zip
Write-Host "Ready: dist\skinweaver.exe"
