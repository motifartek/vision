# Prompt ölçümü

2 video · 3 koşu · tolerans ±3 sn

| varyant | olay (ort/toplam) | yayılım | hata | model olayı | sapma | süre |
|---|---|---|---|---|---|---|
| `gomulu` | 0.7/6 | 0–1 | 0/6 | 6.0 | 1200 ms | 56.8 sn |
| `hareket` | 0.7/6 | 0–2 | 0/6 | 2.0 | 236 ms | 85.7 sn |

## Terazi gürültüsü

Bir varyantın kendi koşumları arasında en fazla **2 olay** oynadı. Bir değişikliğin etkisi ancak bu yayılımı aşarsa iddia edilebilir; bandın içindeki fark ölçüm gürültüsüdür.

## Video kararlılığı

Koşumlar arasında sonucu değişmeyen videolar karşılaştırmada güvenilir taban; değişenler bandı tek başına açabiliyor.

**`gomulu`**

- kararlı (1): uzun-iki-olay
- oynak (1): uzun-tek-olay (0–1)
- her koşuda hata (0): yok

**`hareket`**

- kararlı (1): uzun-iki-olay
- oynak (1): uzun-tek-olay (0–2)
- her koşuda hata (0): yok


## Koşu başına ayrıntı

### `gomulu` — prompt v4 64f7d03718f6d67f

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| uzun-iki-olay | 0/4 | 0/4 | 0/4 |
| uzun-tek-olay | 1/2 | 0/2 | 1/2 |

### `hareket` — prompt v4 64f7d03718f6d67f

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| uzun-iki-olay | 0/4 | 0/4 | 0/4 |
| uzun-tek-olay | 2/2 | 0/2 | 0/2 |

