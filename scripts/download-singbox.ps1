<#
.SYNOPSIS
Downloads the official sing-box binary and wintun driver for Windows.
#>
param(
    [string]$Version = "1.14.3",
    [string]$TargetDir = "bin"
)

$ErrorActionPreference = "Stop"

Write-Host "=== RustyBox: Downloading sing-box v$Version and wintun for Windows ===" -ForegroundColor Cyan

if (-not (Test-Path $TargetDir)) {
    New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
}

# 1. Download sing-box
$singboxZip = Join-Path $TargetDir "sing-box.zip"
$singboxUrl = "https://github.com/SagerNet/sing-box/releases/download/v$Version/sing-box-$Version-windows-amd64.zip"

Write-Host "Downloading sing-box from $singboxUrl..."
Invoke-WebRequest -Uri $singboxUrl -OutFile $singboxZip -UseBasicParsing

$tempExtract = Join-Path $TargetDir "temp_sb"
Expand-Archive -Path $singboxZip -DestinationPath $tempExtract -Force
$exe = Get-ChildItem -Path $tempExtract -Filter "sing-box.exe" -Recurse | Select-Object -First 1
if ($exe) {
    Move-Item -Path $exe.FullName -Destination (Join-Path $TargetDir "sing-box.exe") -Force
}
Remove-Item -Recurse -Force $singboxZip, $tempExtract

# 2. Download wintun
$wintunZip = Join-Path $TargetDir "wintun.zip"
$wintunUrl = "https://www.wintun.net/builds/wintun-0.14.1.zip"

Write-Host "Downloading wintun driver from $wintunUrl..."
try {
    Invoke-WebRequest -Uri $wintunUrl -OutFile $wintunZip -UseBasicParsing
    $tempWintun = Join-Path $TargetDir "temp_wintun"
    Expand-Archive -Path $wintunZip -DestinationPath $tempWintun -Force
    $dll = Get-ChildItem -Path $tempWintun -Filter "wintun.dll" -Recurse | Where-Object { $_.DirectoryName -like "*amd64*" } | Select-Object -First 1
    if ($dll) {
        Move-Item -Path $dll.FullName -Destination (Join-Path $TargetDir "wintun.dll") -Force
    }
    Remove-Item -Recurse -Force $wintunZip, $tempWintun
} catch {
    Write-Warning "Could not download wintun.dll automatically: $_. Please place wintun.dll manually into $TargetDir if using Windows TUN mode."
}

Write-Host "Installed files in $TargetDir:" -ForegroundColor Green
Get-ChildItem -Path $TargetDir | Format-Table Name, Length, LastWriteTime
