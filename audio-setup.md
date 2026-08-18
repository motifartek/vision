# 🔊 Ses Analizi Sistemi — Kurulum Rehberi

Videonun ses kanalından zaman damgalı olay listesi çıkaran sistemin kurulumu.
Kurulum internet ister (model ağırlıkları, crate'ler, npm paketleri).
**Kurulduktan sonra sistem tamamen çevrimdışı çalışır.**

---

## Gereksinimler

| Araç | Sürüm | Nasıl kurulur |
|---|---|---|
| **Rust** (cargo) | stable | [rustup.rs](https://rustup.rs) |
| **Node.js** | ≥ 18 | [nodejs.org](https://nodejs.org) |
| **pnpm** | ≥ 8 | `npm install -g pnpm` |
| ffmpeg *(opsiyonel)* | herhangi | Yaygın formatlar (MP4, WAV, MP3, FLAC, MKV, OGG) symphonia ile süreç içinde çözülür. ffmpeg yalnız nadir formatlar için yedek yoldur — kurulu olması **şart değil**. |

---

## 1. Tek Komutluk Kurulum (Önerilen)

### Windows (PowerShell)

```powershell
.\tools\scripts\setup.ps1                   # CPU
.\tools\scripts\setup.ps1 -Gpu              # DirectML (her DX12 GPU)
.\tools\scripts\setup.ps1 -Model ced-small  # daha küçük model
```

### Linux / macOS / Git Bash

```bash
./tools/scripts/setup.sh                    # CPU
./tools/scripts/setup.sh --gpu cuda         # CUDA 12 + cuDNN 9
./tools/scripts/setup.sh --model ced-small  # daha küçük model
```

Script şunları yapar:
1. Gereksinimleri kontrol eder
2. Model ağırlıklarını HuggingFace'den indirir
3. Rust crate'ini derler + mel doğrulama kapısını çalıştırır
4. Dashboard npm bağımlılıklarını kurar
5. Medya klasörünü oluşturur

> **Tamamlandığında ekrana çıkan talimatları takip edin — bu noktadan sonra internet gerekmez.**

---

## 2. Manuel Kurulum (Adım Adım)

Scripti kullanmak istemiyorsanız aşağıdaki adımları izleyin.

### 2.1 Model Ağırlıklarını İndir

Model dosyaları depoda tutulmaz (~29 MB – 410 MB arası).

```powershell
# Windows
.\apps\ai\inference\scripts\fetch-models.ps1             # varsayılan: ced-base
.\apps\ai\inference\scripts\fetch-models.ps1 -Model ced-tiny  # hızlı ama daha çok yanlış pozitif
```

```bash
# Linux / macOS
sh apps/ai/inference/scripts/fetch-models.sh ced-base
```

Kullanılabilir modeller:

| Model | Boyut | Hız | İsabet |
|---|---|---|---|
| `ced-tiny` | ~29 MB | En hızlı | Yanlış pozitifler olabilir (at, kalp atışı, hapşırık, baykuş) |
| `ced-mini` | ~50 MB | Hızlı | Tiny'den iyi |
| `ced-small` | ~130 MB | Orta | Dengeli |
| `ced-base` | ~410 MB | 9 dk video < 7 sn | **Önerilen** — yanlış pozitifleri ortadan kaldırıyor |

### 2.2 Rust Derlemesi

```bash
# CPU (varsayılan)
cargo build -p inference --release

# GPU seçenekleri
cargo build -p inference --release --features directml   # Windows, her DX12 GPU
cargo build -p inference --release --features cuda        # CUDA 12 + cuDNN 9
cargo build -p inference --release --features tensorrt    # fp16 motoru kendisi kurar
```

> GPU sağlayıcısı bulunamazsa ONNX Runtime sessizce CPU'ya döner — aynı ikili her yerde çalışır.

### 2.3 Mel Doğrulama Kapısı

```bash
cargo run -p inference --release --bin verify-mel
```

Bu kapı mel ön ucunun referansla uyumlu olduğunu doğrular. Geçemezse model sessizce yanlış sonuç üretir — **bu adımı atlamayın**.

### 2.4 Dashboard Bağımlılıkları

```bash
pnpm --dir apps/dashboard install
```

---

## 3. Çalıştırma

### Tek Komutla (Önerilen)

```powershell
# Windows
.\tools\scripts\start.ps1
```

```bash
# Linux / macOS
./tools/scripts/start.sh
```

Bu komut:
1. Inference servisini arka planda başlatır
2. Hazır olmasını bekler
3. Dashboard'u başlatır
4. Tarayıcıyı `http://localhost:3000/videos` adresinde açar
5. **Ctrl+C** ile her iki servisi de kapatır

### Video Yükleme

Videoları elle klasöre kopyalamanıza gerek yok:
1. Dashboard'da **"Video yükle"** butonuna tıklayın
2. Dosyayı sürükle-bırak ile veya dosya seçici ile yükleyin
3. Yükleme tamamlandığında otomatik olarak analiz sayfasına yönlendirilirsiniz

> [!NOTE]
> **Gizlilik & Boyut Notları:**
> - **Boyut Limiti Yoktur:** Yükleme işlemi doğrudan diske akıtılarak (*streaming*) yazılır; 3 GB, 10 GB veya daha büyük videolar belleği (RAM) şişirmeden yüklenebilir.
> - **GitHub'a Gitmez:** Yüklenen tüm videolar `apps/dashboard/public/media/` dizinine kaydedilir ve `.gitignore` ile korunur. Kesinlikle Git'e veya internete gönderilmez.

Alternatif olarak videoları doğrudan `apps/dashboard/public/media/` klasörüne de kopyalayabilirsiniz.

### Manuel Çalıştırma (Alternatif)

İki terminal açın:

**Terminal 1 — Ses Analiz Servisi:**
```powershell
$env:INFERENCE_MEDIA_ROOT = "apps\dashboard\public\media"
.\target\release\inference.exe
```

**Terminal 2 — Dashboard:**
```bash
pnpm --dir apps/dashboard dev
```

Tarayıcıda: `http://localhost:3000/videos`

---

## 4. Yapılandırma

### Inference Ortam Değişkenleri

| Değişken | Varsayılan | Açıklama |
|---|---|---|
| `INFERENCE_PORT` | `8081` | Dinlenen port (adres her zaman 127.0.0.1) |
| `INFERENCE_MODELS_DIR` | `<crate>/models` | Model kök dizini |
| `INFERENCE_MODEL` | `ced-base` | Model alt dizini (`ced-tiny`, `ced-small`, …) |
| `INFERENCE_INT8` | CPU'da `true` | int8 ağırlıkları tercih et |
| `INFERENCE_THREADS` | çekirdek sayısı | ONNX Runtime iş parçacığı |
| `INFERENCE_BATCH` | CPU `32`, GPU `256` | Tek çağrıdaki pencere sayısı |
| `INFERENCE_MEDIA_ROOT` | *(yok)* | **Üretimde mutlaka ayarlayın** — ayarlıysa istenen yollar bu kökün dışına çıkamaz |

### Gateway Ortam Değişkenleri (Opsiyonel)

| Değişken | Varsayılan | Açıklama |
|---|---|---|
| `GATEWAY_KRATOS_URL` | `http://127.0.0.1:4433` | Kratos oturum doğrulama adresi |
| `GATEWAY_KETO_URL` | `http://127.0.0.1:4466` | Keto yetkilendirme adresi |
| `GATEWAY_INFERENCE_URL` | `http://127.0.0.1:8081` | Inference servisi adresi |

### Analiz Profilleri

| Profil | Pencere | Adım | Ne zaman kullanılır |
|---|---|---|---|
| `hassas` | 1 sn | 0.25 sn | Kısa, keskin sesleri yakalamak (cam kırılması, çığlık) |
| `dengeli` | 2 sn | 0.5 sn | **Varsayılan** — çoğu senaryo için yeterli |
| `isabetli` | 10 sn | 5 sn | Uzun, sürekli sesler (makine uğultusu, müzik) |

---

## 5. API Uç Noktaları

### Inference Servisi (127.0.0.1:8081)

```bash
# Sağlık kontrolü
curl http://127.0.0.1:8081/healthz

# 527 sınıf listesi (İngilizce + Türkçe)
curl http://127.0.0.1:8081/v1/labels

# Ses çözümleme
curl -X POST http://127.0.0.1:8081/v1/audio/analyze \
  -H "Content-Type: application/json" \
  -d '{"path":"test3.mp4","profile":"dengeli","threshold":0.35}'
```

İstek alanları: `path` (zorunlu), `profile`, `threshold`, `top_k`, `min_duration_sec`, `gap_sec`, `max_events`, `include_frames`, `batch_size`.

### Gateway (0.0.0.0:8000) — Kimlik doğrulamalı

```bash
# Video ses olayları (Kratos oturumu + Keto yetkisi gerekir)
curl http://localhost:8000/api/videos/test3/audio-events
```

---

## 6. Doğrulama ve Test

```bash
# Mel hattı referansla uyumlu mu?
cargo run -p inference --release --bin verify-mel

# Symphonia vs ffmpeg çözücü karşılaştırması
cargo run -p inference --release --bin compare-decoders -- apps/dashboard/public/media/test3.mp4

# Birim testler (resampler kalite testleri dahil)
cargo test -p inference
```

---

## 7. Sık Karşılaşılan Sorunlar

### "model dosyası bulunamadı"

Model ağırlıkları indirilmemiş. Çözüm:
```bash
sh apps/ai/inference/scripts/fetch-models.sh ced-base
```

### Dashboard'da "Örnek veri" rozeti görünüyor

Inference servisi çalışmıyor veya erişilemiyor. Kontrol:
1. Servis açık mı? → `curl http://127.0.0.1:8081/healthz`
2. Medya dosyası doğru yerde mi? → `apps/dashboard/public/media/` altında `<video-adı>.mp4` olmalı
3. `INFERENCE_MEDIA_ROOT` doğru mu?

### Keto "unable to open database file"

`compose.yaml`'daki `keto-volume-init` servisi çalışmamış. Volume'u sıfırlayın:
```bash
docker compose -f apps/identity/compose.yaml down -v
docker compose -f apps/identity/compose.yaml up -d
```

### GPU varken CPU kullanılıyor

Doğru feature flag ile derleyin:
```bash
cargo build -p inference --release --features directml   # Windows
cargo build -p inference --release --features cuda        # Linux + CUDA 12
```
`/healthz` uç noktasında `providers` alanını kontrol edin.

### verify-mel kapısı geçilemiyor

Mel parametreleri bozulmuş. `apps/ai/inference/src/audio/mel.rs` dosyasındaki sabitlerin şu değerlerde olduğunu kontrol edin:
- `SAMPLE_RATE = 16000`, `N_FFT = 512`, `HOP_LENGTH = 160`, `N_MELS = 64`
- `F_MIN = 0.0`, `F_MAX = 8000.0`, `TOP_DB = 120.0`

### Türkçe karakterler bozuk görünüyor

Dashboard'un `geist` paketini kullandığından emin olun (`next/font/google` değil). `apps/dashboard/package.json`'da `"geist"` bağımlılığı olmalı.

---

## Mimari Özet

```
Tarayıcı (localhost:3000)
    │
    ├── /videos/<id>  →  Dashboard (Next.js)
    │                        │
    │                        ├── useAudioAnalysis()  →  POST /v1/audio/analyze
    │                        │                              │
    │                        │                         Inference Servisi (Rust/Axum)
    │                        │                         127.0.0.1:8081
    │                        │                              │
    │                        │                         CED Model (ONNX Runtime)
    │                        │                         527 sınıf × 3 profil
    │                        │
    │                        └── Video oynatma  →  /media/<id>.mp4 (statik dosya)
    │
    └── /api/videos/<id>/*  →  Gateway (Rust/Axum, 0.0.0.0:8000)
                                    │
                                    ├── Kratos  →  kim bu?
                                    ├── Keto    →  yetkisi var mı?
                                    └── Inference proxy
```
