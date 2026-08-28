# Prompt ölçümü

1 video · 3 koşu · tolerans ±3 sn

| varyant | olay (ort/toplam) | yayılım | hata | model olayı | sapma | süre |
|---|---|---|---|---|---|---|
| `gomulu-parca260` | 3.0/13 | 3–3 | 0/3 | 13.3 | 922 ms | 98.0 sn |
| `gomulu-parca120` | 10.0/13 | 8–12 | 0/3 | 48.7 | 607 ms | 153.7 sn |

## Terazi gürültüsü

Bir varyantın kendi koşumları arasında en fazla **4 olay** oynadı. Bir değişikliğin etkisi ancak bu yayılımı aşarsa iddia edilebilir; bandın içindeki fark ölçüm gürültüsüdür.

## Video kararlılığı

Koşumlar arasında sonucu değişmeyen videolar karşılaştırmada güvenilir taban; değişenler bandı tek başına açabiliyor.

**`gomulu-parca260`**

- kararlı (1): servis-kazasi-10dk
- oynak (0): yok
- her koşuda hata (0): yok

**`gomulu-parca120`**

- kararlı (0): yok
- oynak (1): servis-kazasi-10dk (8–12)
- her koşuda hata (0): yok


## Koşu başına ayrıntı

Hücreler `yakalanan/gerçek (modelin ürettiği)` biçiminde.

### `gomulu-parca260` — prompt v4 d9ebd55f3a39068d

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| servis-kazasi-10dk | 3/13 (14) | 3/13 (15) | 3/13 (11) |

### `gomulu-parca120` — prompt v4 d9ebd55f3a39068d

| video | koşu 1 | koşu 2 | koşu 3 |
|---|---|---|---|
| servis-kazasi-10dk | 10/13 (45) | 8/13 (54) | 12/13 (47) |

