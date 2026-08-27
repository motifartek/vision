# Arkadaşın istem tasarımı — inceleme ve ölçüm

> `tasarim-notlari.md`, `vlm-system-prompt.md`, `llm-system-prompt.md`
> üzerine · 2026-08-27

Tasarım kaliteli ve gerekçeleri sağlam. Aşağıdakiler onu çürütmüyor; **bu
depoda** karşılığı olmayan varsayımlarını ve ölçülen sonucu ayırıyor.

## Doğrulanan kod iddiaları

| İddia | Durum |
|---|---|
| §1.1 istem "ayrı olaylar olarak işaretle" diyor | **Doğru** |
| §1.2 `RawEvent`'te `event_type` yok | **Doğru** |
| §1.4 `risk_cevir` "kritik"i `Orta`ya düşürüyor | **Doğru** — düzeltildi |
| §1.12 `response_format` / `guided_json` yok | **Doğru** |
| §1.11 iki istem sürümü dolaşımda | **Bayat** — Faz 2'de kapatıldı |

## Ölçüm altyapısı bu depoda yok

Tasarımın en yük taşıyan maddeleri şu dosyalara dayanıyor ve **hiçbiri hiçbir
dalda yok**: `golden/taxonomy.py`, `golden/metrics.py`,
`golden_dataset/DATASET_CARD.md`, `golden_dataset/splits.json`.

Bizdeki golden dataset **10 video**; olay alanları `{t_ms, time, event,
severity}`, üç seviyeli. `event_type`, `time_end`, `temporal_kind` yok.

Dolayısıyla §1.2 (F1 yapısal 0), §1.3 (tIoU), §1.4 (50 video Kritik),
§1.5 (150 videoda boş actions), §1.6 (E kategorisi) sayıları burada
üretilemiyor; §5'teki "dev bölümünde ölç (90 video)" adımı da koşulamıyor.

## Asıl çatışma: olayın birimi

Bizim ground truth'umuz **evreleri ayrı olay olarak** etiketliyor:

```
00:06 Beton döşeme çökmeye başladı
00:07 İskele sistemi yıkıldı ve enkaz oluştu
00:08 Toz bulutu yayıldı
00:12 Çalışanlar kalan platformdan enkaza baktı     ← §1.7'nin "müdahale"si
```

§1.1 bunu kusur sayıp tek kayıt + `oncesi`/`sonrasi` öneriyor. İki yaklaşım da
kendi içinde tutarlı ama **aynı şeyi saymıyorlar**.

### Ölçüldü

Tasarımın davranış kuralları (tek kayıt, müdahale ≠ kaza, yüzey tuzağı, yanlış
alarm önceliği, boş liste meşru) çıktı sözleşmesi sabit tutularak uygulandı ve
10 videoda koşuldu:

| Varyant | Olay | Şema | Süre |
|---|---|---|---|
| gömülü | **36/39** (%92) | 10/10 | 10,3 sn |
| arkadaş kuralları | 30/39 (%76) | 10/10 | 7,1 sn |

Mekanizma görünür: varyant sistematik olarak **daha az olay** bildiriyor
(4→3, 4→1, 5→0). Tek kayıt kuralı tam da tasarlandığı işi yapıyor; kayıp,
ground truth'un evreleri ayrı sayması yüzünden.

**Karar gerekiyor:** ya onların etiketleme sözleşmesi benimsenip 10 video
yeniden etiketlenir ve tarif edilen metrik hattı kurulur, ya da §1.1 ve §1.7
bizim sözleşmemize göre bırakılır. İkisini karıştırmak ölçümü anlamsız kılar.

## Yapılandırılmış çıktı: mekanizma yanlış, bulgu değerli

§1.12 haklı ama önerdiği mekanizma çalışmıyor. Ölçüldü:

| Yöntem | Sonuç |
|---|---|
| `guided_json` (üst düzey) | **sessizce yok sayılıyor**, düz metin döner |
| `extra_body.guided_json` | **sessizce yok sayılıyor** |
| `response_format: json_object` | çalışıyor |
| `response_format: json_schema` | çalışıyor, şema **gerçekten zorlanıyor** |

Zorlama şöyle kanıtlandı: `risk` alanına `["kirmizi","mavi"]` enum'u verildi,
modele kaza anlatıldı; doğal cevabı "Yüksek" olmasına rağmen `"kirmizi"`
yazdı. `$defs` + `$ref` + `anyOf` de geçiyor.

### Ama `anyOf` sistemi kırdı

Şema (rapor **veya** yakınlaştırma) devreye alındığında model **her turda
yakınlaştırma** dalını seçti ve hiç rapor vermedi. Ölçüm 37/39'dan **0/39**'a
düştü. Sebep: kısıtlı kod çözmede dal ilk token'da seçiliyor ve kısa `zoom`
nesnesi cazip hâle geliyor.

Yalnız rapor dalı zorlanınca çalışıyor — ama o zaman yakınlaştırma şemaca
imkânsızlaşıyor ve şartnamenin puanladığı dinamik araç seçimi kayboluyor.
İstemle istenen JSON zaten **10/10 geçerli** çıktığı için şema bugün az şey
kazandırıp çok şey götürüyor: **geri alındı.**

Yeniden değerlendirilecekse karar ayırt edici alanla tek şemaya indirilmeli
(`{"karar": "rapor"|"zoom", …}`); `anyOf` bu modelde güvenilir dal seçimi
vermiyor.

## Uygulanan düzeltmeler

- **"Kritik" artık gizlenmiyor.** `risk_cevir` onu `Orta`ya düşürüyordu — model
  en tehlikeli durumu bildirdiğinde sistem iki seviye **aşağı** çekiyordu.
  Şimdi `Yüksek`e çıkıyor. Teslim biçimi üç seviyeli olduğu için `Kritik` ayrı
  değer olarak taşınamıyor, ama tehlike gizlenmiyor.
- `max_tokens` 2048 → 3072.
- Sıralama isteğe bağlı parçalara açıldı; varyantlar kural ekleyebiliyor.

## Uygulanmayanlar ve sebepleri

| Öneri | Neden bekliyor |
|---|---|
| `tur` (14'lü enum), `bitis`, `zamansal_tip`, `guven`, `cikarimlar` | Ground truth bu alanları taşımıyor; ölçülemez |
| `RiskLevel::Kritik` enum'u | Ground truth üç seviyeli; ölçecek şey yok |
| İki aşamalı VLM → LLM | Depoda tek aşama var; gecikmeyi ikiye katlar, ayrı bir karar |
| Şema zorlaması | Yukarıdaki `anyOf` regresyonu |

## Şemalarda görülen küçük sorunlar

- `^\d{2}:\d{2}$` bir saati aşan kayıtta kırılır; `format_timestamp` orada
  `H:MM:SS` üretiyor.
- `guven` şemada `minimum: 0` ama istem "0,40 altını olay yazma" diyor —
  şema bunu zorlayabilirdi.
- `normal_activity` türü, "boş liste doğru cevaptır" kuralıyla çelişiyor:
  model boş liste yerine bu türde bir olay üretmeye davet ediliyor.
- `olaylar[].siddet` İngilizce (`low|medium|high|critical`), `risk` Türkçe
  dört seviyeli; iki ayrı ölçek.
