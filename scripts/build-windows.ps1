$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
$sourceExe = Join-Path $repo "target\release\remotehand-target-monitor.exe"
$stageDir = Join-Path $repo "bin\windows-x86_64"
$stageExe = Join-Path $stageDir "remotehand-target-monitor.exe"
$packageDir = Join-Path $repo "dist\remotehand-target-monitor-windows-x86_64"
$packageBinDir = Join-Path $packageDir "bin\windows-x86_64"
$packageUiDir = Join-Path $packageDir "ui"

Write-Host "Building RemoteHand Target Monitor (release)..."
Push-Location $repo
try {
    cargo build --release
} finally {
    Pop-Location
}

if (-not (Test-Path $sourceExe)) {
    throw "Build completed but executable was not found: $sourceExe"
}

# Keep the repository-local runtime staging path useful for development.
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
Copy-Item -Force $sourceExe $stageExe

# Build a clean installable package. Do not package the repository root:
# it contains .git, Rust sources, target/, documentation, and development files.
if (Test-Path $packageDir) {
    Remove-Item -Recurse -Force $packageDir
}
New-Item -ItemType Directory -Force -Path $packageBinDir | Out-Null
New-Item -ItemType Directory -Force -Path $packageUiDir | Out-Null

Copy-Item -Force (Join-Path $repo "plugin.json") (Join-Path $packageDir "plugin.json")
Copy-Item -Force (Join-Path $repo "ui\index.html") (Join-Path $packageUiDir "index.html")
Copy-Item -Force $sourceExe (Join-Path $packageBinDir "remotehand-target-monitor.exe")

$packageBytes = (Get-ChildItem -File -Recurse $packageDir | Measure-Object -Property Length -Sum).Sum
$packageMiB = [Math]::Round($packageBytes / 1MB, 2)

Write-Host ""
Write-Host "Plugin runtime staged successfully:"
Write-Host "  $stageExe"
Write-Host ""
Write-Host "Clean installable plugin package created:"
Write-Host "  $packageDir"
Write-Host "  Size: $packageMiB MiB"
Write-Host ""
Write-Host "Install THIS folder from RemoteHand (not the repository root):"
Write-Host "  $packageDir"


$manifest = Get-Content -Raw (Join-Path $repo "plugin.json") | ConvertFrom-Json
$packageVersion = [string]$manifest.version
if ([string]::IsNullOrWhiteSpace($packageVersion)) {
    throw "plugin.json does not contain a valid version."
}
$zipPath = Join-Path $repo ("dist\remotehand-target-monitor-v{0}-windows-x86_64.zip" -f $packageVersion)
if (Test-Path $zipPath) {
    Remove-Item -Force $zipPath
}
Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $zipPath -CompressionLevel Optimal
$zipHash = (Get-FileHash -Algorithm SHA256 $zipPath).Hash
$checksumPath = Join-Path $repo "dist\SHA256SUMS.txt"
"$zipHash  $(Split-Path -Leaf $zipPath)" | Set-Content -Encoding ascii $checksumPath

Write-Host ""
Write-Host "Release ZIP created:"
Write-Host "  $zipPath"
Write-Host "  SHA256: $zipHash"
Write-Host ""
Write-Host "Checksum file:"
Write-Host "  $checksumPath"
