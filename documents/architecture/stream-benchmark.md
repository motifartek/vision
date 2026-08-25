# Stream Ölçüm Raporu

> Şartname §4 katılımcıların kendi metriklerini tanımlamasını ve sonuçları
> raporlarda açıkça sunmasını zorunlu tutuyor. Bu belge o sayıları içerir.
>
> Üreten: `tools/bench` · İlgili issue'lar: #1, #6 · Son güncelleme: 2026-08-20

## Ölçütler

| Ölçüt | Tanım |
|---|---|
| **Event coverage recall** | Ground truth olaylarının yüzde kaçının tolerans içinde seçilmiş bir karesi var |
| **Ortalama sapma** | Kapsanan olaylarda kare ile olay arasındaki uzaklık |
| **Kare azaltma oranı** | Kaynak kare sayısı / seçilen kare sayısı |
| **Yanlış sahne kesiti** | Hiçbir gerçek olaya denk gelmeyen kesit sayısı |
| **Boşluk ihlali** | α'dan türeyen kapsama garantisinin aşıldığı video sayısı |
| **Gerçek zaman katı** | Video süresi / işlem süresi |

**Recall neden ana ölçüt:** *"örnekleme mi kaçırdı, model mi anlamadı"*
sorusunu ayırıyor. Recall düşükse sorun stream tarafındadır ve modele hiç
dokunmadan düzeltilebilir. İki başarısızlığı karıştırmak, kalan sürede yanlış
yeri optimize etmenin en kolay yolu olurdu.

## Veri kümesi

Sentetik, ffmpeg ile üretiliyor: `bench generate`. Senaryoları biz kurduğumuz
için olayların tam olarak kaçıncı milisaniyede olduğunu biliyoruz — etiketleme
hatası sıfır.

Görsel karmaşıklığı taklit etmiyor ve ettiğini de iddia etmiyor. Ölçtüğü tek
şey: **örnekleme, olayın olduğu ana kare ayırıyor mu?** Gerçek İSG görüntüsü
üzerindeki değerlendirme Golden Dataset (#5) ile yapılacak; ground truth biçimi
aynı olduğu için harness değişmeden çalışacak.

| Senaryo | Süre | Olay | Neyi sınıyor |
|---|---|---|---|
| `net-olay` | 17 sn | 2 | Temel durum |
| `hareketsiz-kisi` | 20 sn | 2 | Uzun hareketsizlik; olay hareketin bittiği an |
| `cok-kisa-an` | 20.5 sn | 2 | Yarım saniyelik olay |
| `normal-operasyon` | 18 sn | 0 | Yanlış alarm kontrolü |
| `coklu-olay` | 20 sn | 6 | Bütçenin olaylara dağılımı |
| `agir-gurultu` | 25 sn | 2 | Yoğun sensör gürültüsü |
| `kucuk-nesne-orta` | 20 sn | 2 | 640x360 sahnede 100 px nesne |
| `kucuk-nesne-zor` | 20 sn | 2 | Aynı sahnede 40 px nesne |
| `uzun-tek-olay` | 120 sn | 2 | Bütçe kıtlığı: samanlıkta iğne |
| `uzun-iki-olay` | 120 sn | 4 | Bütçe kıtlığı, iki ayrı olay |

**10 video, 24 olay, ~5 dakika görüntü.**

## Sonuç (varsayılan ayarlar)

`bütçe 16 · α 0.25 · dedup 3 · gürültü tabanı açık · tolerans 1000 ms`

| Ölçüt | Değer |
|---|---|
| **Event coverage recall** | **24/24 — %100** |
| Ortalama sapma | 35 ms |
| Ortalama kare | 15.3 (bütçe 16) |
| Ortalama kare azaltma | 78x (uzun videolarda 200x+) |
| Yanlış sahne kesiti | 1 (10 videoda) |
| Boşluk garantisi ihlali | 0 |
| Ortalama hız | gerçek zamanın 74 katı |
| Toplam işlem süresi | 5.6 sn (5 dakikalık görüntü için) |

## Ablasyon: hangi mekanizma ne kadar katkı veriyor

### Sahne kesiti zorlaması

| Ayar | Recall | Ortalama sapma | Kare |
|---|---|---|---|
| Açık | %100 | 35 ms | 15.3 |
| Kapalı | %100 | 54 ms | 15.8 |

Kesitler recall'u değil **zamansal hassasiyeti** artırıyor. Bütçe kıtken fark
çok daha belirgin (aşağıya bakınız).

### α (uniform prior) — sahne kesitleri kapalıyken

Kesitler açıkken α'nın katkısı görünmez oluyor: adım tipi olaylar zaten zorla
dahil ediliyor. Katkıyı izole etmek için kesitler kapatılarak ölçüldü.

| α | Recall | Ortalama sapma |
|---|---|---|
| 0 | %100 | 35 ms |
| 0.1 | %100 | 43 ms |
| 0.25 | %100 | 54 ms |
| 0.5 | %100 | 81 ms |
| 0.75 | %100 | 115 ms |
| **1.0 (saf düzgün)** | **%88** | **272 ms** |

*(Bu tablo düzeltmelerden sonra yeniden ölçüldü, sonuç değişmedi.)*

**Bu tablo tasarımın gerekçesi.** Saf düzgün dağılım 24 olayın 3'ünü yapısal
olarak kaçırıyor — iki dakikalık videoda 16 kare, 7.5 saniyelik aralık demek ve
1.5 saniyelik olay araya düşüyor. Hareket odaklı örnekleme aynı bütçeyle hepsini
yakalıyor, üstelik kareyi olaya 8 kat daha yakın koyuyor.

α'nın sıfır olmaması ise kapsama garantisi için gerekli: `en büyük boşluk ≤
(1/α) × ortalama aralık`. Ölçümlerde 0 ihlal.

### Kare bütçesi

| Bütçe | Recall (kesit açık) | Sapma (kesit açık) | Sapma (kesit kapalı) | Azaltma |
|---|---|---|---|---|
| 4 | **%96** | 98 ms | 313 ms | 340x |
| 8 | %100 | 82 ms | 142 ms | 163x |
| 12 | %100 | 41 ms | 111 ms | 108x |
| 16 | %100 | 35 ms | 54 ms | 78x |
| 24 | %100 | 26 ms | 21 ms | 54x |
| 32 | %100 | 22 ms | 11 ms | 41x |

Recall bütçeye karşı dayanıklı ve değişen esas olarak **zamansal hassasiyet**.
Şartname puanı zaman damgasından geldiği için asıl bakılması gereken sütun
sapma.

Sahne kesitlerinin değeri burada görünüyor: bütçe 4'te sapmayı 313 ms'den
98 ms'ye indiriyorlar.

**Ama bir ödünleşme var:** kesitler bütçenin yarısını alabildiği için, bütçe
4'te örneklemeye yalnızca 2 nokta kalıyor ve recall %96'ya düşüyor. Kesitler
kapalıyken aynı bütçede %100 çıkıyor. Yani çok kısıtlı bütçede kesit ayırmak
zamansal hassasiyeti üç kat iyileştiriyor ama kapsamadan biraz veriyor. Bütçe
8 ve üstünde ödünleşme kayboluyor; varsayılan 16 bu bölgede.

### Tekrar eleme

| Eşik | Recall | Kare | Sapma |
|---|---|---|---|
| 0 (kapalı) | %100 | 17.3 | 32 ms |
| 1 / 3 / 6 / 12 | %100 | 17.2 | 32 ms |

**Bu veri kümesinde ölçülebilir etkisi yok.** Sentetik videolarda birebir aynı
kare neredeyse hiç yok. Eleme, piksel piksel özdeş kareler içeren videoda
çalıştığı gözlendi (12 kareyi 3'e indirdi) ama sentetik küme bunu temsil
etmiyor. Gerçek sabit kamera görüntüsüyle yeniden değerlendirilmeli (#5).

## Gerçek görüntü üzerinde doğrulama

Sentetik küme algoritmayı ayarlamak için; gerçek görüntü onu **çürütmek** için.
İlk gerçek İSG kaydında (56 sn, 480x480, 30 fps, tek sürekli CCTV çekimi, bir
yükleyicinin işçiye çarpması) iki hata ortaya çıktı. İkisi de sentetik kümede
görünmüyordu.

### 1. Sahte sahne kesitleri

**Bulgu:** Tek çekimlik kayıtta **12 sahne kesiti** işaretlendi. Doğru cevap
sıfır: video baştan sona aynı kamera açısı.

**Sebep:** Kepçe kadrajı süpürdüğünde çok sayıda piksel değişiyor ve SAD
tavana vuruyor. Hareket sıçraması "bir şey oldu" der ama **neyin** olduğunu
ayırt etmez: kadrajı süpüren büyük bir nesne ile sahnenin tamamen değişmesi
aynı sıçramayı üretir.

**Kanıt:** Aday kesitlerin parmak izi mesafeleri ölçüldü — 64 bitte **0-7 bit**.
Gerçek bir kesitte bu 25-32 olurdu (birbiriyle alakasız iki görüntü ortalama
32 bit farklıdır). İçerik perceptual olarak hiç değişmemişti.

**Çözüm:** Kesit için ikinci ve bağımsız bir koşul arandı — parmak izi mesafesi
en az 16 bit. Süpürme geçicidir, kesit kalıcıdır.

**Sonuç:** 12 → **0** kesit. Sentetik kümede yanlış kesit 2 → 1.

### 2. Bütçe aşımı

**Bulgu:** Bütçe 16 istendi, **28 kare** döndü.

**Sebep:** Sahne kesitleri bütçenin *üstüne* ekleniyordu. Sentetik kümede 1-2
kesit olduğu için 16→18 ile fark edilmiyordu; 12 kesitli gerçek kayıtta %75
aşım oldu. Bütçenin var oluş sebebi VLM bağlam sınırı olduğundan bu, bütçeyi
anlamsız kılıyor.

**Çözüm:** Kesitler bütçenin içinden alınıyor ve payları bütçenin yarısıyla
sınırlı. Sınırı aşan durumda en güçlü hareketi taşıyan kesitler seçiliyor —
kesit yalnızca sınır işaretidir, olayın kendisi araya düşer.

**Sonuç:** 28 → **16** kare. Sentetik kümede ortalama kare 16.8 → 15.3,
azaltma oranı 67x → 78x, recall 24/24 (değişmedi).

### Düzeltmelerden sonra ölçülen davranış

| | |
|---|---|
| Sahne kesiti | 0 (doğru) |
| Genel bakış | 16 kare, bütçeye tam uyuyor |
| Kritik an | 14.87s (hareket 1.00), 16.00s, 17.33s — kaza penceresinde 3 kare |
| `zoom_range(13000, 19000)` | 12 kare, ortalama 467 ms aralık |

Yakınlaştırma kazanın seyrini kare kare veriyor: 13.5s'de iki kişi kepçenin
yanında, 14.6s'de yükleyici ilerliyor, 14.8s'de kişi kepçenin altında kalıyor,
14.9s sonrası görünmüyor. **Bu videoya özel hiçbir ayar yapılmadı.**

### Not

Tek video bir doğrulama değil, bir çürütme denemesidir. Bu kayıt iki hatayı
ortaya çıkardı; başka kayıtlar başkalarını çıkaracak. Golden Dataset (#5)
geldiğinde asıl ölçüm o küme üzerinde yapılacak.

## Bilinen sınırlar

- **Sentetik küme gerçek görsel karmaşıklığı temsil etmiyor.** Işık değişimi,
  gölge, kalabalık sahne, kısmi örtüşme yok. Golden Dataset (#5) bunun için.
- **Recall doygun.** Neredeyse tüm ayarlarda %100 çıkıyor; ayrım ancak sahne
  kesitleri kapatılıp α=1 yapıldığında görünüyor. Daha zorlu senaryolar
  (kademeli değişim, örtüşen olaylar, düşük kontrast) eklenmeli.
- **Tekrar eleme ölçülemiyor.** Yukarıya bakınız.
- **Yanlış sahne kesiti sayısı (2) mutlak değer olarak raporlanıyor**, oran
  olarak değil; video sayısı arttıkça normalize edilmeli.

## Yeniden üretme

```bash
cargo run --release -p motif-bench -- generate --out data/fixtures/events
cargo run --release -p motif-bench -- run   --dataset data/fixtures/events
cargo run --release -p motif-bench -- sweep --dataset data/fixtures/events --param alpha --no-scene-cuts
cargo run --release -p motif-bench -- sweep --dataset data/fixtures/events --param budget
```

Video dosyaları depoya konmuyor; `generate` deterministik olduğu için ground
truth JSON'larıyla birlikte her zaman yeniden üretilebilir.
