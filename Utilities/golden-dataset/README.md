# Golden Dataset — Olay Anlatısı Olan İSG Videoları

> Tek tek küratörlükle büyür. Her video insan onayından geçer.
> İlgili issue'lar: #5, #1, #8

## Aradığımız şey nedir

Şartname senaryoyu bir örnekle tanımlıyor ve o örnek her şeyi söylüyor:

```
00:15 — Forklift devrilmesi
00:20 — Yerde hareketsiz kişi
00:35 — Personel toplanması

Sonuç: Olası iş kazası · Yüksek yaralanma riski
Öneriler: sağlık ekibi, güvenlik, kayıt
```

Bu bir **sınıflandırma** değil, bir **anlatı**. Tek videoda, birbirine nedensel
olarak bağlı, farklı zamanlarda üç olay: kaza olur, sonucu görülür, müdahale
gelir. §4 bunu açıkça istiyor: *"olayların başlangıç, gelişim ve sonuç
süreçlerini ayırt edebilmelidir."*

Forklift devrilmesi sadece bir örnek. Genel mantık şu:

> **Bir şey ters gider → bir sonucu olur → bir tepki verilir.**

Vinç yükü düşürür, altındaki kişi kaçar, çevredekiler koşar. İşçi konveyöre
sıkışır, iş durur, arkadaşları müdahale eder. Yükleyici işçiye çarpar, kişi
yerde kalır, kalabalık toplanır. Hepsi aynı yapı.

## Kabul ölçütleri

Bir videonun kümeye girebilmesi için:

- [ ] **Kırpılmamış.** Olayın öncesi ve sonrası kadrajda olmalı. Sadece çarpma
      anını gösteren 5 saniyelik klip **kabul edilmez** — yayın ortası, yayın
      kendisi değil.
- [ ] **30 saniye – 3 dakika.** Altında yay oluşmuyor, üstünde etiketleme
      pahalılaşıyor ve final test videoları "kısa" olacak.
- [ ] **En az iki ayrı zaman damgalı olay.** Tek olaylı video sınıflandırma
      örneğidir, anlatı değil.
- [ ] **Gerçek İSG bağlamı.** Fabrika, depo, şantiye, saha operasyonu, liman,
      tesis. Trafik kazası ya da genel güvenlik kamerası değil.
- [ ] **Sabit kamera tercih edilir.** Final testinde gözetim kamerası görüntüsü
      bekleniyor; el kamerası ve sallantılı çekim farklı bir problem.
- [ ] **Tek sürekli çekim.** Kurgulanmış derleme videosu değil. Sahne kesiti
      içeren montajlar ayrı bir şey ölçer.

Şüphedeysen sor: *"bu videodan şartnamedeki gibi üç satırlık bir zaman
çizelgesi çıkarabilir miyim?"* Çıkaramıyorsan video uygun değil.

## Neden hazır veri kümesi kullanmıyoruz

Denendi ve ölçüldü; kaydı burada duruyor çünkü şartname "karşılaşılan zorluklar
ve getirilen çözümler" bölümü istiyor.

| Aday | Neden olmadı |
|---|---|
| **UnsafeNet** (Eskişehir fabrikası, 691 klip, CC BY 4.0) | Sınıflar uygunluk denetimi: "Safe Walkway Violation" birinin boyalı çizgi dışında yürümesi, "Opened Panel Cover" panel kapağının açık kalması. Kaza değil, sonucu yok, müdahalesi yok. Üstelik sabit bir poligon kuralının çözdüğü şey — §4'ün cezalandırdığı tür. |
| **iSafetyBench** (1100 klip, CC BY 4.0) | İçerik doğru (`crushed under overturned vehicle`, `person falling down` + `rescue effort`), ama klipler 4–8 sn ve kırpılmış. Ölçüldü: 40 klipte ortanca 7 sn, en uzun 14 sn, **30 sn üstü sıfır**. Şartnamenin 35 saniyelik yayını taşıyamıyor. |
| **UCF-Crime** (1900 kırpılmamış video) | Kırpılmamış olması doğru, ama hırsızlık/kavga/soygun ağırlıklı. İSG bağlamı yok. |
| **Le2i / UR Fall** | Gerçek zamansal etiket var ama sahnelenmiş düşmeler, ev ve ofis ortamında. Endüstriyel değil. |

Ortak sorun: **şartnamenin istediği şekle sahip yayınlanmış bir benchmark yok.**
Kırpılmamış + İSG + çoklu zaman damgalı olay anlatısı, yarışmanın kendi
tanımladığı bir görev. Bu yüzden küme elle kürate ediliyor.

## Nasıl büyür

Her video tek tek eklenir ve insan onayından geçer.

```bash
# 1. Videoyu indir/kaydet, gözle kontrol et (yukarıdaki ölçütler)
# 2. Kataloğa ekle
python add_video.py "C:/indirilenler/forklift_devrilmesi.mp4" \
    --kaynak "https://..." \
    --not "yukleyici isciye carpiyor, kalabalik topluyor"

# 3. Zaman çizelgesini işaretle
python -m http.server 8200
#    -> localhost:8200/annotate.html
```

`annotate.html` şartnamenin çıktı biçiminin aynısını üretiyor: zaman damgalı
olay listesi, genel özet, risk seviyesi, aksiyon önerileri. Yani **ground truth
= sistemin üretmesi gereken ideal çıktı.** Bu sayede aynı dosya hem benim
event coverage recall ölçümüme hem de VLM çıktı karşılaştırmasına (#8) girdi
oluyor.

## Dosyalar

| | |
|---|---|
| `catalog.json` | Küme. Depoda tutuluyor, videolar tutulmuyor. |
| `videos/` | Ham videolar. `.gitignore`'da — boyut ve telif. |
| `add_video.py` | Kataloğa tek video ekler, metadata'yı ffprobe ile çıkarır. |
| `annotate.html` | Zaman çizelgesi işaretleme arayüzü. |

## Telif

Her kaydın `source_url` ve `license` alanı var. Videolar depoya konmuyor;
katalog kaynağı gösteriyor. Şartname veri setinin herkese açık indirilebilir
olmasını istiyor — kaynak bağlantıları bunu karşılıyor.

Yayın hakkı belirsiz bir video kümeye **eklenmez**. Şüpheli durumda kaydın
`license` alanına ne bilindiği yazılır, boş bırakılmaz.
