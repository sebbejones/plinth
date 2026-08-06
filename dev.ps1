# Launch Plinth in dev mode. Requires the normal system toolchain on PATH:
# the .NET SDK (Fable), Rust (Tauri backend), and Node.

Set-Location $PSScriptRoot

$missing = @('dotnet', 'cargo', 'npm') | Where-Object { -not (Get-Command $_ -ErrorAction SilentlyContinue) }

if ($missing) {
    Write-Host ""
    Write-Host "Missing from PATH: $($missing -join ', ')"
    Write-Host ""
    Write-Host "Install as normal system tools, then open a NEW terminal:"
    Write-Host "  .NET SDK 8   https://dotnet.microsoft.com/download/dotnet/8.0"
    Write-Host "  Rust         https://rustup.rs"
    Write-Host "  Node LTS     https://nodejs.org"
    Write-Host ""
    exit 1
}

npm run tauri dev
