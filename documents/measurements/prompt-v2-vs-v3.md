# Prompt ölçümü — v2 (gömülü) vs v3 (kısa hareket uyarısı)

> `bench prompts --variants gomulu,v3=<dizin> --videos 6` · 2026-08-27
>
> Katalog: [`prompt-katalog-v2.toml`](prompt-katalog-v2.toml)

## Denenen değişiklik

v3, rol cümlesine bir uyarı ekliyor:

> *"Küçük ve kısa süren hareketleri de atlama; kazalar çoğu zaman bir
> saniyeden kısa sürer."*

Gerekçesi makuldü: golden dataset'te kaçırılan olayların tamamı **düşük
hareketli**. Modelin dikkatini oraya çekmek eşleşmeyi artırabilirdi.

## Sonuç

| Varyant | Olay | Şema | Sapma | Boş aksiyon | Süre | Prompt |
|---|---|---|---|---|---|---|
| gömülü | **22/23** (%95) | 6/6 | 868 ms | 0 | 8,7 sn | v2 `40cb6470` |
| v3 | 21/23 (%91) | 6/6 | **548 ms** | 0 | 9,2 sn | v3 `82fd5dd5` |

**Değişiklik iyileştirmedi.** Olay eşleşmesi bir azaldı; kayıp tek bir videoda
yoğunlaştı (`6ZrAeC5mZ5w` 4/4 → 2/4). Zaman sapması düştü ama olay sayısı
azaldığı için bu iyileşme sayılmaz — daha az olay bildiren bir model daha
isabetli görünür.

## Çıkarım

Kulağa mantıklı gelen ve gerçek bir ölçüme (düşük hareketli kaçırmalar)
dayanan bir ekleme işe yaramadı. Ölçüm olmasaydı bu değişiklik "dikkat
çekmeyi artırır" gerekçesiyle kabul edilirdi.

Tek koşuluk fark tesadüf olabilir; model aynı girdiye koşudan koşuya farklı
cevap veriyor. Kararı kesinleştirmek için aynı ölçüm birkaç kez tekrarlanmalı.
Ama **elde iyileşme kanıtı yok**, dolayısıyla v3 alınmıyor.
