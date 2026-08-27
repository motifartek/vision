# Stream — adaptif akış planı

Dal: `stream-adaptif` (`develop`'tan) · İlgili: #1, #6 · Şartname: 3. Senaryo §3, §4

Bu plan `apps/stream` denetiminden çıktı. Denetimin bulduğu şey şuydu: **stream'de
yazılan her şey çalışıyor, ama bir kısmı üretim akışının dışında kalmış.** Plan o
farkı kapatmayı değil, *ölçülebilir fayda getiren* kısmını devreye almayı hedefliyor.

## 1. Ölçülen durum

Hepsi canlı sistemde, çalışan konteynerler üzerinde ölçüldü.

### Çalışanlar

Yedi aracın hepsi çağrılabiliyor. Klip yolu doğru: ağır çekim uygulanıyor
(3 sn aralık 4× ölçekle 11967 ms klip), kare bütçeleri tam tutuyor (24/40/20
istendi, 24/40/20 geldi), aralık taşması kaynağın sonuna kırpılıyor,
yakınlaştırma limiti `429` ile uygulanıyor. Alımda codec normalizasyonu ve
durağan görüntü savunması çalışıyor.

### Yakınlaştırma: sanılandan farklı

İlk gözlem "ajan hiç yakınlaştırmıyor" idi — üç gerçek kaza videosunda 0/3.
Sentetik kümede ise **tetikleniyor**. Yani model belirsizlikte yakınlaşıyor,
net sahnede tek turda karar veriyor. Sorun tetiklenmemesi değil, tetiklendiğinde
ne olduğu.

### Rapor kaybı — en ağır bulgu

`uzun-iki-olay` (120 sn, 4 olay) senaryosunda boru hattı **hiçbir şey**
üretmiyor:

```
{"code":"no_report","error":"model 2 yakınlaştırmadan sonra da rapor vermedi"}
HTTP 422
```

Bozuk rapor değil, eksik rapor değil — rapor yok. Yarışmada yüklenen bir videonun
boş dönmesi doğrudan sıfır demek.

**Kök neden bulundu ve prompt'ta.** Yakınlaştırma istemi şunu diyor:

> Bu aralıkta tam olarak ne olduğunu belirle ve raporu ver. Artık yakınlaştırma isteme.

Ama *aynı istemin* içindeki çıktı sözleşmesi hâlâ zoom seçeneğini sunuyor:

```
Emin değilsen ve bir aralığa yakından bakman gerekiyorsa:
{"zoom": {"t0_ms": <başlangıç>, "t1_ms": <bitiş>}}
```

İstem kendisiyle çelişiyor ve model şemayı takip ediyor. Talimatla yasaklayıp
şemayla izin vermek işe yaramıyor.

### Dejenere yakınlaştırma

25 saniyelik `agir-gurultu` kaydında model `zoom_range(0, 25000)` çağırdı —
videonun tamamı. Yakınlaştırma bütçesinden bir hak harcandı, üretilen klip
öncekiyle neredeyse aynı, kazanç sıfır.

### Geçici hatada kayıp

Aynı senaryo bir koşuda `502 Bad Gateway` ile düştü, tekrarında `200` verdi.
EVREN tarafında geçici bir aksama tüm analizi kaybettiriyor; yeniden deneme yok.

### Hareket profili üretimde yok

41 videodan 38'inde klip var, 19'unda profil — o 19'u da eski test
çağrılarından. Üretim akışı (`video_info` → `clip_range` → `zoom_range`)
`pipeline::profile`'a hiç uğramıyor.

Somut örnek, *Marbal tiles factory accident* (22,7 sn):

| saniye | hareket | model olayı |
|---|---|---|
| 04 | **1.00** (videodaki en yoğun an) | — |
| 08 | 0.66 | ✔ |
| 10–11 | 0.99 / **1.00** | ✔ |
| 16 | 0.98 | ✔ (15'te) |

Model 4. saniyeyi atladı. Bu bir kayıp *olabilir*; olmayabilir de — yüksek
hareket olay demek değil.

### Yanlış pozitif

`normal-operasyon` senaryosunda (ground truth: **0 olay**) model **9 olay**
üretti. Sentetik kümenin tamamında 24 olay üretildi, 6'sı tuttu.

## 2. Ölçüm zemini — ve sınırı

`datasets/sentetik` 10 senaryo, 24 olay, ffmpeg ile üretildiği için etiket
hatası sıfır. `tools/bench` bunu ±3 sn toleransla eşleştiriyor.

**Bu kümeyi neyin ölçüsü saymadığımız önemli.** Kendi dokümanı açıkça yazıyor:
sentetik küme görsel karmaşıklığı taklit etmiyor, ölçtüğü tek şey *örneklemenin
olay anına kare ayırıp ayırmadığı*. Nitekim model `agir-gurultu` kaydını "kamera
sinyal arızası" diye okudu — semantik olarak makul, ground truth'a göre yanlış.

Dolayısıyla buradan çıkan **mutlak** recall (bugün %43) uçtan uca başarı ölçüsü
değil. Ama **göreli** karşılaştırma için geçerli: aynı küme, aynı model, tek
değişen bizim stream tarafındaki değişikliğimiz.

Bir eksik daha var ve düzeltilmeden ilerlenmemeli: yukarıdaki sayılar
**senaryo başına tek koşu**. Model çıktısı koşudan koşuya oynuyor; tek koşuya
dayalı karşılaştırma gürültüyü fayda sanmaya açık.

## 3. Kararlar

**K1 — Rapor asla kaybolmaz.** Bir analiz her koşulda çıktı üretmeli. Model
yakınlaştırma ısrarındaysa bile elde olan klipten rapor istenir.

**K2 — Yasak şemaya girer, talimata değil.** Bir şeyi yapmasını istemiyorsak
çıktı sözleşmesinden çıkarırız. Talimatla yasaklayıp şemayla sunmak ölçüldü,
çalışmıyor.

**K3 — Hareket profili talimat değil, ipucu.** Şartname statik kural tabanlı
çözümleri dışlıyor. Profil "şuraya bak" diye zorlamaz; pencere seçimine girdi
olur, karar modelde kalır.

**K4 — Ölçülmeden kalıcı olmaz.** Her faz sentetik kümede en az üç koşuluk
ortalamayla, öncesi/sonrası karşılaştırılır. Fark gürültü içindeyse değişiklik
geri alınır.

**K5 — Kapsam dışı.** `stream.frame.extracted` klip mimarisine ters; ölü olay
olarak bırakılıyor. Kliplere zaman damgası bindirmesi istemle çelişiyor
("kameranın bastığı saati kullanma"), yapılmıyor.

## 4. Faz planı

Sıra değer/risk oranına göre: önce bedelsiz kazançlar, sonra ölçüm gerektiren
tartışmalı kısım.

---

### Faz 0 — Ölçüm zeminini sağlamlaştır *(bitti)*

Karşılaştırma yapacaksak önce terazi düzgün olmalı.

**Bir varsayım yanlış çıktı.** Plan "harness başarısız analizi sessizce
düşürüyor" diyordu; kod öyle yapmıyor — `bos_sonuc` hatayı kaydediyor, paydayı
koruyor, sıfır eşleşme sayıyor. Düşüren geçici ölçüm scriptiydi. Harness bu
konuda baştan doğruymuş.

**Terazi gerçekten eğriydi, ama başka yerden.** Harness videoyu koşular
arasında yeniden kullanıyordu. Yakınlaştırma bütçesi (`max_zooms_per_video`)
video başına tutulduğu için birikiyordu: birinci koşu 8 hakla başlıyor, üçüncü
koşu `429 zoom_limit_exceeded` alıyor.

Bu iki yönden bozuyordu: sonraki koşum sistematik olarak dezavantajlı başlıyor,
ve iki varyant sırayla ölçüldüğünde **ikincisi haksız yere kötü görünüyor**.
Faz 3'te profil varyantını bu terazide ölçseydik sonuç kendiliğinden aleyhine
çıkardı. Düzeltildi: tekrarlı koşumda her koşu taze video yükleyip ölçüm
sonrası siliyor.

Eklenenler: `--tekrar N`, yayılım/hata/şema sütunları, gürültü bandı hesabı
(fark bandı aşmıyorsa çıktı `ANLAMLI DEĞİL` yazıyor), video kararlılık ayrımı,
`--rapor` ile commit'lenebilir markdown.

#### Ölçülen taban

`documents/measurements/stream-taban-olcumu.md` — 10 video, 3 bağımsız koşu:

| | |
|---|---|
| olay eşleşmesi | 7,7/24 → **%32** (koşular 6–11) |
| **rapor üretemeyen** | **10/30** koşum-video |
| şema geçerli | 20/30 |
| gürültü bandı | **5 olay** |

*Kabul karşılandı:* terazinin kendi gürültüsünü biliyoruz — ve sandığımızdan
büyük.

#### Gürültü bandı tek bir videodan geliyor

Koşu kırılımı bakılmadan "gürültü 5" demek yanıltıcı olurdu:

| video | koşu 1 | koşu 2 | koşu 3 | durum |
|---|---|---|---|---|
| `coklu-olay` | 1/6 | 1/6 | **6/6** | savruluyor |
| `hareketsiz-kisi` | 1/2 | 1/2 | 1/2 | kararlı |
| `kucuk-nesne-orta` | 2/2 | 2/2 | 2/2 | kararlı |
| `uzun-tek-olay` | 2/2 | 2/2 | 2/2 | kararlı |
| `normal-operasyon` | 0/0 | 0/0 | 0/0 | kararlı |
| `net-olay` | 0/2 | 0/2 | hata | — |
| `agir-gurultu` | hata | 0/2 | 0/2 | — |
| `uzun-iki-olay` | 0/4 | hata | hata | — |
| `cok-kisa-an` | hata | hata | hata | hep düşüyor |
| `kucuk-nesne-zor` | hata | hata | hata | hep düşüyor |

Beş olayın **tamamı** `coklu-olay`'ın 1↔6 savrulmasından. Geri kalan küme ya
taş gibi kararlı ya da hata veriyor.

#### Bunun plana etkisi

**Toplam recall üzerinden A/B yapmak bu kümede işe yaramaz.** 24 olaylık kümede
5 olayın altındaki fark iddia edilemez; bu 21 puanlık bir eşik demek ve Faz 3'ün
makul bir kazancı bunun çok altında kalır.

Üç sonuç:

1. **Faz 1 zaten ölçülebilir** — 10/30 hatayı 0/30'a indirmek bir sayım, recall
   farkı değil. Gürültüden etkilenmiyor.
2. **Faz 3 için karşılaştırma yöntemi değişmeli.** Toplam yerine **video bazlı
   eşleştirilmiş** karşılaştırma gerekiyor: aynı video, iki varyant, kim daha
   iyi. Kararlı 4 video zaten sağlam taban.
3. **`coklu-olay` ayrı ele alınmalı**, ortalamaya karışmamalı.

Faz 1'den önce yapılacak ek iş yok; Faz 3'e gelindiğinde eşleştirilmiş
karşılaştırma harness'a eklenecek.

---

### Faz 1 — Rapor kaybını sıfırla *(bitti)*

*Kabul karşılandı ve fazlasıyla.* Ölçüm:
`documents/measurements/stream-faz1-olcumu.md`

| | taban | Faz 1 |
|---|---|---|
| olay eşleşmesi | 7,7/24 (%32) | **14,7/24 (%61)** |
| rapor üretemeyen | 10/30 | **0/30** |
| şema geçerli | 20/30 | **30/30** |
| boş aksiyon | 10 | **0** |
| gürültü bandı | 5 olay | **1 olay** |
| kararlı video | 4/10 | **9/10** |

Kazanç (+7 olay) hem eski hem yeni gürültü bandının çok üstünde; bu fark
iddia edilebilir.

**Asıl neden şema değişikliği.** Hata sınıflarına ayırınca net görünüyor:

| hata sınıfı | taban | Faz 1 (ara) | Faz 1 (son) |
|---|---|---|---|
| `model rapor vermedi` | 8 | **0** | **0** |
| `stream ... döndü` | 1 | 7 | **0** |

Zoom dalını son turun şemasından çıkarmak `no_report` sınıfını **tamamen**
kaldırdı. Talimatla yasaklamak çalışmıyordu, şemadan çıkarmak çalıştı.

**Gürültünün kendisi arızadan geliyormuş.** Faz 0'da bandın tamamını açan
`coklu-olay` (1/6, 1/6, 6/6) artık **6/6, 6/6, 6/6**. Yani o savrulma modelin
rastgeleliği değil, yakınlaştırma döngüsünün kararsızlığıydı. Band 5'ten 1'e
düştü — Faz 3'ün ölçülebilmesi için gereken hassasiyet böylece kazanıldı.

**Yeniden deneme ölçülebilir iş yapıyor.** Faz 0'ın "her koşuda taze video"
değişikliği dosya trafiğini artırdı ve `data/stream` OneDrive altından Docker'a
bind mount edildiği için geçici `500 G/Ç hatası` üretmeye başladı. Ara ölçümde
7 analiz bu yüzden kaybolmuştu.

Son koşuda `stream` günlüğünde **24** adet 500 var, harness'ta **0** hata:
tekrar denemeler hepsini soğurdu. (Bench tracing kurmadığı için uyarı satırları
görünmüyor; ilk bakışta "tetiklenmedi" sanmıştım — günlük sayımı tersini
gösterdi.)

**Yer tutucu rapor hiç devreye girmedi.** Kök neden düzeltildiği için ağa gerek
kalmadı; yine de kuyruk durumlar için duruyor.

#### Kalan zayıf noktalar — artık kararlı

Bunlar boru hattı arızası değil, modelin kaçırması. Kararlı oldukları için
Faz 3'te ölçülebilirler:

| video | sonuç | not |
|---|---|---|
| `uzun-iki-olay` | 0/4 ×3 | 120 sn, tek klipte 240 kare — Faz 3'ün asıl hedefi |
| `net-olay` | 0/2 ×3 | 17 sn, belirgin olay |
| `cok-kisa-an` | 0/2 ×3 | yarım saniyelik olay |
| `uzun-tek-olay` | 1/2 ×3 | 120 sn |

---

### Faz 1 — özgün plan *(referans)*

En yüksek getirili faz ve **hiçbir dezavantajı yok**: bugün sıfır dönen
senaryolardan alacağımız her şey net kazanç.

İki değişiklik:

1. **Son turda şema zoom sunmaz.** `PromptKind` için "son tur" varyantı: çıktı
   sözleşmesi yalnız rapor dalını içerir (K2).
2. **Yine de rapor gelmezse boş dönme.** Elde olan son klipten rapor istenir;
   o da olmazsa en azından `summary` + `risk: Düşük` + boş `events` ile
   şartname biçiminde bir çıktı üretilir ve bunun düşük güvenli olduğu
   `AgentStep`'e yazılır.

Ayrıca geçici HTTP hatalarında (5xx, zaman aşımı) **bir kez** yeniden deneme.

*Kabul:* sentetik kümedeki 10 senaryonun **10'u** rapor üretir. Bugün 8/10.
Şema doğruluğu bozulmaz. Recall düşmez.

*Risk:* zorlanan rapor kalitesiz olabilir. Ama boş cevaptan kötü değil —
şartname puanlamasında sıfırın altı yok.

---

### Faz 2 — Dejenere yakınlaştırmayı engelle *(2 saat)*

Model tüm videoya yakınlaşmak isterse bu bir yakınlaştırma değil; bütçeyi
harcamamalı.

- İstenen aralık kaynağın **%80'inden geniş** ise: bütçe tüketilmez, ajana
  "bu zaten elindeki görüntü, daralt ya da raporla" bilgisi döner.
- Aynı aralık ikinci kez isteniyorsa aynı şekilde reddedilir.

*Kabul:* `agir-gurultu` senaryosunda `zoom_range(0,25000)` bütçe tüketmiyor;
sentetik kümede toplam yakınlaştırma sayısı düşerken recall düşmüyor.

*Risk:* eşik keyfi. %80 ölçümle ayarlanacak, sabit varsayılmayacak.

---

### Faz 3 — Profili ilk pencere seçimine sok *(ölçüldü, geri alındı)*

*Kabul karşılanmadı.* Ölçüm:
`documents/measurements/stream-faz3-hareket-ipucu.md`

Uygulanan varyant: uzun kayıtlarda (≥60 sn) hareket profilinin en yoğun
anları son eke **ipucu** olarak ekleniyor; karar modelde kalıyor (§K3).
Varyantı katalog sürüyor — `hareket_ipucu` parçası gömülü katalogda yok,
dolayısıyla taban koşu profil maliyetini hiç ödemiyor.

| varyant | olay | yayılım | süre |
|---|---|---|---|
| `gomulu` | 0,7/6 | 0–1 | 56,8 sn |
| `hareket` | 0,7/6 | 0–2 | **85,7 sn** |

**Fark +0,0 olay** — gürültü bandının (2) içinde, anlamlı değil. Bedeli ise
kesin: profil hesabı çözümlemeyi **%51 yavaşlatıyor**.

#### İpucu doğruydu; sorun orada değildi

Bu fazın en değerli çıktısı bu. İpucunun gösterdiği anlar ground truth ile
karşılaştırıldı:

| video | gerçek olaylar | ipucunun verdiği |
|---|---|---|
| `uzun-tek-olay` | 01:10, 01:11 | 00:16, 00:33, 00:49, **01:10**, 01:59 |
| `uzun-iki-olay` | 00:25, 00:26, 01:26, 01:27 | 00:16, **00:25**, 00:33, 00:49, **01:26** |

Hareket profili **her iki olayı da buldu**. Yani modele tam olarak nereye
bakacağı söylendi — ve altı koşunun altısında da `uzun-iki-olay`'da 0/4 aldı.

Sonuç: **darboğaz örnekleme ya da dikkat yönlendirme değil, semantik tanıma.**
Model bu sahnelerde olayı "olay" olarak tanımıyor; nereye bakacağını bilmemesi
sorun değildi. Bu, sentetik kümenin kendi dokümanının söylediğiyle örtüşüyor:
küme örneklemeyi ölçüyor, anlamayı değil.

İkincil gözlem: ipucu modeli **daha muhafazakâr** yaptı — koşu başına üretilen
olay 6,0'dan 2,0'a düştü. Recall değişmediğine göre bu, doğru olayları da
bastırmış olabilir.

#### Neden kod geri alındı

Plan §K4 bunu önceden bağlamıştı: *"Fark gürültü içindeyse değişiklik geri
alınır."* Ayrıca denetimin kendi bulgusu, kullanılmayan makinenin yük olduğuydu;
ölçülüp faydasız çıkan bir yolu depoda bırakmak aynı hatayı tekrarlamak olurdu.

Geri alınan: `PromptContext.hareket`, `ClipSource::motion_buckets`, tepe seçimi
ve ajan bağlantısı, varyant katalogu. Kalan: bu bulgu ve ölçüm dosyası.

#### Ölçümün sınırı — dürüstlük payı

Bu test **zayıf**: 2 video, 3 koşu, gürültü bandı 2. Sentetik sahnelerde model
zaten olayı tanımadığı için, ipucunun gerçek kayıtlarda fayda sağlayıp
sağlamayacağı bu kümeyle **ölçülemez**. Fikir çürütülmedi; bu veriyle
doğrulanamadı.

Yeniden denenecekse gereken: gerçek İSG videolarından, olayları etiketlenmiş
bir küme (Golden Dataset, #5). Sentetik kümeyle tekrar denemenin bilgi değeri
yok.

---

### Faz 3 — özgün plan *(referans)*

Klip mimarisinde profilin **tek gerçek kaldıracı** bu: kare seçmek değil,
*hangi aralığı keseceğine* girdi olmak.

Bugün ilk bakış her zaman `clip_range(0, duration)` — videonun tamamı, tekdüze
2 fps. 120 saniyelik kayıtta bu tek klipte **240 kare** demek. Profil burada
"kaydın en yoğun N saniyesi şurası" diyebilir.

Uygulama, K3'e sadık kalarak:

- Yalnızca **uzun** kayıtlarda devreye girer (eşik ölçümle; 60 sn üzeri aday).
  Kısa videoda tamamını göndermek zaten doğru.
- Profil bir **ön ek klibi seçmez**; ilk bakış yine tam kayıt olur. Değişen şey
  son ekte modele verilen ipucu: *"hareket şu aralıklarda yoğunlaşıyor: …"*.
  Karar modelde kalır (K3).
- İkinci varyant olarak, ilk turda tam kayıt yerine profilin işaret ettiği
  pencerelerin klibi denenir ve A/B karşılaştırılır.

*Kabul:* sentetik kümede — özellikle iki adet 120 sn'lik senaryoda — recall
**artar**, yanlış pozitif artmaz. Artmıyorsa faz geri alınır ve bu dokümana
"ölçüldü, fayda yok" diye yazılır.

*Dezavantajı açıkça:* profil hesabı alım süresine ek yük getirir (video bir kez
daha çözülür). Ayrıca yüksek hareket ≠ olay; model yanlış yere
yönlendirilebilir. Bu yüzden ipucu biçiminde ve ölçüme bağlı.

---

### Faz 4 — Yanlış pozitif *(yarım gün, koşullu)*

`normal-operasyon` senaryosunda 0 olaya karşı 9 olay üretildi. Bu stream değil
istem tarafı, ama ölçüsü aynı harness'ta.

Faz 1–3'ten sonra hâlâ duruyorsa ele alınır; `olay_olmayan` parçası
`prompt-system` altyapısıyla varyant olarak ölçülür.

*Kabul:* olaysız senaryoda üretilen olay sayısı düşer, gerçek olaylı
senaryolarda recall düşmez.

---

## 5. Yapmayacaklarımız

Denetimde çıkan ama bilinçli olarak bırakılanlar:

- **`stream.frame.extracted`** — klip mimarisine ters (K5). Sözleşmede ölü
  duruyor; tüketicisi yok, zarar vermiyor.
- **Kliplere zaman damgası** — istemle çelişiyor (K5). Zaman kararlılığı zaten
  ölçüldü: aynı video iki koşuda birebir aynı zamanları verdi.
- **Sahne kesme eşiği** — 19 profilde 9065 örnekte 4 kesme. Sabit kameralı
  güvenlik kaydında kesme olmaması normal; benchmark kazancı 19 ms.
- **`crop_region`'ın skor tutarsızlığı** — gerçek ama aracı kimse çağırmıyor.
- **Bellek iddiası** — kare tamponu sabit, örnek vektörü ~5 KB/sn. "Bağımsız"
  değil "sınırlı ve doğrusal" demek yeterli; kod değişmiyor.
