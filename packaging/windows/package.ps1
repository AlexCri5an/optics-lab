# PowerShell script to package OpticsLab for Windows (Portable ZIP & MSI installer)
param(
  [ValidateSet('x64', 'arm64')] [string] $Arch = 'x64',
  [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Version = (Select-String -Path (Join-Path $Root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$Dist = Join-Path $Root 'dist'
$TargetDir = Join-Path $Root 'target'
$Stage = Join-Path $TargetDir 'windows-package'

Write-Host "==> Packaging OpticsLab v$Version for Windows ($Arch)..."
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

if (-not $SkipBuild) {
  cargo build --release -p optics-desktop -p optics-cli
}

Copy-Item (Join-Path $TargetDir 'release\OpticsLab.exe') (Join-Path $Stage 'optics-desktop.exe')
Copy-Item (Join-Path $TargetDir 'release\OpticsLab.exe') (Join-Path $Stage 'OpticsLab.exe')
Copy-Item (Join-Path $TargetDir 'release\optics-cli.exe') $Stage

$ZipName = "OpticsLab-$Version-windows-$Arch-portable.zip"
$ZipPath = Join-Path $Dist $ZipName

Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $ZipPath -Force
Write-Host "==> Created portable zip: $ZipPath"
