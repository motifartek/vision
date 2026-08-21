# 📊 Model Benchmark ve Mimari Karar Raporu

> **İlgili Şartname Maddeleri:** Madde 4 (Temel Beklentiler - Performans ve Ölçümleme), Madde 7 (Değerlendirme Kriterleri - Teknik İmplementasyon)

## 1. Test Ortamı ve Veri Seti
Sistemimiz, savunma sanayi ve saha operasyonları senaryosuna (Senaryo 3) özel olarak derlenen, 16 dilli ve çok aşamalı filtreleme (CLIP + İnsan incelemesi) süreçlerinden geçirilmiş **778 adet yüksek kaliteli test görseli** üzerinde değerlendirilmiştir. (Tüm görsellere utilities/dataset içindeki indirme linkinden ulaşılabilir.)

**Test Verisi Dağılımı:**
*   `normal_ops` (Negatif Sınıf / Yanlış Alarm Testi): 339 görsel
*   `worker_fall` (İşçi Düşmesi): 186 görsel
*   `forklift_accident` (Forklift Kazası): 117 görsel
*   `fire_smoke` (Yangın/Duman): 102 görsel
*   `intrusion` (İzinsiz Giriş): 44 görsel

## 2. Karşılaştırmalı Performans Metrikleri (KPI)

Aday Multimodal LLM (VLM) modelleri, vLLM altyapısı üzerinde aşağıdaki metriklerle test edilmiştir:

| Görevler / Metrikler | Qwen3-VL-32B | Qwen3-VL-30B-A3B(MoE) | Qwen3-Omni-30B-A3B | Gemma-4-31B-it | InternVL3-78B |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Tehlike Tespiti (Binary)** | | | | | |
| Tehlike Recall (Duyarlılık) | **97.7%** | 96.6% | 96.1% | 98.6% | - [1] |
| Yanlış Alarm Oranı (FPR) ⚠️ | **0.6%** | **0.6%** | **0.6%** | 1.2% | - [1] |
| **Kategori Bilişi (Multi-class)** | | | | | |
| Genel Doğruluk (Accuracy) | **98.2%** | 97.3% | 97.3% | 96.7% | - [1] |
| **Spesifik Kategori Başarısı** | | | | | |
| `fire_smoke` Tespiti | **100.0%** | 99.0% | 99.0% | 99.0% | - [1] |
| `worker_fall` Tespiti | **98.4%** | 95.1% | 97.3% | 90.8% | - [1] |
| `forklift_accident` Tespiti | 95.7% | 94.8% | 91.3% | **97.4%** | - [1] |
| `intrusion` Tespiti | 93.9% | 92.9% | 90.5% | **97.6%** | - [1] |

*(⚠️ FPR - False Positive Rate: Savunma ve saha operasyonlarında operatör güvenini sarsmamak için en kritik metriktir.)*

*[1] Donanım Kısıtı:* InternVL3-78B, VRAM gereksinimini karşılayamadığımdan (OOM Error) değerlendirme dışı bırakılmıştır.
Ayrıca, 78 milyar parametreli bir modelin yerel donanımlar üzerindeki çıkarım (inference) süresi, projenin temel hedeflerinden olan "düşük gecikmeli operasyonel karar destek" gereksinimiyle örtüşmediği için mimari açıdan uygun bulunmamıştır.


## 3. Mimari Karar

Benchmark sonuçları incelendiğinde, `Qwen3-VL-32B-Instruct` modelinin kağıt üzerinde %98.2 doğruluk ile en yüksek skoru aldığı görülmektedir. Geliştirilen ajan sisteminde `Qwen3-VL-32B-Instruct` modelini kullanmayı uygun buluyoruz. 
