# vision - tek komutla inference + dashboard baslat (Windows / PowerShell)
#
#   .\tools\scripts\start.ps1           # CPU
#   .\tools\scripts\start.ps1 -Gpu      # DirectML
#
# Ctrl+C ile her iki servisi de kapatir.

param(
    [switch]$Gpu,
    [string]$Model
)

$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "..\..")

# --- inference binary kontrol ---
$exePath = Join-Path $repo "target\release\inference.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "[HATA] inference.exe bulunamadi. Once kurulumu calistirin:" -ForegroundColor Red
    Write-Host "       .\tools\scripts\setup.ps1" -ForegroundColor Yellow
    exit 1
}

# --- zaten calisiyor mu? ---
$running = Get-Process inference -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "[!] inference.exe zaten calisiyor (PID $($running.Id))." -ForegroundColor Yellow
    $answer = Read-Host "    Yeniden baslatmak ister misiniz? (E/h)"
    if ($answer -match '^[Ee]?$') {
        Stop-Process -Id $running.Id -Force
        Start-Sleep -Milliseconds 500
        Write-Host "    Durduruldu." -ForegroundColor Green
    } else {
        Write-Host "    Mevcut inference kullaniliyor." -ForegroundColor DarkGray
    }
}

# --- ortam degiskenleri ---
$mediaRoot = Join-Path $repo "apps\dashboard\public\media"
New-Item -ItemType Directory -Force $mediaRoot | Out-Null

$env:INFERENCE_MEDIA_ROOT = $mediaRoot
if ($Model) { $env:INFERENCE_MODEL = $Model }

# --- inference'i arka planda baslat ---
Write-Host "`nInference servisi baslatiliyor..." -ForegroundColor Cyan
$inferenceProc = Start-Process -FilePath $exePath -PassThru -WindowStyle Hidden

# healthz'e ping atarak hazir olmasini bekle (en fazla 30 sn)
$ready = $false
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 500
    try {
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:8081/healthz" -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
        if ($resp.StatusCode -eq 200) {
            $ready = $true
            break
        }
    } catch {
        # henuz hazir degil
    }
}

if (-not $ready) {
    Write-Host "[HATA] Inference servisi 30 saniyede hazir olmadi." -ForegroundColor Red
    Stop-Process -Id $inferenceProc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

Write-Host "Inference hazir (PID $($inferenceProc.Id), port 8081)" -ForegroundColor Green

# --- dashboard baslat ---
Write-Host "Dashboard baslatiliyor..." -ForegroundColor Cyan

# tarayiciyi ac
Start-Sleep -Milliseconds 1000
Start-Process "http://localhost:3000/videos"

Write-Host "`n  Tarayici acildi -> http://localhost:3000/videos" -ForegroundColor Green
Write-Host "  Kapatmak icin Ctrl+C basin.`n" -ForegroundColor DarkGray

try {
    # Dashboard on planda calisir, Ctrl+C burada yakalanir
    pnpm --dir (Join-Path $repo "apps\dashboard") dev
} finally {
    # Dashboard kapaninca inference'i da kapat
    Write-Host "`nInference kapatiliyor..." -ForegroundColor Yellow
    Stop-Process -Id $inferenceProc.Id -Force -ErrorAction SilentlyContinue
    Write-Host "Tamam. Her iki servis de kapatildi." -ForegroundColor Green
}
