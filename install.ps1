# Motif installer for Windows
# Run: irm https://raw.githubusercontent.com/Cashmeran/Motif/main/install.ps1 | iex

$ErrorActionPreference = "Stop"
$Repo = "Cashmeran/Motif"
$Version = "v0.3.0"
$Binary = "motif-windows-x86_64.exe"

Write-Host "Installing Motif $Version..." -ForegroundColor Cyan

$Url = "https://github.com/$Repo/releases/download/$Version/$Binary"
$Dest = "$env:USERPROFILE\.cargo\bin\motif.exe"

Write-Host "  Downloading $Binary..."
try {
    Invoke-WebRequest -Uri $Url -OutFile "$env:TEMP\motif.exe" -ErrorAction Stop
} catch {
    Write-Host "Pre-built binary unavailable. Building from source..." -ForegroundColor Yellow
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "Rust is not installed. Install it from https://rustup.rs first." -ForegroundColor Red
        exit 1
    }
    cargo install --git "https://github.com/$Repo.git" motif-cli
    Write-Host "✓ Motif installed from source. Run 'motif' to start." -ForegroundColor Green
    exit 0
}

Move-Item -Force "$env:TEMP\motif.exe" $Dest
Write-Host "✓ Motif $Version installed to $Dest" -ForegroundColor Green
Write-Host "  Run 'motif' to start."
