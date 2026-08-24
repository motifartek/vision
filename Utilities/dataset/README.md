> **Yarışma:** TEKNOFEST 2026 - Yapay Zeka Ajanları Yarışması (Senaryo 3)  
> **Lisans:** Academic Research Use Only (CC-BY-NC-4.0 benzeri)  
> **Dil Desteği:** 16 farklı dilde veri toplanmıştır. Türkçe, İngilizce, Rusça, Çince, Almanca ve diğerleri (16 Dil)

## 📋 Veri Seti Hakkında

Bu veri seti, endüstriyel saha operasyonlarında meydana gelebilecek **kritik olayların tespiti** ve **karar destek sistemlerinin eğitimi** amacıyla oluşturulmuştur. 

Veri seti oluşturulurken 16 farklı dilde araştırılma yapılmıştır.
1. Türkçe (tr)
2. İngilizce (en)
3. Almanca (de)
4. Rusça (ru)
5. İspanyolca (es)
6. Fransızca (fr)
7. İtalyanca (it)
8. Portekizce (pt)
9. Lehçe (pl)
10. Çince (zh)
11. Japonca (ja)
12. Korece (ko)
13. Hintçe (hi)
14. Arapça (ar)
15. Vietnamca (vi)
16. Endonezce (id)

Veri seti, internet üzerindeki açık kaynaklardan (haber siteleri, stok fotoğraf platformları, kamu güvenliği arşivleri) toplanan görsellerin; çok dilli sorgu matrisleri, CLIP tabanlı semantik filtreleme ve insan doğrulama süreçlerinden geçirilmesiyle elde edilmiştir.

### 🎯 Kapsanan Olay Sınıfları (Şartname Hizalı)

| Sınıf Adı | Açıklama | Şartname Karşılığı |
| :--- | :--- | :--- |
| `fire_smoke` | Yangın, duman, patlama | Kritik anomali |
| `forklift_accident` | Forklift devrilmesi, çarpışma, ezilme | "Forklift devrilmesi..." |
| `intrusion` | İzinsiz giriş, çevre ihlali | Güvenlik riski |
| `worker_fall` | İşçi düşmesi, kayma, takılma | Riskli durum tespiti |
| `normal_ops` | Normal operasyon (Negatif sınıf) | Yanlış alarm oranını düşürmek için |

## 📥 İndirme ve Kurulum

Telif hakları (Madde 17) ve depolama optimizasyonu gereği, görsellerin fiziksel kopyaları yerine **metadata indeksi** ve **otomatik indirme scripti** sağlanmıştır. Bu yöntem, akademik dünyada (örn. LAION-5B, Common Crawl) standarttır.

Veri setini yerel ortamınıza indirmek için:

```bash
# 1. Gerekli kütüphaneleri yükleyin
uv sync

# 2. İndirme scriptini çalıştırın
uv run download_dataset.py 
```

Bu komut, metadata_clean.json dosyasındaki kaynak URL'leri kullanarak görselleri paralel olarak ./dataset_downloaded/ klasörüne indirecektir.

## 🗂️ Dosya Yapısı

dataset/
├── forklift_accident/       # Sınıf bazlı görseller (İndirildikten sonra)
│   ├── tr_q00_001.jpg       # Dosya adı formatı: {dil}_{sorgu_id}_{gorsel_id}.jpg
│   ├── en_q00_005.jpg
│   └── ...
├── fire_smoke/
├── intrusion/
├── normal_ops/
├── worker_fall/
├── metadata_clean.json      # Tüm veri setinin kimliği (Sınıf, Dil, URL, Lisans)
├── download_dataset.py      # Yüksek hızlı paralel indirme aracı
└── README.md                

## 📊 Metadata Formatı (metadata_clean.json)
Her bir görsel için aşağıdaki yapısal bilgi tutulmaktadır:

```json
{
  "file_name": "forklift_accident/tr_q00_001.jpg",
  "class": "forklift_accident",
  "language": "tr",
  "source_url": "https://...",
  "license": "Academic Research Use Only"
}
```

## 🛠️ Veri Mühendisliği Pipeline'ı

Bu veri seti şu adımlarla üretilmiştir:
1. **Çok Dilli Sorgu Matrisi:** 16 dilde [Ekipman × Olay × Sahne × Kamera] kartezyen çarpımı ile sorgu üretimi.
2. **Toplama:** DuckDuckGo ve YouTube üzerinden otomatik kazıma (ddgs, yt-dlp).
3. **Filtreleme (Precision):**
    - **imagededup:** Algısal hash (PHash) ile kopya temizleme.
    - **OpenCLIP:** Sahne alaka skoru (ViT-B-32).
    - **Moondream2 / Qwen-VL:** Olay doğrulama kapısı (VLM Gate).
4. **Konsolidasyon:** Fiziksel varlık kontrolü ve URL senkronizasyonu.
