# Prompt ölçümü

10 video · 3 koşu · tolerans ±3 sn

| varyant | olay (ort/toplam) | yayılım | hata | model olayı | sapma | süre |
|---|---|---|---|---|---|---|
| `gomulu` | 14.7/24 | 14–15 | 0/30 | 31.3 | 473 ms | 41.1 sn |

## Terazi gürültüsü

Bir varyantın kendi koşumları arasında en fazla **1 olay** oynadı. Bir değişikliğin etkisi ancak bu yayılımı aşarsa iddia edilebilir; bandın içindeki fark ölçüm gürültüsüdür.

## Video kararlılığı

Koşumlar arasında sonucu değişmeyen videolar karşılaştırmada güvenilir taban; değişenler bandı tek başına açabiliyor.

**`gomulu`**

- kararlı (9): agir-gurultu, cok-kisa-an, coklu-olay, kucuk-nesne-orta, kucuk-nesne-zor, net-olay, normal-operasyon, uzun-iki-olay, uzun-tek-olay
- oynak (1): hareketsiz-kisi (1–2)
- her koşuda hata (0): yok


## Koşu başına ayrıntı

### `gomulu` — prompt v4 64f7d03718f6d67f

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| agir-gurultu | 2/2 | 2/2 | 2/2 |
| cok-kisa-an | 0/2 | 0/2 | 0/2 |
| coklu-olay | 6/6 | 6/6 | 6/6 |
| hareketsiz-kisi | 2/2 | 1/2 | 2/2 |
| kucuk-nesne-orta | 2/2 | 2/2 | 2/2 |
| kucuk-nesne-zor | 2/2 | 2/2 | 2/2 |
| net-olay | 0/2 | 0/2 | 0/2 |
| normal-operasyon | 0/0 | 0/0 | 0/0 |
| uzun-iki-olay | 0/4 | 0/4 | 0/4 |
| uzun-tek-olay | 1/2 | 1/2 | 1/2 |

