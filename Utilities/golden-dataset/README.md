# Golden Dataset — İSG Video Kümesi

> Şartname 3. Senaryo'nun olay listesiyle **birebir hizalanmış** gerçek
> endüstriyel güvenlik kamerası görüntüleri.
> İlgili issue'lar: #5, #1, #8

## Neden bu küme

Şartname ve mentör maili sistemin hangi olayları tespit etmesi gerektiğini
açıkça sayıyor. Genel amaçlı bir video anomali kümesi (UCF-Crime gibi, ki
hırsızlık ve kavga ağırlıklı) bu listeyle örtüşmüyor. Golden dataset ölçtüğümüz
şeyin ta kendisini temsil etmeli.

| Şartnamedeki olay | Kategori | Kaynak etiketi |
|---|---|---|
| Alan ihlali | `alan_ihlali` | Safe Walkway Violation, Unauthorized Intervention, moving in a suspicious manner |
| Uygunsuz ekipman kullanımı | `uygunsuz_ekipman` | Opened Panel Cover, operating heavy equipment dangerously, misusing lift platform |
| İş kazası | `is_kazasi` | falling load, body pulled into machine, foot stuck in conveyor, structural collapse, warehouse shelves toppling, platform failure |
| Düşme | `dusme` | person falling down, hanging from something after slip |
| Forklift kaynaklı riskler | `forklift_riski` | Carrying Overload with Forklift, operating forklift |
| (yangın — İSG kapsamında) | `yangin` | fire incident, extinguishing fire |
| Normal operasyon (yanlış alarm kontrolü) | `normal_operasyon` | Safe Walkway, Authorized Intervention, Closed Panel Cover, Safe Carrying + iSafetyBench rutin eylemler |

Eşleme tablosu `fetch_golden.py` içinde kod olarak duruyor; kaynak etiketi bu
kategorilere düşmüyorsa klip **alınmıyor**.

## Kaynaklar

Her ikisi de **CC BY 4.0** ve gerçek endüstriyel görüntü.

### 1. UnsafeNet — Safe and Unsafe Behaviours

691 klip, 1920×1080, 24 fps, 1–20 sn. **Eskişehir'de bir organize sanayi
bölgesindeki üretim tesisinin güvenlik kameralarından**, şirket ve çalışan
izinleri alınarak Kasım–Aralık 2022'de toplanmış.

Türk fabrikası olması ayrıca değerli: finalde karşılaşacağımız görüntülerle
aynı görsel alan — aynı kamera tipi, aynı yerleşim, aynı iş kıyafetleri.

- Önal, Ö. & Dandıl, E. (2024). *Video dataset for the detection of safe and
  unsafe behaviours in workplaces.* Data in Brief.
  <https://doi.org/10.1016/j.dib.2024.110819>
- Ayna: <https://huggingface.co/datasets/Voxel51/Safe_and_Unsafe_Behaviours>

### 2. iSafetyBench

1100 klip (420 tehlikeli, 680 rutin), 4–8 sn. Fabrika, depo, şantiye, otopark.
67 tehlikeli eylem etiketi, 10 kategori. Video-dil benchmark'ı olarak
tasarlandığı için doğrudan VLM değerlendirmesine uygun (#8).

Her klipte serbest metin `caption` var — Türkçe özet kalitesini değerlendirirken
referans olarak kullanılabilir.

- *iSafetyBench: A video-language benchmark for safety in industrial
  environment.* arXiv:2508.00399
- Veri: <https://github.com/iSafetyBench/data> ·
  <https://huggingface.co/datasets/raiyaanabdullah/isafety-bench>

## Kullanım

```bash
python fetch_golden.py --plan                    # ne inecek, ne kadar yer tutacak
python fetch_golden.py --limit 25                # kategori başına 25 klip
python fetch_golden.py --only dusme,forklift_riski
```

Videolar `videos/` altına iner, `catalog.json` üretilir. Seçim **tohumlu
rastgele** (`--seed`, varsayılan 42): aynı tohum aynı kümeyi verir, ölçümler
tekrar üretilebilir kalır.

Videolar depoya konmuyor — boyut ve telif. Script'in kendisi ve `catalog.json`
depoda; küme her zaman yeniden üretilebilir. Bu, Berat'ın görüntü kümesinde
kullandığı desenin aynısı.

## Zaman damgası: eksik olan parça

Her iki kaynak da **klip seviyesinde** etiketli, kare seviyesinde zaman damgası
vermiyorlar. Bu, endüstriyel güvenlik veri kümelerinde yaygın bir kalıp.

Bizim için önemli, çünkü şartname puanı kritik anın **zaman damgasıyla**
belirtilmesinden geliyor ve `tools/bench` içindeki event coverage recall
ölçütü olayın kaçıncı milisaniyede olduğunu bilmek zorunda.

İyi haber: klipler 4–20 saniye ve zaten davranışı içerecek şekilde kırpılmış.
Kritik anın işaretlenmesi klip başına saniyeler süren bir iş.

`catalog.json` her klip için `event_ms: null` bırakıyor. İşaretleme adımı
tamamlanınca bu alan dolar ve katalog doğrudan `bench run --dataset` girdisi
olur.

### İşaretleme durumu

- [ ] Kritik an işaretleme arayüzü (stream test panosuna eklenecek)
- [ ] 175 klibin işaretlenmesi (ekip işi, bölünebilir)
- [ ] `catalog.json` → `GroundTruth` dönüştürücü

## Lisans ve atıf

Bu dizindeki **script ve katalog** projenin lisansına tabi. **Videolar**
kaynaklarının CC BY 4.0 lisansı altında; kullanan herkes yukarıdaki iki
çalışmaya atıf vermek zorunda.

Şartname açık kaynak paylaşımı ve veri setinin herkese açık indirme
bağlantısını zorunlu tutuyor; ikisi de bu kurulumla karşılanıyor.
