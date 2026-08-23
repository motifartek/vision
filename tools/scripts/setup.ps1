# vision - tek komutluk kurulum (Windows / PowerShell)
#
#   .\tools\scripts\setup.ps1                  # DirectML (varsayilan, her DX12 GPU)
#   .\tools\scripts\setup.ps1 -Cpu             # GPU istemiyorsaniz
#   .\tools\scripts\setup.ps1 -Model ced-small # daha isabetli model
#
# Varsayilan neden GPU: olculdu, 1 saat 40 dakikalik kayit CPU'da 298 sn,
# DirectML ile 44 sn suruyor. Bayragi unutan biri yedi kat yavas bir kurulum
# elde ediyordu. DirectML DX12 destekleyen her kartta calisir; bulamazsa ONNX
# Runtime sessizce CPU'ya doner, yani ayni ikili GPU'suz makinede de calisir.
#
# Kurulum internet ister (model agirliklari, crate'ler, npm paketleri).
# Kurulduktan sonra sistem tamamen cevrimdisi calisir.

param(
    [string]$Model = "ced-base",
    [switch]$Cpu,
    # Geriye donuk uyumluluk: eski belgelerde ve kaslarda -Gpu var, artik varsayilan.
    [switch]$Gpu,
    [switch]$SkipDashboard
)

$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Write-Host "vision kurulumu - $repo" -ForegroundColor Cyan

function Require($name, $hint) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "'$name' bulunamadi. $hint"
    }
    Write-Host "  [tamam] $name" -ForegroundColor Green
}

Write-Host "`n[1/4] Gereksinimler" -ForegroundColor Yellow
Require cargo "Rust kurun: https://rustup.rs"
if (-not $SkipDashboard) {
    Require node "Node.js kurun: https://nodejs.org"
    Require pnpm "Kurulum: npm install -g pnpm"
}
if (Get-Command ffmpeg -ErrorAction SilentlyContinue) {
    Write-Host "  [tamam] ffmpeg (yedek cozucu)" -ForegroundColor Green
} else {
    # Zorunlu degil: ses cozme surec icinde symphonia ile yapiliyor.
    Write-Host "  [not ] ffmpeg yok - yaygin formatlar yine calisir" -ForegroundColor DarkGray
}

Write-Host "`n[2/4] Model agirliklari ($Model)" -ForegroundColor Yellow
& (Join-Path $PSScriptRoot "..\..\apps\ai\inference\scripts\fetch-models.ps1") -Model $Model

Write-Host "`n[3/4] Rust derlemesi" -ForegroundColor Yellow

# inference.exe zaten calisiyorsa Cargo uzerine yazamaz (os error 5).
$running = Get-Process inference -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "  [!] inference.exe zaten calisiyor (PID $($running.Id))." -ForegroundColor Yellow
    $answer = Read-Host "  Durdurup devam etmek ister misiniz? (E/h)"
    if ($answer -match '^[Ee]?$') {
        Stop-Process -Id $running.Id -Force
        Start-Sleep -Milliseconds 500
        Write-Host "  inference durduruldu." -ForegroundColor Green
    } else {
        Write-Host "  Derleme atlandi - once inference'i durdurun, sonra tekrar calistirin." -ForegroundColor Red
        exit 1
    }
}

Push-Location $repo
try {
    # Ayni feature seti hem derlemede hem dogrulamada kullanilir. Eskiden
    # dogrulama adimi feature'siz kosuyordu: kurulan GPU derlemesini degil CPU
    # yolunu sinar, ustelik crate'i her seferinde bastan derlerdi.
    if ($Cpu) {
        Write-Host "  CPU icin derleniyor..." -ForegroundColor DarkGray
        $features = @()
    } else {
        Write-Host "  DirectML ile derleniyor (GPU)..." -ForegroundColor DarkGray
        $features = @("--features", "directml")
    }

    cargo build -p inference --release $features
    if ($LASTEXITCODE -ne 0) { throw "cargo build basarisiz" }

    Write-Host "`n  Dogrulama kapisi (mel hatti referansla uyumlu mu)" -ForegroundColor DarkGray
    cargo run -p inference --release $features --bin verify-mel
    if ($LASTEXITCODE -ne 0) {
        # "Uygulama Denetimi ilkesi bu dosyayi engelledi (os error 4551)" mel
        # hattiyla ilgili degildir: Windows Smart App Control yeni derlenmis
        # imzasiz ikilileri engelliyor. Yanlis teshis koymayalim.
        Write-Host "  [!] Dogrulama kapisi calistirilamadi ya da gecilemedi." -ForegroundColor Yellow
        Write-Host "      Ciktida 'Uygulama Denetimi ilkesi' geciyorsa sorun mel hattinda degil;" -ForegroundColor DarkGray
        Write-Host "      Windows Smart App Control imzasiz ikiliyi engelliyor demektir." -ForegroundColor DarkGray
        throw "dogrulama kapisi gecilemedi"
    }
} finally {
    Pop-Location
}

if (-not $SkipDashboard) {
    Write-Host "`n[4/4] Dashboard bagimliliklari" -ForegroundColor Yellow
    pnpm --dir (Join-Path $repo "apps\dashboard") install
    if ($LASTEXITCODE -ne 0) { throw "pnpm install basarisiz" }
} else {
    Write-Host "`n[4/4] Dashboard atlandi" -ForegroundColor DarkGray
}

$media = Join-Path $repo "apps\dashboard\public\media"
New-Item -ItemType Directory -Force $media | Out-Null

$msg = @"

=======================================================
  Kurulum basariyla tamamlandi!
=======================================================

Sistemi baslatmak icin tek komut calistirin:

  .\tools\scripts\start.ps1

Bu komut:
  - Ses analiz servisini arka planda baslatir
  - Dashboard'u ayaga kaldirir
  - Tarayicinizi otomatik acar (http://localhost:3000/videos)

Videolarinizi web arayuzundeki 'Video yukle' butonuyla
surukleyip birakarak dogrudan yukleyebilirsiniz.
(Dosya boyutu siniri yoktur, internet gerekmez.)
"@

Write-Host $msg -ForegroundColor Green
