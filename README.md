# MotifAI — Video Analiz ve Karar Destek Sistemi

TEKNOFEST 2026 Yapay Zekâ Dil Ajanları Yarışması, 3. Senaryo.

Bir güvenlik kamerası kaydı yüklenir; sistem olayları zaman damgalarıyla
çıkarır, Türkçe özet üretir, risk değerlendirmesi yapar ve operatöre aksiyon
önerir. Çıktı şartname §5'teki biçimdedir:

```json
{
  "summary": "Bir depoda forklift, raf sistemine çarparak kaza meydana getirir…",
  "events": [
    {"time": "00:12", "event": "Forklift sağdaki raf sistemine çarpar"},
    {"time": "00:13", "event": "Raf çöker, toz bulutu oluşur"}
  ],
  "risk": "Yüksek",
  "actions": ["Alanı boşalt", "Sağlık ekibini çağır"]
}
```

## Nasıl çalışıyor

Videonun tamamını modele vermek pahalı ve doğruluğu düşürüyor. Sistem bunun
yerine **nereye bakılacağına** karar veriyor:

```
video ──► stream ──────────────► vision ──────────► rapor
          H.264'e normalize      klip ister          {summary, events,
          hareket profili        modele sorar         risk, actions}
          klip üretir            gerekirse
          ağır çekim             yakınlaştırır
```

| Bileşen | Ne yapar |
|---|---|
| `apps/stream` | Video alımı, H.264 normalizasyonu, hareket analizi, klip üretimi, ajan araç yüzeyi |
| `apps/ai/vision` | Analiz ajanı: klip ister, çıkarım servisine sorar, şartname raporunu üretir |
| `apps/ai/inference` | Ses olayı sınıflandırma (CED modeli) |
| `apps/gateway` | Kimlik doğrulama ve yönlendirme (Ory Kratos/Keto) |
| `apps/dashboard` | Operatör paneli (Next.js) |
| `packages/optics` | Kare çözme, hareket ölçümü, örnekleme, klip kesme |
| `packages/event-sdk` | Servisler arası tipler ve şartname rapor biçimi |
| `tools/bench` | Ölçüm harness'ı — olay kapsama, azaltma oranı, hız |

Boru hattı **nereye bakılacağına** karar verir; **ne olduğuna** yalnızca model
karar verir. `packages/optics` içindeki hiçbir modül "kaza oldu" demez.

## Gereksinimler

- **Rust** 1.80+ — `rustup` ile
- **ffmpeg** ve **ffprobe** — PATH üzerinde olmalı
- **Node.js 20+ / pnpm** — yalnızca panel için
- **EVREN çıkarım servisi anahtarı** — takıma e-posta ile iletiliyor

`protoc` gerekmiyor; paketlenmiş ikili kullanılıyor.

> **Windows notu:** `apps/ai/inference` (ses analizi) ONNX Runtime kullanıyor ve
> bağlanırken `DirectML.lib` / `DXCORE.lib` arıyor. Bunlar güncel Windows SDK ile
> geliyor; VS 2017 Build Tools yeterli değil ve `LNK1181` alırsınız. Video
> tarafı (`stream`, `vision`) bundan etkilenmiyor.

ffmpeg kurulumunu doğrulamak için:

```bash
ffmpeg -version && ffprobe -version
```

## Çalıştırma

Anahtar **hiçbir dosyaya yazılmaz**, yalnızca ortam değişkeni olarak verilir.
Bu depo halka açıktır.

```bash
export EVREN_KEY="sk-evren-..."
```

İki servis gerekiyor. Ayrı kabuklarda:

```bash
cargo run --release -p stream
```

```bash
cargo run --release -p vision
```

`stream` 8100, `vision` 8110 portunu dinler. Hazır olduklarını doğrulayın:

```bash
curl localhost:8100/healthz && curl localhost:8110/healthz
```

### Bir videoyu analiz etmek

Yükleyin — dönen `id` sonraki adımda kullanılır:

```bash
curl -X POST -F "file=@kayit.mp4" localhost:8100/v1/videos
```

Şartname biçiminde rapor alın:

```bash
curl -X POST localhost:8110/v1/analyze/sartname -H 'content-type: application/json' -d '{"video_id":"BURAYA-ID"}'
```

Ajanın hangi adımlardan geçtiğini de görmek isterseniz `/v1/analyze` uç
noktası raporu olay başına `t_ms`, `severity` ve adım listesiyle döndürür.

### Panel

```bash
pnpm --dir apps/dashboard install
```

```bash
pnpm --dir apps/dashboard dev
```

Panel kimlik doğrulama için `apps/gateway` ve Ory yığınının çalışmasını
bekler:

```bash
docker compose -f platform/docker/compose.yaml up -d
```

Analiz burada yapılıyor. Bir videonun sayfasında (`/videos/{id}`):

- **Hareket ısı haritası** video üzerinde, oynatmayla eşzamanlı — boru hattının
  nereye baktığı görünür olsun diye.
- **Hareket profili şeridi**: toplam hareket, en hareketli bölge, sahne
  kesitleri ve raporlanan olaylar aynı zaman ekseninde. Tıklamak o ana gider.
- **Görsel analiz** sekmesi şartname çıktısını verir: özet, risk, zaman damgalı
  olaylar ve aksiyon önerileri. Olaya tıklamak videoyu o ana götürür.
- **Modele giden yük** katlanır bölümü, gönderilen klibin kendisini oynatır;
  yanında ağır çekim oranı, servisin göreceği kare sayısı, token tahmini ve
  istemin tam metni durur.

Yüklenen video hem ses hem görüntü servisine gider; ikisi orijinal dosya adı
üzerinden eşleşir. Görüntü servisi kapalıysa ses analizi çalışmaya devam eder,
yalnız görsel analiz o video için kapalı kalır.

## Ölçüm

Şartname §4 katılımcıların kendi metriklerini tanımlamasını istiyor. Birincil
metrik **olay kapsama oranı**: elle etiketlenmiş olaylardan kaçının seçilen
karelere düştüğü. Bu, "örnekleme olayı kaçırdı mı" sorusunu "model anladı mı"
sorusundan ayırıyor.

```bash
cargo run --release -p motif-bench -- run --dataset Utilities/golden-dataset/videos
```

10 videoluk elle etiketlenmiş küme üzerinde son ölçüm:

| Metrik | Değer |
|---|---|
| Olay kapsama (örnekleme) | %92 — 36/39 |
| Olay eşleşmesi (uçtan uca, ±3 sn) | %94 — 37/39 |
| Geçerli şartname JSON'u | 10/10 |
| Ham kareye göre azaltma | 57× |
| Ortalama zaman sapması | 223 ms |
| Analiz süresi (video başına) | ortalama 12,5 sn |

Kaçırılan olayların tamamı **düşük hareketli** — hareket güdümlü örneklemenin
bilinen ve belgelenmiş zayıflığı.

## Ölçümden çıkan üç karar

Sistemdeki bazı tercihler ilk bakışta tuhaf görünüyor; üçü de ölçümle
zorunlu hâle geldi:

**Klip gönderiliyor, kare değil.** Çıkarım servisi en fazla iki görüntü kabul
ediyor, `vlm` modeli görüntüyü tamamen reddediyor. Zamansal içeriğin tek
teslim yolu video.

**Yakınlaştırma ağır çekimle yapılıyor.** Servis her videoyu sabit 2 fps
örneklüyor, yani dar pencere göndermek çözünürlüğü artırmıyor: 2 saniyelik
klipten yine 4 kare çıkıyor. Detay için pencere yavaşlatılıyor.

**Zaman çevirisi kodda yapılıyor.** Ağır çekimde model klibin kendi saatini
raporluyor; isteme dönüşüm formülü açıkça yazılsa bile aritmetiği güvenilir
yapmıyor. Çeviriyi `ClipRef::to_source_ms` üstleniyor.

Ayrıntılar ve ölçüm gövdeleri:
[`documents/architecture/evren-servisi-bulgular.md`](documents/architecture/evren-servisi-bulgular.md)
ve [`documents/architecture/stream-benchmark.md`](documents/architecture/stream-benchmark.md).

## Geliştirme

```bash
cargo test --workspace
```

Ses servisi Windows'ta yukarıdaki SDK notu yüzünden bağlanamıyorsa video
tarafını ayrı koşabilirsiniz:

```bash
cargo test -p motif-optics -p motif-event-sdk -p stream -p vision -p motif-bench
```

```bash
cargo clippy --workspace --all-targets
```

Golden dataset araçları `Utilities/golden-dataset/` altında: aday video
ekleme, indirme, elle etiketleme arayüzü ve bench'e dönüştürme.

## Lisans

Apache-2.0 — [LICENSE](LICENSE).
