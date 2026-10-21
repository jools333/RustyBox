<#
.SYNOPSIS
Builds RustyBox for Windows (Frontend + Tauri release/debug)
#>
param(
    [switch]$Debug,
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"

Write-Host "=== Building RustyBox (Windows) ===" -ForegroundColor Cyan

# 0. Configure Toolchain & Incremental
if (Test-Path "C:\Users\lotarev\.w64devkit\bin") {
    $env:PATH = "C:\Users\lotarev\.w64devkit\bin;$env:PATH"
} elseif (Test-Path "tools\w64devkit\bin") {
    $env:PATH = "$PSScriptRoot\..\tools\w64devkit\bin;$env:PATH"
}
$env:CARGO_INCREMENTAL = "0"

# 1. Ensure sing-box and wintun are present
if (-not (Test-Path "bin\sing-box.exe") -or -not (Test-Path "bin\wintun.dll")) {
    Write-Host "sing-box or wintun driver not found in .\bin. Downloading..." -ForegroundColor Yellow
    & .\scripts\download-singbox.ps1
}

# 2. Build frontend
if (-not $SkipFrontend) {
    Write-Host "Checking frontend assets..." -ForegroundColor Cyan
    Push-Location "frontend"
    try {
        if (-not (Test-Path "node_modules")) {
            Write-Host "Installing frontend dependencies..."
            npm install
        }
        Write-Host "Building frontend assets..." -ForegroundColor Cyan
        npm run build
    } finally {
        Pop-Location
    }
}

# 3. Build Tauri
Write-Host "Building Tauri executable..." -ForegroundColor Cyan
if ($Debug) {
    cargo build --package rustybox --bin rustybox
} else {
    cargo build --release --package rustybox --bin rustybox
}

Write-Host "Build finished successfully!" -ForegroundColor Green
if ($Debug) {
    Write-Host "Artifact: target\debug\rustybox.exe" -ForegroundColor Green
} else {
    Write-Host "Artifact: target\release\rustybox.exe" -ForegroundColor Green
}
