# Uzun kayıt ölçüm kümesi

Uzun videolarda **parça boyu** karşılaştırması için. Video depoda tutulmuyor
(`datasets/**/*.mp4` yok sayılıyor); ground truth burada, videoyu aşağıdaki
komut yeniden üretiyor.

## Neden bu küme

`datasets/sentetik` örneklemeyi ölçüyor, semantiği değil — soyut dikdörtgenlerde
"iş güvenliği olayı" aramanın anlamı yok, Faz 3 ve Faz 4'te iki kez bu duvara
çarpıldı. Parça boyu sorusu ise gerçek görüntü gerektiriyor: model sahneyi
tanıyabilmeli ki parça uzunluğunun tespite etkisi görülebilsin.

Gerçek uzun İSG kaydı elimizde yoktu. Onun yerine **gerçek** bir servis kazası
kaydı (44,800 sn, 1280×720, tek sabit kamera) tekrarlanarak 10 dakikaya
uzatıldı. Böylece hem görüntü gerçek hem de olayın **tam zamanı biliniyor**.

## Ground truth nereden geliyor

Olay `30,5 + k × 44,800` saniyelerde; 13 olay. Bu zaman üç bağımsız kaynağın
mutabakatı:

- hareket profili 31–32. saniyede tepe yapıyor (skor 1.00, videodaki en yüksek)
- model bağımsız koşularda 00:29–00:30 diyor
- panelin gösterdiği rapor da aynı anı işaret ediyor

**İnsan anotasyonu değil.** Mutlak recall değeri bu yüzden bir başarı ölçütü
sayılmamalı; kümenin amacı **iki yapılandırmayı karşılaştırmak**.

## Zayıflığı

Tekrar yapay: model aynı sahneyi 13 kez görüyor. Parça boyu sorusunu bozmuyor
(her parça bağımsız çağrı) ama gerçek, tekrarsız uzun kayıt bulunduğunda bu
küme onunla değiştirilmeli.

## Yeniden üretmek

Kaynak: `data/stream/raw` altındaki 44,8 saniyelik servis kazası kaydı.

```bash
# 15 kez ardışık ekle, 600 saniyede kes
for i in $(seq 1 15); do printf "file '%s'\n" "$PWD/<kaynak>.mp4"; done > liste.txt
ffmpeg -f concat -safe 0 -i liste.txt -c copy -t 600 datasets/uzun/servis-kazasi-10dk.mp4
```

Kaynak süresi **tam 44,800 sn** olmalı; farklıysa ground truth'taki periyot da
değişir. 45 sn varsayılsaydı 13 tekrarda 2,6 saniyelik sapma birikir ve
±3 sn toleransının sınırına dayanırdı.

## Koşturmak

```bash
bench prompts --dataset datasets/uzun --parca-boylari 260000,120000 --tekrar 3
```

İki boy **aynı koşuda** karşılaştırılıyor: gürültü bandı oturumlar arasında
değişiyor (ölçüldü — aynı kod, aynı küme, 1'e karşı 5 olay), ayrı koşularda
kıyaslamak yanıltırdı.
