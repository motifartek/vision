# `apps/stream` + `packages/optics` — Kodlama Faz Planı

> Sahibi: @fatihkaraca1 · İlgili issue'lar: #1, #6 · Hedef: Grand Final
> Bu doküman kodlama sırasında açık tutulmak üzere yazıldı. Her fazın sonunda
> **çalıştırılabilir bir komut** ve **elle görülebilir bir çıktı** var.

## Tasarımın tek cümlesi

> Boru hattı **nereye bakılacağına** karar verir; **ne olduğuna** sadece model karar verir.

Bu ilke şartname §4'teki *"Statik, yalnızca kural tabanlı çözümler düşük puanlanacaktır"*
maddesinin cevabıdır ve aşağıdaki her teknik seçimin gerekçesidir.

## Crate haritası

```
packages/
  core/         → paylaşılan tipler, hata tipleri, ID üretimi, config
  optics/       → video I/O + hareket analizi + örnekleme   ★ İŞİN KALBİ
  event-sdk/    → NATS subject sabitleri + mesaj tipleri (serde)
  database/     → (#2, Deniz)
apps/
  stream/       → servis: MinIO + NATS + tool API
tools/
  bench/        → ölçüm harness'ı, KPI raporu
```

`optics` saf kütüphane + bir CLI binary'si olacak. Ağ, MinIO, NATS bilmeyecek —
sadece dosya girer, veri çıkar. Bu sayede tek başına test edilebilir ve
benchmark'lar servis ayağa kaldırmadan koşar.

---

# Faz 0 — Zemin (0.5 gün)

**Amaç:** Repoyu derlenir hale getirmek ve ekibi bloke eden kontratları ilk gün dışarı vermek.

**Neden ilk:** Kök `Cargo.toml` şu anda **derlenmiyor** — workspace üyesi olarak listelenen
8 dizinin `Cargo.toml`'u yok. Ayrıca #2/#3/#4'ün üçü de mesaj şemalarını bekliyor;
bunlar sadece struct tanımı olduğu için ucuz, ama beklettiğin her saat üç kişiyi durduruyor.

**Dosyalar**

```
packages/core/Cargo.toml + src/lib.rs
packages/optics/Cargo.toml + src/lib.rs
packages/event-sdk/Cargo.toml + src/lib.rs
apps/stream/Cargo.toml + src/main.rs
tools/bench/Cargo.toml + src/main.rs
Cargo.toml (workspace: [workspace.dependencies] ile ortak sürümler)
```

**Yapılacaklar**

- [ ] Kök `Cargo.toml`'daki hayalet üyeleri temizle; gerçekten var olan crate'leri bırak
- [ ] `[workspace.dependencies]` bloğu ile serde/tokio/tracing/thiserror sürümlerini tek yerden yönet
- [ ] `packages/core`: `VideoId`, `AppError`, `Result<T>`, tracing init helper
- [ ] `packages/event-sdk`: NATS subject sabitleri + ilk mesaj tipleri (aşağıda)
- [ ] `ffmpeg -version` / `ffprobe -version` kontrolü yapan bir preflight fonksiyonu
- [ ] `documents/features/stream-service.md` iskeleti (kurulum adımları buraya yazılacak)

**Kilit detay — event-sdk ilk taslak**

```rust
pub mod subjects {
    pub const VIDEO_INGESTED:  &str = "stream.video.ingested";
    pub const FRAME_EXTRACTED: &str = "stream.frame.extracted";
    pub const RISK_DETECTED:   &str = "event.risk.detected";
    // request/reply (pass 3 tool çağrıları)
    pub const TOOL_PREFIX:     &str = "stream.tool.";
}

#[derive(Serialize, Deserialize)]
pub struct FrameRef {
    pub t_ms: u64,
    pub object_key: String,
    pub motion_score: f32,
    pub is_scene_cut: bool,
}
```

> Bu tipler **ilk taslak**. Faz 5'te gerçekle çarpışınca revize edilecek —
> ekibe böyle duyur ki erken bağımlılık kurmasınlar.

**✅ Nihai çıktı**

```bash
cargo build --workspace
```

Workspace hatasız derleniyor, `cargo test --workspace` koşuyor.
Ekip `event-sdk`'ye bakıp kendi tarafını yazmaya başlayabiliyor.

---

# Faz 1 — Video I/O katmanı (1 gün)

**Amaç:** ffmpeg'i subprocess olarak sürüp ham gri kareleri Rust'a akıtmak.

**Neden bu sırada:** Projedeki **en riskli teknik varsayım** bu. Çalışmazsa tüm plan değişir,
o yüzden ilk gün öğrenmek istiyoruz. OpenCV binding'lerine bilerek girmiyoruz — Windows'ta
derleme süreci gerçek bir zaman kaybı ve 10 günümüz yok.

**Dosyalar**

```
packages/optics/src/probe.rs      → ffprobe sarmalayıcı
packages/optics/src/decode.rs     → ffmpeg raw pipe decoder
packages/optics/src/types.rs      → VideoInfo, AnalysisFrame
packages/optics/src/bin/optics.rs → CLI (clap)
```

**Kilit detay — metadata**

```bash
ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate,codec_name -show_entries format=duration,size -of json input.mp4
```

`r_frame_rate` `"30000/1001"` gibi kesir gelir — pay/payda ayrıştır, float'a çevir.

**Kilit detay — ham gri kare akışı**

```bash
ffmpeg -v error -i input.mp4 -vf "fps=15,scale=160:90,format=gray" -f rawvideo -pix_fmt gray -
```

- Kare başına tam **160 × 90 = 14 400 bayt**. `read_exact` ile blok blok oku.
- `fps=15` filtresi çıktıyı **sabit kare hızına zorlar**, dolayısıyla
  `t_ms = index * 1000 / 15` **kesin** olur. VFR (değişken kare hızlı) videolarda bile.
- Analiz için 15 fps fazlasıyla yeterli; 30 fps decode etmenin anlamı yok. Bu tek satır
  işi iki katına kadar hızlandırıyor.
- `analysis_fps` ve analiz çözünürlüğü **config olacak**, koda gömülmeyecek.

**Tipler**

```rust
pub struct VideoInfo {
    pub duration_ms: u64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub codec: String,
}

pub struct AnalysisFrame {
    pub index: u32,
    pub t_ms: u64,
    pub data: Vec<u8>, // w*h gri piksel
}
```

Decoder bir `Iterator<Item = Result<AnalysisFrame>>` döndürsün — tüm videoyu belleğe
almadan akıtmak için. **Bellek kullanımı video uzunluğundan bağımsız kalmalı** (KPI).

**Doğrulama — kendi test videonu üret**

```bash
ffmpeg -f lavfi -i testsrc2=size=640x360:rate=30 -t 60 test.mp4
```

`testsrc2` üzerinde **görünür bir sayaç** var. Kare 450'yi çıkarıp üzerindeki sayaçla
kendi hesapladığın `t_ms`'i karşılaştır — zaman matematiğini gözle doğrulamanın en hızlı yolu.

**✅ Nihai çıktı**

```bash
optics decode test.mp4 --limit 500 --timing
```

```
500 kare çözüldü · 3.2 sn · 156 kare/sn · 10.4x realtime · tepe bellek 41 MB
```

**En riskli varsayım kanıtlanmış olur** ve elinde ilk gerçek performans sayısı olur.

---

# Faz 2 — Hareket profili (1 gün)

**Amaç:** Videonun tamamı için tek boyutlu "nerede bir şey oluyor" eğrisini çıkarmak.

**Dosyalar**

```
packages/optics/src/motion.rs   → SAD, normalizasyon
packages/optics/src/scenecut.rs → sahne kesiti tespiti
packages/optics/src/hash.rs     → dHash
packages/optics/src/profile.rs  → MotionProfile + serde
```

**Kilit detay — SAD (Sum of Absolute Differences)**

```rust
fn sad(a: &[u8], b: &[u8]) -> u64 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as u64).sum()
}
```

Hepsi bu. Normalize: `score = sad / (w * h * 255)` → 0..1 aralığı.
14 400 baytlık iki dizi üzerinde çalıştığı için son derece hızlı; darboğaz decode tarafı olacak.

**Kilit detay — sahne kesiti, eşiksiz**

Sabit eşik ("0.3'ü geçerse kesittir") **statik kural** sayılır ve puan kaybettirir.
Bunun yerine kayan pencerede **medyan + MAD** (Median Absolute Deviation) kullan:

```
pencere = son N örnek
kesit  ⟺ score > medyan(pencere) + k · MAD(pencere)
```

Eşik videonun kendi istatistiğinden türüyor; karanlık/gürültülü videoda otomatik
yükseliyor, sakin videoda düşüyor. Elle seçilen tek şey `k` (≈3), o da istatistiksel
bir sabit — sahneye özel bir kural değil. Jüriye savunması kolay.

**Kilit detay — dHash (dedup için)**

pHash yerine dHash yeterli ve daha basit: kareyi 9×8'e küçült, her satırda komşu
pikselleri karşılaştır (`p[x] > p[x+1]`) → 8 bit × 8 satır = **64 bitlik hash**.
İki kare arasındaki Hamming mesafesi ≤ 5 ise "aynı" say. Zaten elimizde gri
küçültülmüş kare var, ekstra decode gerekmiyor.

**Tipler**

```rust
pub struct MotionSample {
    pub t_ms: u64,
    pub score: f32,
    pub is_scene_cut: bool,
    pub dhash: u64,
}

pub struct MotionProfile {
    pub video_id: String,
    pub analysis_fps: f64,
    pub duration_ms: u64,
    pub samples: Vec<MotionSample>,
}
```

Profil **diske/MinIO'ya yazılacak** — Faz 3 ve tool çağrıları bunu asla yeniden hesaplamayacak.

**Görselleştirme (atlama!)**

CLI terminale ASCII sparkline bassın, ayrıca `--svg` ile eğriyi SVG olarak versin.
Hem debug ederken hem **jüri demosunda** çok işe yarayacak: "bakın, sistem videonun
neresine dikkat ettiğini gösteriyor" demek somut bir görsel.

**✅ Nihai çıktı**

```bash
optics profile kaza.mp4 --out profile.json --svg profile.svg
```

```
00:00 ▁▁▂▁▁▁▃▂▁▁▅█▇▄▂▁▁▁▂▁ 00:40
                 ↑ 00:14.2 sahne kesiti
612 örnek · 3 sahne kesiti · 41 sn video · 2.1 sn işlem · 19x realtime
```

Herhangi bir video için hareket eğrisi + ilk somut throughput sayısı.

---

# Faz 3 — Adaptif örnekleme (1.5 gün) ★ Projenin fikir merkezi

**Amaç:** Hareket eğrisi + kare bütçesi → seçilmiş kareler, tam kalitede, doğru zaman damgasıyla.

**Dosyalar**

```
packages/optics/src/sample.rs   → ters dönüşüm örneklemesi
packages/optics/src/extract.rs  → tam kaliteli kare çıkarma
packages/optics/src/overlay.rs  → zaman damgası bindirme
```

**Kilit detay — hareket ekseninde örnekleme**

Zaman ekseninde eşit aralık değil, **hareket ekseninde** eşit aralık:

```
1. m'[i] = m[i] + α · ortalama(m)        ← uniform prior eklenir
2. M[i]  = kümülatif toplam(m')
3. j = 0..N için:  hedef = (j + 0.5) · M[son] / N
                   i = ikili_arama(M, hedef)  →  kareyi seç
```

`α` (uniform prior) tek ayar düğmesi ve **elle eşik seçmeni tamamen ortadan kaldırıyor**:

| α | Davranış |
|---|---|
| `0` | Saf hareket odaklı — sakin bölgelerden hiç kare almaz |
| `0.2` | Önerilen başlangıç — hareketi önceler ama sessizliği de tarar |
| `→ ∞` | Sabit aralıklı örnekleme (baseline) |

Bu tasarımın güzelliği: **baseline karşılaştırması ayrı kod gerektirmiyor.**
`α`'yı süpürüp recall eğrisini çizersin, sabit örnekleme eğrinin bir ucunda kalır.
Şartnamenin istediği ölçüm raporu için birebir malzeme.

> ⚠️ **`α`'nın sıfır olmaması kritik.** "Yerde hareketsiz kişi" şartnamedeki örnek
> olaylardan biri ve tanımı gereği **hareketsiz**. Saf hareket odaklı örnekleme
> bu olayı yapısal olarak kaçırır. Uniform prior bunun sigortası.

**Koruma bantları**

- Sahne kesitleri her zaman seçime dahil (bütçe dışı, ek olarak)
- `min_gap_ms` — birbirine çok yakın kareleri ayıkla
- dHash dedup — Hamming ≤ 5 olanlardan birini at, yerine sıradaki adayı al

**Kilit detay — tam kaliteli çıkarma**

```bash
ffmpeg -ss 15.200 -i input.mp4 -frames:v 1 -q:v 2 -y frame_015200.jpg
```

- `-ss`'i `-i`'den **önce** koy (hızlı arama). Modern ffmpeg bunu çoğu formatta
  isabetli yapar — ama **Faz 4'te doğrula**: çıkarılan karenin gerçekten istenen
  saniyede olduğunu `testsrc2` sayacıyla kontrol et. Sapma varsa
  `-ss (t-1) -i input -ss 1` iki aşamalı arama desenine geç.
- ~30 kare için ayrı ayrı ffmpeg süreci açmak kabul edilebilir; gerekirse paralelleştir.

**Kilit detay — zaman damgası bindirme**

VLM kare *sırasını* bilir, *saati* bilmez — ama şartname puanı saatten veriyor.
İki yol, ikisini de A/B'ye sok:

1. **Görsel bindirme:** ffmpeg `drawtext` filtresi. Windows'ta font yolu sorun
   çıkarabilir (`fontfile=` ile mutlak yol vermek gerekir).
   **Yedek plan:** rakamlar ve iki nokta için elle yazılmış 5×7 bitmap font, ~50 satır
   Rust, sıfır bağımlılık riski. Bindirmeyi kendin piksel olarak bas.
2. **Metin indeksi:** kare listesiyle birlikte `"Kare 7 → t=00:15.2"` eşlemesi geç.

**Config**

```rust
pub struct SamplingConfig {
    pub budget: usize,          // çalışma zamanı — donanım belli değil
    pub uniform_prior: f32,     // α
    pub min_gap_ms: u64,
    pub dedup_hamming: u32,
    pub force_scene_cuts: bool,
    pub timestamp_overlay: bool,
}
```

**✅ Nihai çıktı**

```bash
optics sample kaza.mp4 --budget 16 --alpha 0.2 --out frames/ --overlay
```

```
16 kare seçildi (+2 sahne kesiti) · 3 kopya elendi
frames/000920.jpg  t=00:09.2  m=0.11
frames/014200.jpg  t=00:14.2  m=0.94  ← sahne kesiti
...
```

**Boru hattının çekirdeği bitmiş olur.** Bu noktada elinde, herhangi bir videodan
doğru zaman damgalı, VLM'e gönderilmeye hazır kareler var.

---

# Faz 4 — Ölçüm harness'ı (1 gün)

**Amaç:** "Örnekleme olayı kaçırdı mı?" sorusunu modelden **bağımsız** olarak cevaplamak.

**Neden bu sırada:** Buradan sonrası servis/altyapı işi. Algoritmanın doğru
olduğunu **önce** kanıtla, sonra etrafına servis sar. Ayrıca şartnamenin zorunlu
tuttuğu ölçüm raporunun malzemesi burada üretiliyor.

**Dosyalar**

```
tools/bench/src/main.rs
tools/bench/src/recall.rs
tools/bench/src/report.rs
documents/dataset/ground-truth.schema.json
```

**Ana metrik — Event Coverage Recall**

```
recall = (±1.0 sn içinde en az bir seçilmiş karesi olan ground-truth olay sayısı)
         ─────────────────────────────────────────────────────────────────────
                      (toplam ground-truth olay sayısı)
```

Bu metriğin değeri: VLM'e hiç dokunmadan senin tarafını ölçüyor. Recall %60 ise
sorun örneklemededir, modelde değil — ve tersi.

**Ground truth formatı** (Berat'ın #5'i ile aynı olmalı, koordine et)

```json
{
  "video": "kaza_01.mp4",
  "duration_ms": 41000,
  "events": [
    { "t_ms": 14200, "label": "forklift devrildi", "severity": "high" }
  ]
}
```

**Bloke olma:** #5 (Golden Dataset) Berat'ta ve gecikebilir. **Bekleme** —
kendin 8-10 video alıp elle etiketle, harness'ı onunla çalıştır. Berat'ın seti
gelince aynı harness büyük veri üzerinde koşar.

**Koşulacak deneyler**

- [ ] `α` süpürmesi: 0 → 1 arası recall eğrisi (+ sabit örnekleme baseline'ı)
- [ ] Bütçe süpürmesi: 8 / 16 / 32 / 64 kare
- [ ] Zaman damgası isabet doğrulaması (`-ss` arama hassasiyeti)
- [ ] Zaman damgası bindirmesi açık/kapalı — VLM geldikten sonra ölçülür, yer tut

**✅ Nihai çıktı**

```bash
bench run --dataset documents/dataset/mini/ --sweep alpha
```

```
α        recall   kare   zaman hatası
0.00     78.3%    16     0.42 sn
0.20     96.7%    16     0.38 sn   ★
1.00     91.2%    16     0.51 sn
uniform  84.1%    16     0.44 sn
```

**Elinde sayı var.** Artık "adaptif örnekleme daha iyi" bir iddia değil, ölçüm.
Bu tablo doğrudan jüri raporuna gidiyor.

---

# Faz 5 — Kontratların kesinleşmesi (0.5 gün)

**Amaç:** Faz 1-4'te öğrendiklerinle `event-sdk`'yi gerçeğe göre düzeltmek.

Faz 0'daki taslak tipler artık gerçek verinin nasıl göründüğünü bilerek revize edilir:
alan isimleri, opsiyonellik, tool istek/cevap tipleri.

**Yapılacaklar**

- [ ] `FrameExtracted` / `VideoIngested` son hali
- [ ] 6 tool için `Request`/`Response` tipleri
- [ ] NATS subject şeması: `stream.tool.zoom_range` vb.
- [ ] `schema_version` alanı ekle — kırılma olursa fark edilsin
- [ ] Ekibe değişikliği #6 altında yorum olarak duyur

**✅ Nihai çıktı:** Deniz ve AI tarafı kesinleşmiş tiplere karşı kod yazabiliyor.

---

# Faz 6 — `apps/stream` servisi (1.5 gün)

**Amaç:** Kütüphaneyi ağa bağlamak. Video girer → MinIO + NATS çıkar.

**Dosyalar**

```
apps/stream/src/main.rs
apps/stream/src/ingest.rs    → yükleme + MinIO
apps/stream/src/pipeline.rs  → pass 1+2 orkestrasyonu
apps/stream/src/storage.rs   → MinIO istemcisi (aws-sdk-s3, custom endpoint)
apps/stream/src/cache.rs     → profil önbelleği
```

**Yapılacaklar**

- [ ] MinIO bağlantısı (`aws-sdk-s3`, `endpoint_url` + `force_path_style`)
- [ ] Ham videoyu `raw/<video_id>.mp4` olarak yaz
- [ ] `ffprobe` → `stream.video.ingested` yayınla
- [ ] Pass 1 + Pass 2 çalıştır, kareleri `frames/<video_id>/<t_ms>.jpg` olarak yaz
- [ ] `stream.frame.extracted` yayınla
- [ ] Profili önbelleğe al (tool çağrıları yeniden hesaplamasın)
- [ ] Graceful shutdown, yapılandırılmış loglama, hata yolları

**Dikkat**

- Kareler mesaja **base64 gömülmeyecek** — MinIO anahtarı geçilecek. NATS mesajı küçük kalsın.
- Uzun videoda pass 1 dakikalar sürebilir; bu iş **arka planda** koşmalı, HTTP isteğini bloklamamalı.

**✅ Nihai çıktı**

```bash
make run:dev stream
```

Video POST edildiğinde: NATS'ta `stream.frame.extracted` mesajı görünüyor,
MinIO'da 16 JPEG + `profile.json` duruyor. Uçtan uca ilk gerçek akış.

---

# Faz 7 — Tool API / Pass 3 (1 gün)

**Amaç:** Ajanın videoyu *soruşturabilmesi*. Tasarımın en ayırt edici parçası.

**Dosyalar**

```
apps/stream/src/tools/mod.rs
apps/stream/src/tools/{zoom,frame,crop,profile,info}.rs
apps/stream/src/segment_cache.rs
```

**6 tool**

```
video_info(video_id)                        -> VideoInfo
motion_profile(video_id, bucket_ms)         -> [MotionSample]
sample_overview(video_id, budget)           -> [FrameRef]
zoom_range(video_id, t0_ms, t1_ms, budget)  -> [FrameRef]
get_frame(video_id, t_ms, max_dim)          -> FrameRef
crop_region(video_id, t_ms, bbox)           -> FrameRef
```

**Yapılacaklar**

- [ ] NATS request/reply handler'ları (`stream.tool.*`)
- [ ] Segment önbelleği — aynı bölgeye tekrar zoom ucuz olsun
- [ ] Zoom özyineleme limiti (worst-case gecikme sınırı — #6'da açık madde)
- [ ] Her tool için timeout + anlamlı hata cevabı
- [ ] Tool şemalarını AI tarafının okuyabileceği biçimde dışa ver

**✅ Nihai çıktı**

```bash
nats req stream.tool.zoom_range '{"video_id":"x","t0_ms":12000,"t1_ms":17000,"budget":12}'
```

12 kare, 12.0–17.0 sn arası yoğun, ~40 ms.

**Ajan artık videoya zoom yapabiliyor.** Şartnamenin "mock fonksiyonların ajanın
araçları olarak kullanılması" (%35) ve "dinamik araç seçimi / çok adımlı karar
zincirleri" (%20) maddeleri bu noktada karşılanmış olur.

---

# Faz 8 — Entegrasyon, tuning, dokümantasyon (2 gün)

**Amaç:** AI tarafıyla birleşmek, sayıları iyileştirmek, teslim edilebilir hale getirmek.

**Yapılacaklar**

- [ ] `apps/ai/orchestrator` ile uçtan uca ilk tam akış
- [ ] Gerçek VLM ile: zaman damgası bindirmesi A/B — nihayet ölçülebilir
- [ ] Kare bütçesini gerçek VRAM'e göre ayarla (donanım belli olunca)
- [ ] YOLO on/off benchmark'ı (#6 kararı — negatif sonuç da raporlanacak)
- [ ] Golden Dataset (#5) tam seti üzerinde final KPI koşusu
- [ ] `documents/features/stream-service.md`: kurulum, ffmpeg bağımlılığı, çalıştırma adımları
- [ ] Mimari diyagram (şartname teslim listesinde zorunlu)
- [ ] Demo için: hareket eğrisi SVG'si + zoom davranışının görsel anlatımı

**✅ Nihai çıktı:** Teslim edilebilir stream servisi + şartnamenin istediği ölçüm raporu.

---

# Risk kaydı

| Risk | Etki | Erken sinyal | Karşılık |
|---|---|---|---|
| ffmpeg pipe decode beklenenden yavaş | Yüksek | Faz 1 timing | `analysis_fps`/çözünürlük düşür, decode'u paralelleştir |
| `-ss` araması isabetsiz | Yüksek — tüm zaman damgaları kayar | Faz 4 doğrulama | İki aşamalı `-ss` desenine geç |
| `drawtext` Windows'ta font bulamıyor | Orta | Faz 3 | Elle bitmap font (yedek plan hazır) |
| #5 Golden Dataset gecikir | Orta | Faz 4 | Kendi mini setinle ilerle, sonra büyüt |
| VLM zaman damgasını yine de tutturamaz | Yüksek | Faz 8 | Bindirme + metin indeksi + zoom'da tek kare sorgusu |
| Donanım geç belli olur | Orta | — | Bütçeler zaten çalışma zamanı config'i |
| VFR video zaman matematiğini bozar | Orta | Faz 1 `testsrc2` | `fps=` filtresi zaten CFR'ye zorluyor |

# Bağımlılık haritası

```
Faz 0 ──┬──► Faz 1 ──► Faz 2 ──► Faz 3 ──► Faz 4 ──► Faz 5 ──► Faz 6 ──► Faz 7 ──► Faz 8
        │                                    ▲                                      ▲
        └──► (event-sdk taslağı)             │                                      │
             → #2 #3 #4 açılır         #5 golden dataset                     #7 #8 model
```

# Geri kalırsak kesme sırası

Sondan başa doğru kes, **asla baştan değil**:

1. **`crop_region`** — en az kritik tool, VLM tam kareyle de idare eder
2. **SVG görselleştirme** — demo için hoş ama fonksiyonel değil
3. **YOLO benchmark'ı** — "denemedik" demek "yanlış yaptık" demekten iyidir
4. **`sample_overview` tool'u** — pass 2 zaten otomatik koşuyor, ajanın tekrar çağırmasına gerek yok
5. **Segment önbelleği** — yavaş ama çalışır

**Asla kesilmeyecekler:** Faz 1-3 (çekirdek boru hattı), Faz 4 (ölçüm — şartname zorunlu),
`zoom_range` (tasarımın ayırt edici parçası ve %20'lik otonomi puanının dayanağı).

# Bitti sayılma kriteri (tüm servis)

- [ ] Video dosyası girer → `stream.frame.extracted` çıkar, uçtan uca
- [ ] 6 tool'un tamamı ajan tarafından çağrılabiliyor
- [ ] Event coverage recall ≥ %95, ölçülmüş
- [ ] Adaptif örnekleme, eşit kare bütçesinde sabit örneklemeyi yeniyor — sayıyla
- [ ] Zaman damgası hatası < 1.0 sn
- [ ] Bellek kullanımı video uzunluğundan bağımsız
- [ ] ffmpeg bağımlılığı ve çalıştırma adımları dokümante
- [ ] KPI tablosu gerçek sayılarla dolu

---

*İlgili: #1, #6 · Şartname: 3. Senaryo §3, §4, §7 · Tasarım gerekçeleri #6'da tartışılıyor*
