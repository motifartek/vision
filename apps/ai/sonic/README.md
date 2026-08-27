# inference — ses olay tespiti servisi

Videonun ses kanalından zaman damgalı olay listesi çıkarır. Model **CED**
(AudioSet, 527 sınıf), çalışma zamanı **ONNX Runtime**, ön uç Rust'ta yazılmış
log-mel.

## Kurulum

Model ağırlıkları depoda tutulmaz, script ile indirilir:

```bash
sh apps/ai/sonic/scripts/fetch-models.sh ced-base
```

Windows PowerShell için: `apps\ai\sonic\scripts\fetch-models.ps1`.

**Docker'da elle indirmek gerekmez.** `sonic-models-init` servisi aynı scripti
konteyner içinde koşturup ağırlıkları `sonic-models` birimine yazar, `sonic`
ancak o bittikten sonra başlar. Script var olan dosyaları atladığı için indirme
yalnız ilk `up`'ta olur; sonraki açılışlar ağa hiç çıkmaz. Hedef dizin
`SONIC_MODELS_DIR`'den gelir, yani script ana makinede ve konteynerde aynıdır.

Başka bir model istenirse `SONIC_MODEL` hem indirmeyi hem servisi birlikte
yönlendirir:

```bash
SONIC_MODEL=ced-tiny docker compose -f platform/docker/compose.yaml up -d sonic
```

`ced-tiny`, `ced-mini`, `ced-small`, `ced-base` indirilebilir; hepsi aynı
öznitelik hattını kullanır, yalnız ağırlık dosyası değişir.

**Varsayılan `ced-base`.** Aynı videoda Tiny'nin ürettiği yanlış pozitifleri
(at, kalp atışı, hapşırık, baykuş) hiç üretmiyor, gerçek sesleri koruyor.
Bedeli: indirme 410 MB (Tiny 29 MB) ve çıkarım ~4,5× yavaş — ama 9 dakikalık
video yine 7 saniyenin altında bitiyor. Ölçüm ve karşılaştırma:
aşağıdaki "Ölçümler" bölümü.

Hız öncelikliyse `SONIC_MODEL=ced-tiny` yeterli; iki modeli yan yana
tutup duruma göre geçebilirsiniz.

Ses çözme süreç içinde `symphonia` ile yapılır (WAV, FLAC, MP3, AAC/MP4, MKV,
OGG…). `ffmpeg` yalnız symphonia'nın çözemediği formatlar için **yedek** yoldur;
kurulu olması şart değildir ama olması kapsamı genişletir. Hangi yolun
kullanıldığı yanıttaki `media.decoder` alanında görünür.

## Çalıştırma

```bash
SONIC_MEDIA_ROOT=/path/to/media cargo run -p inference --release
```

Servis **varsayılan olarak yalnız `127.0.0.1:8081`** dinler: kendi kimlik
doğrulaması yoktur. Adres `SONIC_BIND` ile değiştirilebilir ama bunun tek
meşru kullanımı konteynerdir — orada loopback'e bağlanmak servisi tümüyle kör
eder, çünkü hem yayımlanan port hem `http://sonic:8081` konteynerin dışından
gelir. Compose `SONIC_BIND=0.0.0.0:8081` verir ve kapıyı bunun yerine
yayımlanan portu loopback'e sabitleyerek (`127.0.0.1:8081:8081`) kapatır.
Tasarımda dışarıya açılan kapı gateway'dir, ama
**bugün gateway akışta değil** — dashboard doğrudan buraya bağlanıyor
(bkz. `audio-setup.md`). Bu yüzden tarayıcı kökeni de yerel arayüzle sınırlı:
yalnız `localhost` / `127.0.0.1` / `[::1]` kökenli sayfalar istek atabilir.

### Ortam değişkenleri

| Değişken | Varsayılan | Açıklama |
|---|---|---|
| `SONIC_BIND` | `127.0.0.1:<SONIC_PORT>` | Dinleme adresi; yalnız konteynerde `0.0.0.0:8081` yapılmalı |
| `SONIC_PORT` | `8081` | Dinlenen port (`SONIC_BIND` verilmediğinde) |
| `SONIC_MODELS_DIR` | `<crate>/models` | Model kök dizini |
| `SONIC_MODEL` | `ced-base` | Alt dizin adı (`ced-tiny`, `ced-small`, …) |
| `SONIC_INT8` | CPU'da `true` | int8 ağırlıkları tercih et |
| `SONIC_THREADS` | çekirdek sayısı | ONNX Runtime iş parçacığı |
| `SONIC_BATCH` | CPU `32`, GPU `64` | Tek çağrıdaki pencere sayısı |
| `SONIC_MODEL_HOST` | *(yok)* | Ayarlanırsa çıkarım bu adresteki `model-host` sürecine taşınır; bkz. "Model host modu" |
| `SONIC_DML_DEVICE` | `0` | DirectML adaptör numarası; çift GPU'lu makinede ayrık kart genelde `1` |
| `SONIC_MEDIA_ROOT` | *(yok)* | Ayarlanırsa istenen yollar bu kökün dışına çıkamaz |
| `SONIC_MAX_UPLOAD_BYTES` | `0` (sınırsız) | Yükleme tavanı; dosya diske akıtıldığı için varsayılan sınırsız |

`SONIC_MEDIA_ROOT` ayarlı değilse servis başlarken uyarı basar ve yerel
dosya sistemindeki herhangi bir yolu okuyabilir — üretimde mutlaka ayarlayın.

## GPU

Aynı video (11 dk 58 sn, 1434 pencere), aynı makine (RTX 4050 Laptop 6 GB,
20 iş parçacıklı CPU) — üç kurulum ölçüldü:

| Kurulum | Ağırlık | Çıkarım | Toplam | Gerçek zaman |
|---|---|---|---|---|
| Docker, CPU | int8 | 12.332 ms | 13.231 ms | 54× |
| Docker, CUDA | fp32 | 21.466 ms | 22.758 ms | 32× |
| **Host, DirectML** | fp32 | **5.740 ms** | **6.453 ms** | **111×** |

İki sonuç sezgiye aykırı, ikisi de ölçüldü:

**Docker'da CUDA, CPU'dan yavaş.** int8 nicemleme CPU'ya özgü; GPU fp32'ye
çıkmak zorunda ve bu kartın fp32'si 20 iş parçacıklı CPU'nun int8'ini
geçemiyor. Batch büyütmek kötüleştiriyor (64→192: 17,9→19,4 sn), yani darboğaz
çekirdek başlatma maliyeti bile değil. `compose.gpu.yaml` duruyor ama
hızlandırmak için değil, bol VRAM'li bir kartta yeniden ölçmek isteyenler için.

**DirectML Linux konteynerinde imkânsız.** Windows DirectX 12 API'si; ONNX
Runtime'ın DirectML sağlayıcısı yalnız Windows için dağıtılıyor. Hızın tek
gerçek kaynağı bu, ama konteynerin içinden erişilemiyor.

### Model host modu

Çözüm, servisi taşımak değil **yalnız ONNX çağrısını** taşımak: sonic Docker'da
kalıyor, tensör→tensör çağrısı host'taki `model-host` sürecine gidiyor. Ölçüme
göre toplam sürenin %94'ü zaten o tek çağrıda geçiyor; çözme, log-mel, olay
bölütleme ve güvenlik kuralları konteynerde kalabiliyor.

```powershell
.\tools\scripts\setup-model-host.ps1     # ön koşullar, adaptör, ağırlık, derleme, mel kapısı
$env:SONIC_DML_DEVICE = "1"
.\target\release\model-host.exe
```

```bash
docker compose -f platform/docker/compose.yaml \
               -f apps/ai/sonic/compose.modelhost.yaml up -d
```

`SONIC_DML_DEVICE` **kritik**: DirectML varsayılan olarak 0 numaralı adaptörü
seçiyor ve çift GPU'lu laptoplarda bu genelde tümleşik karttır. Ayrık kart
boşta beklerken tümleşikte koşmak CPU'dan bile yavaş olabilir. Setup scripti
kartları listeleyip ayrık olanı kendisi buluyor.

`SONIC_MODEL_HOST` ayarlı değilse hiçbir şey değişmez: konteyner içi CPU
çıkarımı. Karışık makineli takımda (Windows + Mac/Linux) varsayılan yol budur
ve `docker compose up` tek başına çalışmaya devam eder. Geri dönmek için
`-f apps/ai/sonic/compose.modelhost.yaml` katmanını kaldırmak yeterli.

**Doğrulama `/healthz`'deki `providers` alanına bakarak yapılmaz** — o alan
derleme özelliklerinden kuruluyor, yani *istenen* sağlayıcı zincirini gösteriyor,
gerçekten etkin olanı değil. ONNX Runtime bulamadığı sağlayıcıyı sessizce
atlıyor. Ölçün: `timing.inference_ms` yukarıdaki tabloyla karşılaştırılmalı ve
analiz sırasında Görev Yöneticisi'nde ayrık kartta hareket görünmeli.

Doğrudan GPU derlemesi de mümkün (sağlayıcı bulunamazsa CPU'ya dönülür):

```bash
cargo build -p sonic --release --features cuda
cargo build -p sonic --release --features directml
```

int8 ağırlıkları CPU'ya özgüdür; GPU derlemelerinde varsayılan fp32'dir.

## Uç noktalar

- `GET /healthz` — model, sağlayıcı, profiller
- `GET /v1/labels` — 527 sınıf (İngilizce + Türkçe)
- `GET /v1/videos` — medya kökündeki videolar (`id`, `filename`, `size`)
- `GET /v1/videos/:id` — uzantısız kimlikten dosya bilgisi
- `POST /v1/upload` — `multipart/form-data`, `file` alanı
- `POST /v1/audio/analyze` — çözümleme

`analyze` isteğindeki `path` uzantılı dosya adı ya da **uzantısız kimlik**
olabilir: `test3` isteği medya kökündeki `test3.mkv` dosyasını bulur. Çağıranın
`.mp4` varsayması mp4 dışında yüklenen her videoyu kırıyordu.

Yükleme yalnız video uzantılarını kabul eder (`mp4, mkv, webm, mov, avi, flv,
wmv, m4v`). Medya kökü genellikle dashboard'un statik kökü olduğu için buraya
yazılan bir `.html` dosyası arayüzün kendi origin'inden servis edilirdi. Aynı
adlı dosyanın üzerine yazılır, aynı kimliği farklı uzantıyla kullanan bir dosya
varsa istek `409` ile reddedilir.

```bash
curl -X POST http://127.0.0.1:8081/v1/audio/analyze \
  -H "Content-Type: application/json" \
  -d '{"path":"video.mp4","profile":"dengeli","threshold":0.35}'
```

İstek alanları: `path` (zorunlu), `profile` (`hassas` | `dengeli` | `isabetli`),
`threshold`, `top_k`, `min_duration_sec`, `gap_sec`, `max_events`,
`include_frames`, `batch_size`.

Yanıt `events` (zaman damgalı olaylar), `summary` (sınıf başına toplam süre),
`safety` (güvenlik olayları ve kural bulguları), istenirse `frames` (pencere
başına ilk-K) ve `timing` içerir.

`events` listesi `max_events` sınırına takılırsa `events_truncated: true` döner.
Kırpma **güvenlik sınıflarını muaf tutar** ve güvenlik kuralları kırpma öncesi tam
liste üzerinde koşar, yani bulgular kırpmadan etkilenmez. `summary` de kırpma
öncesi listeyi anlatır — bu yüzden `summary` içindeki olay sayılarının toplamı,
kırpma olduğunda `events.length`'ten büyük olabilir. `timing`, aşama
bazlı süreleri ve gerçek zaman katsayısını her istekte kendisi ölçer.

## Doğrulama

Mel ön ucunun referansla uyumunu sınayan kapı — herhangi bir mel parametresi
kayarsa model sessizce saçmalar, bu yüzden hat üzerindeki en kritik test:

```bash
cargo run -p inference --bin verify-mel
```

Dosyanın tamamını tek pencere olarak besler (sherpa-onnx'in kurulumuyla birebir
aynı), ilk-5 etiketi yayımlanmış referans sonuçlarla karşılaştırır ve `prob`
çıktısının [0,1] aralığında olduğunu — yani sigmoid'in grafiğe gömülü olduğunu —
teyit eder.

İkinci kapı, süreç içi çözmenin ffmpeg ile aynı sonucu verdiğini sınar — hız
kazancı ancak kalite değişmiyorsa geçerlidir:

```bash
cargo run -p inference --release --bin compare-decoders -- video.mp4
```

Örnek sayısı, RMS, hizalama kayması, hizalama sonrası kalan fark ve modelin
ilk-5 etiket/skorlarını karşılaştırır. Çözücü tarafında bir şey değiştirildiğinde
bu araç tekrar çalıştırılmalı.

Birim testler: `cargo test -p inference`. Bunlara resampler kalite testleri de
dahildir (440 Hz tonun genliği korunmalı, 12 kHz Nyquist üstü ton bastırılmalı —
aksi halde aliasing mel spektrumunu bozar).
