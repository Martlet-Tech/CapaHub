param(
    [Parameter(Mandatory=$true)]
    [string]$PluginName
)

$CapaHubRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$PluginDir = Join-Path $CapaHubRoot "plugins" $PluginName
$TargetDir = Join-Path $CapaHubRoot "target\release"
$OutputDir = "$env:LOCALAPPDATA\CapaHub\plugins\$PluginName"

Write-Host "Building plugin: $PluginName" -ForegroundColor Cyan

Set-Location $CapaHubRoot
cargo build --release -p $PluginName

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
Copy-Item (Join-Path $TargetDir "$PluginName.dll") (Join-Path $OutputDir "$PluginName.dll") -Force
Copy-Item (Join-Path $PluginDir "plugin.toml") (Join-Path $OutputDir "plugin.toml") -Force

Write-Host "Plugin deployed to: $OutputDir" -ForegroundColor Green
