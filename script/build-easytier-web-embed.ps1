param(
    [switch]$SkipInstall
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptDir '..')).Path

function Test-Command {
    param([Parameter(Mandatory = $true)][string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

Write-Host "Repository root: $repoRoot" -ForegroundColor Cyan
Set-Location $repoRoot

if (-not (Test-Command node)) {
    throw "Node.js was not found. Please install it first, for example: winget install OpenJS.NodeJS.LTS"
}

if (-not (Test-Command cargo)) {
    throw "Cargo/Rust was not found. Please install Rust first, for example: winget install Rustlang.Rustup"
}

if (-not (Test-Command pnpm)) {
    Write-Host "pnpm not found. Trying to activate via corepack..." -ForegroundColor Yellow
    if (Test-Command corepack) {
        & corepack enable
        & corepack prepare pnpm@latest --activate
    }
}

if (-not (Test-Command pnpm)) {
    throw "pnpm is still unavailable. Please install it with: npm install -g pnpm"
}

if (-not $SkipInstall) {
    Write-Host "Installing workspace dependencies..." -ForegroundColor Cyan
    pnpm install
}

Write-Host "Building web frontend assets..." -ForegroundColor Cyan
pnpm --dir easytier-web/frontend build

Write-Host "Building embed-enabled easytier-web binary..." -ForegroundColor Cyan
cargo build --release --package=easytier-web --features=embed

$suffix = if ($IsWindows) { '.exe' } else { '' }
$src = Join-Path $repoRoot "target/release/easytier-web$suffix"
$dst = Join-Path $repoRoot "target/release/easytier-web-embed$suffix"

if (-not (Test-Path $src)) {
    throw "Expected build output was not found: $src"
}

Copy-Item $src $dst -Force
Write-Host "Created: $dst" -ForegroundColor Green
Write-Host "You can run it with:" -ForegroundColor Green
Write-Host "  .\target\release\easytier-web-embed$suffix --help" -ForegroundColor Green
