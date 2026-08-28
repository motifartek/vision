# Prompt ölçümü

10 video · 3 koşu · tolerans ±3 sn

| varyant | olay (ort/toplam) | yayılım | hata | model olayı | sapma | süre |
|---|---|---|---|---|---|---|
| `gomulu` | 14.3/24 | 12–17 | 0/30 | 37.3 | 513 ms | 38.6 sn |
| `olaysiz` | 1.3/24 | 0–2 | 0/30 | 2.7 | 250 ms | 25.5 sn |

## Terazi gürültüsü

Bir varyantın kendi koşumları arasında en fazla **5 olay** oynadı. Bir değişikliğin etkisi ancak bu yayılımı aşarsa iddia edilebilir; bandın içindeki fark ölçüm gürültüsüdür.

## Video kararlılığı

Koşumlar arasında sonucu değişmeyen videolar karşılaştırmada güvenilir taban; değişenler bandı tek başına açabiliyor.

**`gomulu`**

- kararlı (7): agir-gurultu, hareketsiz-kisi, kucuk-nesne-orta, kucuk-nesne-zor, net-olay, normal-operasyon, uzun-iki-olay
- oynak (3): cok-kisa-an (0–2), coklu-olay (3–6), uzun-tek-olay (0–1)
- her koşuda hata (0): yok

**`olaysiz`**

- kararlı (8): agir-gurultu, cok-kisa-an, coklu-olay, kucuk-nesne-orta, kucuk-nesne-zor, net-olay, normal-operasyon, uzun-iki-olay
- oynak (2): hareketsiz-kisi (0–2), uzun-tek-olay (0–2)
- her koşuda hata (0): yok


## Yanlış alarm (olaysız kayıtlar)

Ground truth'u sıfır olan kayıtlarda üretilen her olay yanlıştır. Eşleştirme belirsizliği olmadığı için bu ölçüt recall'dan daha güvenilir.

| varyant | video | koşu başına üretilen olay |
|---|---|---|
| `gomulu` | normal-operasyon | 7, 9, 6 |
| `olaysiz` | normal-operasyon | 0, 0, 0 |

## Koşu başına ayrıntı

Hücreler `yakalanan/gerçek (modelin ürettiği)` biçiminde.

### `gomulu` — prompt v4 64f7d03718f6d67f

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| agir-gurultu | 2/2 (3) | 2/2 (3) | 2/2 (1) |
| cok-kisa-an | 0/2 (1) | 2/2 (3) | 0/2 (0) |
| coklu-olay | 3/6 (6) | 6/6 (7) | 6/6 (7) |
| hareketsiz-kisi | 2/2 (19) | 2/2 (3) | 2/2 (5) |
| kucuk-nesne-orta | 2/2 (3) | 2/2 (2) | 2/2 (3) |
| kucuk-nesne-zor | 2/2 (2) | 2/2 (2) | 2/2 (2) |
| net-olay | 0/2 (1) | 0/2 (1) | 0/2 (0) |
| normal-operasyon | 0/0 (7) | 0/0 (9) | 0/0 (6) |
| uzun-iki-olay | 0/4 (3) | 0/4 (1) | 0/4 (0) |
| uzun-tek-olay | 1/2 (6) | 1/2 (3) | 0/2 (3) |

### `olaysiz` — prompt v4 f40d61208a93312f

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| agir-gurultu | 0/2 (0) | 0/2 (0) | 0/2 (0) |
| cok-kisa-an | 0/2 (2) | 0/2 (2) | 0/2 (0) |
| coklu-olay | 0/6 (0) | 0/6 (0) | 0/6 (0) |
| hareketsiz-kisi | 0/2 (0) | 0/2 (0) | 2/2 (3) |
| kucuk-nesne-orta | 0/2 (0) | 0/2 (0) | 0/2 (0) |
| kucuk-nesne-zor | 0/2 (0) | 0/2 (0) | 0/2 (0) |
| net-olay | 0/2 (0) | 0/2 (0) | 0/2 (0) |
| normal-operasyon | 0/0 (0) | 0/0 (0) | 0/0 (0) |
| uzun-iki-olay | 0/4 (0) | 0/4 (0) | 0/4 (0) |
| uzun-tek-olay | 2/2 (1) | 0/2 (0) | 0/2 (0) |

