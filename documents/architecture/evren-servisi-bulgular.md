# EVREN Çıkarım Servisi — Ölçülmüş Bulgular

> SSB'nin tüm takımlara açtığı çıkarım servisi üzerinde yapılan ilk testler.
> Golden dataset videolarıyla, gerçek isteklerle ölçüldü.
> İlgili issue'lar: #1, #6, #8 · Tarih: 2026-08-24

**Anahtarlar bu depoda tutulmaz.** `EVREN_KEY` ortam değişkeninden okunur.
Depo halka açık ve jüri tarafından izleniyor.

## Servis ne sunuyor

| Model | Ne | Bağlam | Giriş |
|---|---|---|---|
| `vlm` (Qwen3-VL-32B) | Video analizi | 262k | **yalnızca video**, görüntü → HTTP 400 |
| `llm-large` (Qwen3.5-122B-A10B) | Çok kipli | 262k | 1 video **veya en fazla 2 görüntü** |
| `llm-fast` (Qwen3.6-35B-A3B) | Çok kipli, hızlı | 262k | 1 video veya en fazla 2 görüntü |
| `router` (Qwen3-8B) | Metin | 41k | — |
| `guard` (Qwen3Guard-Gen-4B) | İçerik güvenliği | 33k | — |

8 × NVIDIA H200, vLLM, BF16, kuantizasyon yok. Kota yok, takım başına kısıt yok.

Video base64 `data:video/mp4;base64,...` olarak `video_url` alanında gidiyor;
istek gövdesi tavanı 256 MB (base64 şişmesiyle pratikte ~190 MB dosya).
Servis videoyu **kendi örneklüyor: 2 fps, en fazla 520 kare** (= 260 saniye).

## Tasarımı doğrudan etkileyen bulgu

> **Kare kümesi göndermek mümkün değil.** En fazla 2 görüntü kabul ediliyor.

`apps/stream` seçilmiş 16 kareyi JPEG olarak modele veriyordu. Bu platformda
öyle bir teslim yolu yok. Modele zamansal içerik vermenin tek yolu **video
göndermek**.

Bu, örnekleme çalışmasını çürütmüyor ama **çıktısının biçimini değiştiriyor**:
hareket profili artık "hangi kareleri seçeyim" değil, **"hangi saniye aralığını
kesip göndereyim"** sorusunu cevaplıyor.

## Ölçümler

Golden dataset videolarıyla, `vlm` modeline gerçek istekler:

| Video | Süre | Giriş token | Süre |
|---|---|---|---|
| `-NF8DZCdcUQ` | 35 sn | 11.064 | 10.6 sn |
| `yukleyici-isciye-carpma` | 57 sn | 13.353 | 15.9 sn |
| `6ZrAeC5mZ5w` (H.264'e çevrilmiş) | 26 sn | 11.013 | 13.7 sn |
| `yukleyici` 10-22 sn klibi | 12 sn | **3.036** | 10.2 sn |

Kabaca **saniyede ~300 token**. 262k bağlamda bu, token açısından ~14 dakikalık
videoya yer var demek; ama 520 kare tavanı 260 saniyede önce bağlıyor.

### Zaman damgası doğruluğu iyi

`-NF8DZCdcUQ` için model çıktısı ile elle etiketlenmiş ground truth:

| Ground truth | Model |
|---|---|
| 00:12 raf devrilmeye başladı | 00:12 forklift sağdaki rafı çarpar |
| 00:13 malzeme yere döküldü | 00:13 raf çöker, toz bulutu oluşur |
| 00:33 bir çalışan yaklaştı | 00:34 bir çalışan dumanlı alanda ilerler |

Çıktı geçerli JSON, Türkçe, şartnamenin istediği dört anahtarla.

## Üç somut sorun

### 1. AV1 kodlu video servisi kırıyor

`6ZrAeC5mZ5w` HTTP 400 döndürdü. Hata gövdesi `shape=(0, 480, 854, 3)` ve
`frames_indices=[]` gösteriyor: servisin OpenCV arka ucu videodan **tek kare
bile** çıkaramamış. Video AV1 kodluydu; çalışan diğer videolar H.264.

**Aynı video H.264'e çevrilince sorunsuz çalıştı.**

Sonuç: gönderilen her video H.264'e normalize edilmeli. Bu doğrudan
`apps/stream`'in işi ve zorunlu bir adım — final test videolarının kodlaması
bilinmiyor.

```
ffmpeg -i girdi -c:v libx264 -profile:v main -pix_fmt yuv420p ...
```

### 2. Kameranın kendi saati modeli yanıltıyor

Üzerinde zaman damgası basılı CCTV kaydında model geçen süre yerine **duvar
saatini** yazdı:

```
"time": "14:26:11"     ← kameranın damgası
"time": "00:14"        ← şartnamenin istediği
```

Prompt'ta açıkça *"videonun başından itibaren geçen süreyi ver, kameranın
üzerindeki saati kullanma"* denince düzeldi. Prompt'a kalıcı olarak yazılmalı.

### 3. Uzun videoda olay ayrımı bozuluyor

57 saniyelik kayıtta model dört alakasız "olay" ekledi (yükleyicinin rutin
çalışması: 00:20, 00:30, 00:40, 00:50). 12 saniyelik klipte bunlar kayboldu ve
olay listesi temizlendi.

Bu, mailde yazan *"video isteklerinde kısa klip tercih edilmesi"* tavsiyesinin
ölçülmüş karşılığı: kısa klip hem **4.4 kat az token** hem **daha temiz olay
listesi**.

## Açık kalan: şiddet ayrımı

`yukleyici-isciye-carpma`'da model hem tam videoda hem kısa klipte olayı
**"yakın geçiş"** olarak okudu; benim elle çıkardığım ground truth ise
**"yükleyici işçiye çarptı"** diyor.

Hangimizin haklı olduğu net değil. Kare incelemesinde işçi 14.8 sn'de kepçenin
bulunduğu noktada, 14.9 sn'den sonra kadrajda değil — ama yükleyicinin kendisi
de görüşü kapatıyor olabilir. 480x480 çözünürlükte kesin konuşmak zor.

**Ground truth gözden geçirilmeli.** Model ile etiket çeliştiğinde otomatik
olarak etiketin haklı olduğunu varsaymak, ölçümü kendi hatamıza göre ayarlamak
olur.

## Stream servisinin bu platformdaki rolü

Kare seçip göndermek yerine:

1. **Normalize et** — H.264, standart profil. Zorunlu, aksi hâlde istek düşüyor.
2. **Aralık seç** — hareket profili hangi saniyelerin önemli olduğunu söylüyor;
   o pencere kesilip gönderiliyor. Az token, temiz olay listesi.
3. **Yakınlaştır** — ajan bir aralığa odaklanmak istediğinde o aralığın klibi
   üretiliyor.
4. **Parçala** — 260 saniyeyi aşan videolar tek istekte gönderilemiyor.

Yakınlaştırmada dikkat: servis **her zaman 2 fps** örneklüyor, dolayısıyla dar
bir pencere göndermek zamansal çözünürlüğü artırmıyor (2 sn → 4 kare). Daha
yüksek çözünürlük için pencere **ağır çekime alınıyor**.

## Ağır çekim: denendi, çalışıyor — ama bir tuzağı var

35 saniyelik kaydın 12.0–15.0 sn aralığı iki biçimde gönderildi:

| | Süre | Modelin çıktısı |
|---|---|---|
| Gerçek zaman | 3 sn | 4 aşama, her biri tek satır |
| **8× ağır çekim** | 23.7 sn | 4 aşama, **her biri ayrıntılı** |

Ağır çekimde model forkliftin geri kaçışını, tek tek düşen metal parçaları ve
toz yoğunluğunun değişimini ayırt etti. Gerçek zamanlı klipte bunların hiçbiri
yoktu. Yöntem işe yarıyor.

**Tuzak:** model klibin **kendi saatini** raporluyor. Kaynakta 12–15 sn olan
olayı `00:20 – 00:22` diye verdi ve "yaklaşık 22 saniye içinde gerçekleşiyor"
dedi.

Prompt'ta dönüşüm formülü açıkça verildi (*"kaynak_saniye = 12.0 + klip/8"*) —
**düzelmedi**. Model bu aritmetiği güvenilir yapmıyor.

Bu yüzden dönüşüm modele bırakılmıyor: `ClipRef` `t0_ms` ve `time_scale`
taşıyor, `to_source_ms()` ve `rebase_events()` çeviriyi kodda yapıyor.

İkinci gözlem: ağır çekimde model "forklift devrildi" dedi, oysa devrilen raf.
**Detay artarken uydurma riski de artıyor.** Ağır çekim her yere değil, ajanın
özellikle istediği dar pencerelere uygulanmalı.

## Ekibin bilmesi gerekenler

- **Mimari dokümanı Qwen-2VL diyor, platformda Qwen3-VL-32B var.** Güncellenmeli.
- **`rerank` modelinin kartında uyarı var:** geri getirme kalitesini *düşürüyor*
  (R@1 0.95 → 0.55). RAG kuracak olan bilmeli.
- **Şartname "offline, dış API bağımlılığı olmamalı" diyor** ama bu bir hizmet.
  Finalde yerel olarak sağlanacağı varsayılıyor; **mentöre teyit ettirilmeli.**
- Takım başına izole Qdrant örneği de veriliyor (`packages/database`, #2).
