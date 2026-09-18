# Package the release exe into dist/quire-windows-x64.zip (M8-lite).
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item .\target\release\quire.exe .\dist\quire.exe -Force
Compress-Archive -Path .\dist\quire.exe -DestinationPath .\dist\quire-windows-x64.zip -Force
Remove-Item .\dist\quire.exe
Write-Output "dist/quire-windows-x64.zip ready"
