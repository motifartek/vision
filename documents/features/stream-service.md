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

Sonraki fazlarda eklenecek komutlar:

```
optics sample <video> --budget 16 --alpha 0.2       # adaptif örnekleme
bench run --dataset <dir> --sweep alpha             # KPI raporu
```

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
