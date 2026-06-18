# Motif installer for Windows
# Run: irm https://raw.githubusercontent.com/Cashmeran/Motif/main/install.ps1 | iex

$ErrorActionPreference = "Stop"
Write-Host "Installing Motif..." -ForegroundColor Cyan

if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust is not installed. Install it from https://rustup.rs first." -ForegroundColor Red
    exit 1
}

cargo install --git https://github.com/Cashmeran/Motif.git motif-cli

Write-Host "✓ Motif installed. Run 'motif' to start." -ForegroundColor Green
