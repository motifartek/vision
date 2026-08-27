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

### Faz 0 — Ölçüm zeminini sağlamlaştır *(yarım gün)*

Karşılaştırma yapacaksak önce terazi düzgün olmalı.

- Harness başarısız analizi **kaydeder**, sessizce düşürmez (bugün `422`/`502`
  satırı tabloya hiç girmiyor).
- Senaryo başına 3 koşu, ortalama ve dağılım raporlanır.
- Çıktı `documents/measurements/` altına commit'lenebilir bir dosya.

*Kabul:* aynı kod iki kez ölçüldüğünde raporlanan recall farkı, faz kabullerinde
kullanılacak eşikten küçük. Yani terazinin kendi gürültüsünü biliyoruz.

---

### Faz 1 — Rapor kaybını sıfırla *(yarım gün)*

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

### Faz 3 — Profili ilk pencere seçimine sok *(1 gün)* ★ tartışmalı olan

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
