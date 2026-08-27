# Sonic model host kurulumu (Windows)
#
# Cikarimi konteynerden cikarip host'taki karta tasiyan `model-host` surecini
# kurar. Sonic servisi Docker'da kalir; disari cikan yalnizca tensor->tensor
# ONNX cagrisi.
#
# NEDEN: DirectML bir Windows DirectX 12 API'si, Linux konteynerinde
# calismiyor. Docker'da CUDA denendi ve CPU'dan yavas cikti. Olculen sonuc
# (11 dk 58 sn'lik kayit, ayni video):
#
#   Docker CPU   (int8)  13,2 sn   54x
#   Docker CUDA  (fp32)  22,8 sn   32x
#   Host DirectML(fp32)   6,5 sn  111x     <- bu kurulum
#
# Kullanim:
#   .\tools\scripts\setup-model-host.ps1
#   .\tools\scripts\setup-model-host.ps1 -Model ced-tiny
#   .\tools\scripts\setup-model-host.ps1 -DmlDevice 0   # adaptoru elle sec

param(
    [string]$Model = "ced-base",
    [int]$DmlDevice = -1,
    [switch]$SkipVerify
)

$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "..\..")

function Adim($n, $metin) { Write-Host "`n[$n] $metin" -ForegroundColor Yellow }
function Tamam($metin) { Write-Host "  [tamam] $metin" -ForegroundColor Green }
function Bilgi($metin) { Write-Host "  $metin" -ForegroundColor DarkGray }

Write-Host "sonic model host kurulumu - $repo" -ForegroundColor Cyan

# --- [1/5] Platform ve on kosullar -------------------------------------------
Adim "1/5" "On kosullar"

# PowerShell Core Mac/Linux'ta da kosuyor; oralarda DirectML yok.
if ($IsLinux -or $IsMacOS) {
    Write-Host @"
  Bu kurulum yalniz Windows icindir: DirectML bir DirectX 12 API'si.
  Bu makinede yapacak bir sey yok - Docker yolunda kalin, her sey calisiyor:

      docker compose -f platform/docker/compose.yaml up -d

"@ -ForegroundColor Yellow
    exit 0
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "'cargo' bulunamadi. Rust kurun: https://rustup.rs"
}
Tamam "cargo"

# ONNX Runtime DirectML'e baglanirken dxcore.lib / DirectML.lib ariyor. Bunlar
# guncel Windows SDK ile geliyor; VS 2017 Build Tools yetmiyor ve hata
# LNK1181 olarak cikiyor - mesaj DirectML'den hic bahsetmedigi icin yanlis
# teshise yol aciyor. O yuzden burada, derlemeden once bakiyoruz.
$sdkKok = "C:\Program Files (x86)\Windows Kits\10\Lib"
$dxcore = if (Test-Path $sdkKok) {
    Get-ChildItem -Path $sdkKok -Recurse -Filter "dxcore.lib" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "\\x64\\" } | Select-Object -First 1
} else { $null }

if (-not $dxcore) {
    throw @"
Windows SDK icinde dxcore.lib bulunamadi.
DirectML ile baglama bu dosyaya ihtiyac duyuyor; olmadan derleme LNK1181 ile duser.
Cozum: Visual Studio Installer -> 'Windows 11 SDK' (10.0.22000 veya ustu) kurun.
"@
}
Tamam "Windows SDK ($($dxcore.Directory.Parent.Parent.Name))"

# --- [2/5] Ekran karti secimi ------------------------------------------------
Adim "2/5" "Ekran karti"

$kartlar = @(Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name)
for ($i = 0; $i -lt $kartlar.Count; $i++) { Bilgi "adaptor $i : $($kartlar[$i])" }

if ($DmlDevice -ge 0) {
    $secilen = $DmlDevice
    Bilgi "adaptor elle verildi: $secilen"
} else {
    # DirectML varsayilan olarak 0 numarali adaptoru seciyor ve cift GPU'lu
    # laptoplarda bu genelde tumlesik karttir. Ayrik kart bosta beklerken
    # tumlesikte kosmak CPU'dan bile yavas olabilir - olculdu.
    $ayrik = 0
    for ($i = 0; $i -lt $kartlar.Count; $i++) {
        if ($kartlar[$i] -match "NVIDIA|Radeon|Arc\b") { $ayrik = $i; break }
    }
    $secilen = $ayrik
}
Tamam "SONIC_DML_DEVICE=$secilen -> $($kartlar[$secilen])"

# --- [3/5] Model agirliklari -------------------------------------------------
Adim "3/5" "Model agirliklari ($Model)"
# $LASTEXITCODE burada kullanilmaz: o yalnizca yerel calistirabilirler icin
# ayarlaniyor, PowerShell scripti cagirmak onu degistirmiyor ve onceki bir
# komuttan kalan deger yanlis alarm veriyordu. Cagrilan script de
# $ErrorActionPreference = "Stop" kullandigi icin gercek hata zaten firlatilir.
& (Join-Path $repo "apps\ai\sonic\scripts\fetch-models.ps1") -Model $Model

# --- [4/5] Derleme -----------------------------------------------------------
Adim "4/5" "model-host derlemesi (DirectML)"

$calisan = Get-Process model-host -ErrorAction SilentlyContinue
if ($calisan) {
    Bilgi "model-host calisiyor (PID $($calisan.Id)); Cargo uzerine yazamaz, durduruluyor"
    Stop-Process -Id $calisan.Id -Force
    Start-Sleep -Milliseconds 500
}

Push-Location $repo
try {
    cargo build -p sonic --release --features directml --bin model-host
    if ($LASTEXITCODE -ne 0) { throw "cargo build basarisiz" }

    if (-not $SkipVerify) {
        # Mel on ucu referansla uyusmazsa model sessizce sacmalar; hat
        # uzerindeki en kritik test bu.
        Adim "5/5" "Dogrulama kapisi (mel hatti)"
        cargo run -p sonic --release --features directml --bin verify-mel
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  [!] Dogrulama kapisi gecilemedi." -ForegroundColor Yellow
            Write-Host "      Ciktida 'Uygulama Denetimi ilkesi' geciyorsa sorun mel hattinda degil;" -ForegroundColor DarkGray
            Write-Host "      Windows Smart App Control imzasiz ikiliyi engelliyor demektir." -ForegroundColor DarkGray
            throw "dogrulama kapisi gecilemedi"
        }
    } else {
        Adim "5/5" "Dogrulama atlandi (-SkipVerify)"
    }
} finally {
    Pop-Location
}

# --- Bitis -------------------------------------------------------------------
$exe = Join-Path $repo "target\release\model-host.exe"

Write-Host @"

=======================================================
  Kurulum tamamlandi
=======================================================

1) Model sunucusunu baslatin (bu pencere acik kalmali):

     `$env:SONIC_DML_DEVICE = "$secilen"
     $exe

   Konteynerden host.docker.internal uzerinden gelindigi icin varsayilan
   dinleme adresi 0.0.0.0:8082. Kimlik dogrulamasi YOK - guvenlik duvarinda
   8082'yi yerel aginiza acmayin. Windows ilk calistirmada sorarsa yalnizca
   ozel aglara izin verin.

2) Yigini bu katmanla kaldirin:

     docker compose -f platform/docker/compose.yaml ``
                    -f apps/ai/sonic/compose.modelhost.yaml up -d

3) Calistigini dogrulayin - /healthz'deki `providers` alanina GUVENMEYIN,
   o alan istenen saglayici zincirini gosterir, etkin olani degil:

     curl http://127.0.0.1:8082/healthz
     docker logs docker-sonic-1 | Select-String "model sunucusuna"

   Bir analiz kosarken Gorev Yoneticisi > Performans'ta $($kartlar[$secilen])
   uzerinde hareket gorunmeli. Gorunmuyorsa yanlis adaptor secilmistir:
   -DmlDevice ile baska bir numara deneyin.

Geri donmek icin: -f apps/ai/sonic/compose.modelhost.yaml katmanini kaldirin.
Sonic konteyner ici CPU cikarimina doner, baska hicbir sey gerekmez.
"@ -ForegroundColor Green
