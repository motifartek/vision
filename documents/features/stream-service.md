# Stream Servisi — Kurulum ve Çalıştırma

> Durum: **Faz 0** (iskelet). Boru hattı Faz 1-3'te, servis katmanı Faz 6-7'de gelecek.
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
cargo test --workspace
```

Faz 1-4 ilerledikçe eklenecek komutlar:

```
optics info    <video>                              # metadata           (Faz 1)
optics decode  <video> --limit N --timing           # çözme throughput'u (Faz 1)
optics profile <video> --out p.json --svg p.svg     # hareket eğrisi     (Faz 2)
optics sample  <video> --budget 16 --alpha 0.2      # örnekleme          (Faz 3)
bench run --dataset <dir> --sweep alpha             # KPI raporu         (Faz 4)
```

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
