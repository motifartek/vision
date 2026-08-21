# Stream Servisi — Kurulum ve Çalıştırma

> Durum: video I/O katmanı hazır (probe + decode). Hareket analizi ve örnekleme sırada;
> servis katmanı (MinIO, NATS, araç yüzeyi) sonraki fazlarda.
> Yol haritası: [`documents/architecture/stream-phase-plan.md`](../architecture/stream-phase-plan.md)
> İlgili issue'lar: #1, #6

## Ne yapar

`apps/stream`, yüklenen bir videoyu **güvenlikle ilgili tüm olayları koruyacak
şekilde mümkün olan en az kareye** indirir ve bu yeteneği AI ajanına
çağrılabilir araçlar olarak sunar.

Varlık sebebi VLM'in bağlam penceresidir: 2 dakikalık bir video saniyede bir
kare örneklense 120 kare eder, bu da tek bir istek için fazla. Gerçekçi bütçe
16-64 kare. Yani ~120 adaydan ~32'sini seçmek gerekir — **ve kaza tam da
atlanan karede olmamalıdır.**

## Bağımlılıklar

| Bağımlılık | Sürüm | Neden |
|---|---|---|
| Rust | 1.75+ (2021 edition) | Tüm servis |
| **ffmpeg** | 6.0+ | Video çözme ve kare çıkarma |
| **ffprobe** | ffmpeg ile gelir | Video metadata |

OpenCV **gerekmiyor**. Medya işleri ffmpeg'i alt süreç olarak çalıştırıp ham
piksel tamponlarını boru (pipe) üzerinden okuyarak yapılıyor; piksel matematiği
Rust'ta elle yazılıyor. Bunun bedeli tek bir kurulum adımı, kazancı Windows'ta
OpenCV binding derleme derdinin tamamen ortadan kalkması.

### Rust kurulumu

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Windows için: <https://rustup.rs> üzerinden `rustup-init.exe`.

### ffmpeg kurulumu

```bash
sudo apt install ffmpeg
```

macOS: `brew install ffmpeg`.
Windows: <https://www.gyan.dev/ffmpeg/builds/> adresinden `release-full` paketini
indirip `bin/` klasörünü PATH'e ekleyin.

Doğrulama:

```bash
cargo run -p motif-optics --bin optics -- preflight
```

## Derleme

```bash
cargo build --workspace
```

> **Bilinen sorun:** `apps/gateway`'in `build.rs` dosyası `tonic_build` ile
> `.proto` derliyor ve bu, sistemde `protoc` bulunmasını gerektiriyor. Kurulu
> değilse workspace derlemesi gateway'de durur. İki çözüm var:
>
> 1. `protoc` kur (<https://github.com/protocolbuffers/protobuf/releases>), veya
> 2. gateway'in `Cargo.toml`'una `protoc-bin-vendored` ekleyip `build.rs` içinde
>    `std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);`
>    çağır — böylece sistem kurulumu gerekmez.
>
> İkinci seçenek tercih edilmeli: şartname tekrar üretilebilirliği zorunlu
> tutuyor ve elle kurulum adımı ne kadar azsa o kadar iyi. Gateway #3 kapsamında
> olduğu için karar sahibi @hdenizkaraman.
>
> Bu arada stream tarafı bağımsız derlenebilir:
>
> ```bash
> cargo build -p motif-core -p motif-event-sdk -p motif-optics -p stream
> ```

## Komutlar

```bash
cargo run -p motif-optics --bin optics -- preflight
cargo run -p motif-optics --bin optics -- config
cargo run -p motif-optics --bin optics -- info <video>
cargo run --release -p motif-optics --bin optics -- decode <video>
cargo run -p motif-optics --bin optics -- spawn-cost --samples 15
cargo test --workspace
```

Test videosu üretmek için (üzerinde görünür sayaç ve saat vardır, zaman
damgalarını gözle doğrulamak için birebir):

```bash
ffmpeg -y -f lavfi -i "testsrc2=size=1280x720:rate=30" -t 120 -pix_fmt yuv420p demo.mp4
```

Hareket profili — terminalde eğri, JSON ve SVG çıktısı:

```bash
cargo run --release -p motif-optics --bin optics -- profile <video> --plot --svg p.svg --out p.json
```

Adaptif örnekleme — kare seçimi ve tam kalitede çıkarma:

```bash
cargo run --release -p motif-optics --bin optics -- sample <video> --budget 16 --alpha 0.25 --out kareler/ --overlay
```

Sonraki fazlarda eklenecek komutlar:

```
bench run --dataset <dir> --sweep alpha             # KPI raporu
```

## Örnekleme: α ve kapsama garantisi

Kare seçimi hareket eğrisinin kümülatif toplamı üzerinde **ters dönüşüm
örneklemesi** ile yapılır: N nokta hareket ekseninde eşit aralıklarla
yerleştirilir. Yoğunluk kendiliğinden ayarlanır, elle hiçbir eşik seçilmez.

Ağırlık, hareket dağılımı ile düzgün dağılımın karışımıdır:

```
w[i] = (1 - α) * m[i] / Σm  +  α * (1 / n)
```

α tek başına bir üst sınır garantisi verir. Ardışık iki seçim arasında
kümülatif ağırlık tam olarak `1/N` arttığı ve düzgün bileşen bu aralığa en az
`α * Δ/n` katkı verdiği için:

```
en büyük boşluk ≤ (1 / α) × ortalama aralık
```

α = 0.25 ile hiçbir boşluk ortalama aralığın 4 katını geçemez. Ayrı bir
`max_gap` parametresine gerek yok — α zaten kapsama düğmesidir. Bu özellik
hem birim testle hem gerçek videoyla doğrulanıyor.

> α'nın **sıfır olmaması kritik**: "yerde hareketsiz kişi" şartnamedeki örnek
> olaylardan biri ve tanımı gereği hareketsiz. Saf hareket odaklı örnekleme bu
> olayı yapısal olarak kaçırır; uniform prior bunun sigortasıdır.

### Gürültü tabanı

Sabit kameralı kayıtta sensör gürültüsü her kareye benzer bir hareket ekler.
Düşülmezse taban toplamda gerçek olayı ezer. Gürültülü test videosunda ölçüldü:
14 saniyelik sakin bölüm toplam hareketin üçte ikisini üretiyor, 3 saniyelik
olay üçte birini — ve seçim de o oranda dağılıyor, yani adaptiflik kayboluyor.

Çözüm: hareket skorlarının **medyanını** ağırlıktan düşmek. Medyan tam olarak
gürültü tabanıdır ve videonun kendi dağılımından geldiği için ölçüt hâlâ
uyarlanabilir kalır. `--raw-motion` ile kapatılabilir (benchmark için).

| | Olay penceresine düşen kare |
|---|---|
| Taban düşülmeden | 12 karenin 1'i |
| Taban düşülerek | 14 karenin 10'u |

### Tekrar eleme

Eleme için **iki koşul birden** aranır: parmak izleri yakın **ve** iki kare
arasında biriken hareket ihmal edilebilir.

İkinci koşul zorunlu çıktı. Yalnız parmak izine bakan sürüm ölçüldü ve
zararlıydı: dHash kareyi 9x8'e indirgediği için, büyük ölçekli yerleşimi
benzeyen ama içeriği belirgin biçimde değişen kareler aynı sanıldı — gerçek
test videosunda 14 karelik seçimin 9'u yanlışlıkla elendi ve olay
penceresinden tek kare kaldı.

Hareket eğrisi bu soruyu doğrudan cevaplıyor: iki kare arasında hareket
birikmişse arada bir şey olmuştur, parmak izleri ne kadar benzerse benzesin.

## Ölçümler

2 dakikalık 1280x720 30 fps test videosu, 160x90 gri / 15 fps analiz, release derlemesi:

| Ölçüm | Değer |
|---|---|
| ffprobe metadata | 56 ms |
| ffmpeg süreç açma maliyeti | **20 ms** (15 ölçüm ortalaması) |
| İlk kareye kadar | 80 ms (ffmpeg açılışı dahil) |
| Tam çözme (1800 kare) | 1340 ms |
| Throughput | 1343 kare/sn |
| **Gerçek zaman katı** | **89.5x** |

Hedef pass 1 için ≥50x realtime idi; alt süreç yaklaşımı bunu rahatça karşılıyor.

Hareket profili (çözme + kare farkı + parmak izi + sahne kesiti, tek geçiş):

| Ölçüm | Değer |
|---|---|
| 17 sn sentetik olay videosu | 161 ms |
| **Gerçek zaman katı** | **~105x** |

Sentetik senaryo (8 sn sakin → 3 sn hareket → 6 sn sakin) doğru okunuyor:
hareketin başında ve sonunda birer sahne kesiti, arada hiç.

> **Not:** Ses tarafında (`feature/audio`) ffmpeg alt süreci için ~960 ms
> ölçülmüş ve bu yüzden süreç içi çözmeye (symphonia) geçilmişti. Video
> tarafında aynı ölçüm 20 ms çıkıyor. Fark büyük ihtimalle soğuk/sıcak
> başlangıç: ikili bir kez okunduktan sonra işletim sistemi önbelleğinden
> geliyor. Video için saf Rust bir H.264 çözücü alternatifi zaten yok.

## Crate yapısı

| Crate | Sorumluluk |
|---|---|
| `packages/core` | Paylaşılan tipler, hata yüzeyi, telemetri. Ağ/medya bilmez. |
| `packages/event-sdk` | NATS konuları ve mesaj kontratları. Servisler arası tek doğruluk kaynağı. |
| `packages/optics` | Video I/O, hareket analizi, örnekleme. **Dosya girer, veri çıkar** — ağ bilmez, bu yüzden tek başına test edilebilir. |
| `apps/stream` | Servis katmanı: MinIO, NATS, ajan araç yüzeyi. |
| `tools/bench` | Ölçüm harness'ı, KPI raporu. |

## Yapılandırma

Kare bütçeleri ve analiz çözünürlüğü **çalışma zamanı ayarıdır**, koda
gömülmez — hedef donanım henüz belli değil ve bütçe ona göre değişecek.

| Ayar | Varsayılan | Açıklama |
|---|---|---|
| `analysis_fps` | 15 | Pass 1'de saniyede analiz edilen kare |
| `analysis_width/height` | 160x90 | Analiz karesi çözünürlüğü |
| `budget` | 16 | Genel bakışta modele gidecek kare sayısı |
| `uniform_prior` (α) | 0.2 | Hareket odaklılık ile eşit taramanın dengesi |

> α'nın **sıfır olmaması kritik**: "yerde hareketsiz kişi" şartnamedeki örnek
> olaylardan biri ve tanımı gereği hareketsiz. Saf hareket odaklı örnekleme bu
> olayı yapısal olarak kaçırır; uniform prior bunun sigortasıdır.

---

## Servisi çalıştırma

```bash
cargo run -p stream
```

Hiçbir altyapı gerekmiyor: nesne deposu varsayılan olarak yerel dosya sistemi
(`data/stream`), NATS isteğe bağlı. Ayağa kalkınca <http://localhost:8100>
adresinde **test arayüzü** açılır — video yükle, hareket profilini gör, eğriye
tıklayıp o ana yakınlaş, araçları elle çağır.

## Uçlar

| Uç | İş |
|---|---|
| `GET /` | Test arayüzü (ikiliye gömülü, build adımı yok) |
| `GET /healthz` | Durum, aktif yapılandırma, araç listesi |
| `POST /v1/videos` | Video yükle (multipart, alan adı `file`) |
| `GET /v1/videos` | Yüklenmiş videolar |
| `GET /v1/videos/{id}` | Kayıt + kalan yakınlaştırma bütçesi |
| `DELETE /v1/videos/{id}` | Videoyu ve tüm nesnelerini sil |
| `GET /v1/videos/{id}/profile` | Hareket profili (`?bucket_ms=1000` ile kovalanmış) |
| `GET /v1/videos/{id}/profile.svg` | Hareket eğrisi görseli |
| `POST /v1/videos/{id}/overview` | Genel bakış kareleri seç |
| `POST /v1/tools/{tool}` | **Ajan araç yüzeyi** |
| `GET /v1/blobs/{key}` | Kare/nesne sun |

## Ajan araçları

Altı araç hem HTTP hem NATS istek/cevap (`stream.tool.<ad>`) üzerinden
çağrılabilir; ikisi de aynı gövdeyi kullanır, iş mantığı tek yerdedir.

| Araç | İş |
|---|---|
| `video_info` | Süre, çözünürlük, fps, codec |
| `motion_profile` | Hareket eğrisi (kovalanmış) |
| `sample_overview` | Pass 2 — videonun kaba taraması |
| `zoom_range` | **Pass 3 — ajanın bir aralığa yakınlaşması** |
| `get_frame` | Tek zaman noktasının karesi |
| `crop_region` | Karenin bir bölgesini kırpıp büyüt |

`zoom_range` sistemin ayırt edici parçası: ajan kaba bakışta bir şey fark
edince o aralığın yoğun karelerini kendi kararıyla ister. Video yeniden
çözülmez — profil bir kez çıkarıldığı için yakınlaştırma neredeyse bedava.

Video başına yakınlaştırma sayısı sınırlıdır (`STREAM_MAX_ZOOMS`, varsayılan 8):
ajan yakınlaşmaya kendi karar verdiği için kararsız bir model aynı aralığa
tekrar tekrar girip gecikmeyi sınırsız büyütebilir. Sınıra ulaşınca
`zoom_limit_exceeded` koduyla "eldeki karelerle sonuca varın" mesajı döner.

### Ölçülen davranış

2 dakikalık test videosu, olay 70.0–71.5 sn arasında:

- **Genel bakış** (bütçe 16): 18 karenin 11'i olay penceresine düştü — bütçenin
  %61'i videonun %1.25'ine.
- **`zoom_range(69000, 72000)`**: 14 kare, ortalama 164 ms aralıkla; sahne
  kesiti sınırları işaretli.
- Zaman damgası bindirmesi doğrulandı: t=70600 ms karesinin üzerinde `01:10.6`.

## Ortam değişkenleri

| Değişken | Varsayılan | Açıklama |
|---|---|---|
| `STREAM_BIND` | `0.0.0.0:8100` | Dinlenecek adres |
| `STREAM_STORAGE_ROOT` | `data/stream` | Nesne deposu kökü |
| `NATS_URL` | — | Verilmezse olay yayını ve NATS araç yüzeyi kapalı |
| `STREAM_OVERVIEW_BUDGET` | 16 | Genel bakış kare sayısı |
| `STREAM_ZOOM_BUDGET` | 12 | Yakınlaştırma kare sayısı |
| `STREAM_MAX_ZOOMS` | 8 | Video başına yakınlaştırma sınırı |
| `STREAM_UNIFORM_PRIOR` | 0.25 | α — kapsama garantisi düğmesi |
| `STREAM_ANALYSIS_FPS` | 15 | Pass 1 analiz kare hızı |
| `STREAM_TIMESTAMP_OVERLAY` | açık | Karelere zaman damgası bindir |
| `STREAM_FRAME_MAX_DIM` | 768 | Modele giden karenin uzun kenarı |
| `STREAM_MAX_UPLOAD_BYTES` | 2 GB | Yükleme sınırı |

## Nesne deposu

`BlobStore` arayüzünün arkasında şu an tek gerçekleme var: `LocalStore` (yerel
dosya sistemi). **MinIO/S3 gerçeklemesi henüz yazılmadı** — aynı arayüzün
arkasına girecek ve servisin geri kalanı değişmeyecek. Bu bilinçli bir sıralama:
test arayüzünde gerçek video üzerinde çalışmak için altyapı ayağa kaldırmak
gerekmesin.

Anahtar düzeni:

```
raw/<id>.<uzanti>        ham video
meta/<id>.json           kütük kaydı
profiles/<id>.json       hareket profili (bir kez hesaplanır)
frames/<id>/<t_ms>.jpg   çıkarılmış kareler (sıfır dolgulu, kronolojik sıralanır)
```
