# inference — ses olay tespiti servisi

Videonun ses kanalından zaman damgalı olay listesi çıkarır. Model **CED**
(AudioSet, 527 sınıf), çalışma zamanı **ONNX Runtime**, ön uç Rust'ta yazılmış
log-mel.

## Kurulum

Model ağırlıkları depoda tutulmaz, script ile indirilir:

```bash
sh apps/ai/inference/scripts/fetch-models.sh ced-base
```

Windows PowerShell için: `apps\ai\inference\scripts\fetch-models.ps1`.

`ced-tiny`, `ced-mini`, `ced-small`, `ced-base` indirilebilir; hepsi aynı
öznitelik hattını kullanır, yalnız ağırlık dosyası değişir.

**Varsayılan `ced-base`.** Aynı videoda Tiny'nin ürettiği yanlış pozitifleri
(at, kalp atışı, hapşırık, baykuş) hiç üretmiyor, gerçek sesleri koruyor.
Bedeli: indirme 410 MB (Tiny 29 MB) ve çıkarım ~4,5× yavaş — ama 9 dakikalık
video yine 7 saniyenin altında bitiyor. Ölçüm ve karşılaştırma:
aşağıdaki "Ölçümler" bölümü.

Hız öncelikliyse `INFERENCE_MODEL=ced-tiny` yeterli; iki modeli yan yana
tutup duruma göre geçebilirsiniz.

Ses çözme süreç içinde `symphonia` ile yapılır (WAV, FLAC, MP3, AAC/MP4, MKV,
OGG…). `ffmpeg` yalnız symphonia'nın çözemediği formatlar için **yedek** yoldur;
kurulu olması şart değildir ama olması kapsamı genişletir. Hangi yolun
kullanıldığı yanıttaki `media.decoder` alanında görünür.

## Çalıştırma

```bash
INFERENCE_MEDIA_ROOT=/path/to/media cargo run -p inference --release
```

Servis **yalnız `127.0.0.1:8081`** dinler ve bu adres yapılandırılamaz: kendi
kimlik doğrulaması yoktur. Tasarımda dışarıya açılan kapı gateway'dir, ama
**bugün gateway akışta değil** — dashboard doğrudan buraya bağlanıyor
(bkz. `audio-setup.md`). Bu yüzden tarayıcı kökeni de yerel arayüzle sınırlı:
yalnız `localhost` / `127.0.0.1` / `[::1]` kökenli sayfalar istek atabilir.

### Ortam değişkenleri

| Değişken | Varsayılan | Açıklama |
|---|---|---|
| `INFERENCE_PORT` | `8081` | Dinlenen port (adres her zaman 127.0.0.1) |
| `INFERENCE_MODELS_DIR` | `<crate>/models` | Model kök dizini |
| `INFERENCE_MODEL` | `ced-base` | Alt dizin adı (`ced-tiny`, `ced-small`, …) |
| `INFERENCE_INT8` | CPU'da `true` | int8 ağırlıkları tercih et |
| `INFERENCE_THREADS` | çekirdek sayısı | ONNX Runtime iş parçacığı |
| `INFERENCE_BATCH` | CPU `32`, GPU `64` | Tek çağrıdaki pencere sayısı |
| `INFERENCE_MEDIA_ROOT` | *(yok)* | Ayarlanırsa istenen yollar bu kökün dışına çıkamaz |
| `INFERENCE_MAX_UPLOAD_BYTES` | `0` (sınırsız) | Yükleme tavanı; dosya diske akıtıldığı için varsayılan sınırsız |

`INFERENCE_MEDIA_ROOT` ayarlı değilse servis başlarken uyarı basar ve yerel
dosya sistemindeki herhangi bir yolu okuyabilir — üretimde mutlaka ayarlayın.

## GPU

Hedef makinede GPU varsa ilgili özellikle derleyin; sağlayıcı bulunamazsa ONNX
Runtime sessizce CPU'ya döner, yani aynı kaynak her iki makinede de çalışır.

```bash
cargo build -p inference --release --features cuda
cargo build -p inference --release --features tensorrt   # fp16 motoru kendisi kurar
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
