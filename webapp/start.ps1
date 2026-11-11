# SpendCraft Webapp Companion Startup Script
# Fleet Port: 11262 (Vite Frontend), 11263 (Backend)
param(
    [switch]$Install
)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if ($Install -or -not (Test-Path "node_modules")) {
    Write-Host "Installing webapp dependencies via npm..." -ForegroundColor Cyan
    npm install
}

Write-Host "Starting SpendCraft Webapp on http://localhost:11262 (Proxying API to 11263)..." -ForegroundColor Green
npm run dev
