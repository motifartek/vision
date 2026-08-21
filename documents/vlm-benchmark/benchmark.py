import os
import base64
import json
import pandas as pd
from sklearn.metrics import recall_score, accuracy_score, confusion_matrix
from vllm import LLM, SamplingParams

MODEL_NAME = "" 
DATASET_DIR = ""

categories = ["fire_smoke", "forklift_accident", "intrusion", "normal_ops", "worker_fall"]

def evaluate():
    print(f"Model Yükleniyor: {MODEL_NAME}")
    print("vLLM Engine başlatılıyor... Lütfen modelin belleğe yüklenmesini bekleyin.\n")
    
    try:
        llm = LLM(
            model=MODEL_NAME, 
            trust_remote_code=True,
            max_model_len=4096  
        )
    except Exception as e:
        print(f"Model yüklenirken hata oluştu (Bellek yetersiz olabilir): {e}")
        return [], [], [], []
    
    sampling_params = SamplingParams(temperature=0.0, max_tokens=100)
    
    tasks = []          
    messages_list = []  
    
    print("Veri seti taranıyor...")
    
    # 1. Resimleri tara ve listeyi oluştur
    for category in categories:
        folder_path = os.path.join(DATASET_DIR, category)
        if not os.path.exists(folder_path): 
            continue
            
        for img_name in os.listdir(folder_path):
            if not img_name.lower().endswith(('.png', '.jpg', '.jpeg')): 
                continue
                
            img_path = os.path.join(folder_path, img_name)
            
            with open(img_path, "rb") as image_file:
                base64_image = base64.b64encode(image_file.read()).decode('utf-8')
            
            # Ground Truth Ayarı (Normal = 0, Tehlike = 1)
            is_danger_true = 0 if category == "normal_ops" else 1
            tasks.append((img_path, category, img_name, is_danger_true))
            
            prompt = """
            You are an AI specialized in Health, Safety, and Environment (HSE) monitoring.
            Analyze the image and respond strictly in JSON format.
            1. 'is_danger': boolean (true if there is a fire, forklift accident, intrusion, or worker fall. false if normal).
            2. 'category': string (must be one of: 'fire_smoke', 'forklift_accident', 'intrusion', 'normal_ops', 'worker_fall').
            """
            
            messages = [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{base64_image}"}}
                    ]
                }
            ]
            messages_list.append(messages)

    toplam_resim = len(messages_list)
    print(f"Toplam {toplam_resim} resim bulundu. Inference (tekil işleme) başlıyor...\n")
    
    results = []
    
    # 2. Sequential Inference (Mac için çökme engelleyici döngü)
    for idx, (task, message) in enumerate(zip(tasks, messages_list)):
        img_path, category, img_name, is_danger_true = task
        print(f"İşleniyor ({idx + 1}/{toplam_resim}): {img_name}...", end=" ")
        
        try:
            output = llm.chat(messages=[message], sampling_params=sampling_params, use_tqdm=False)
            res_content = output[0].outputs[0].text

            clean_content = res_content.strip().lstrip("```json").rstrip("```").strip()
            res_json = json.loads(clean_content)
            
            is_danger_pred = 1 if res_json.get('is_danger', False) else 0
            category_pred = res_json.get('category', 'normal_ops')
            
            results.append((is_danger_true, is_danger_pred, category, category_pred))
            print("Başarılı.")
            
        except json.JSONDecodeError:
            print(f"HATA: JSON ayrıştırılamadı. Model Çıktısı: {res_content[:50]}...")
            results.append((is_danger_true, 0, category, 'parse_error'))
        except Exception as e:
            print(f"HATA: Çıkarım sırasında sistem hatası: {e}")
            results.append((is_danger_true, 0, category, 'system_error'))

    # Matris hesaplaması için listeleri ayır
    if not results:
        return [], [], [], []
        
    y_true_binary = [r[0] for r in results]
    y_pred_binary = [r[1] for r in results]
    y_true_multi = [r[2] for r in results]
    y_pred_multi = [r[3] for r in results]

    return y_true_binary, y_pred_binary, y_true_multi, y_pred_multi

# Benchmark
t_bin, p_bin, t_multi, p_multi = evaluate()

if not t_bin:
    print("Hiçbir resim değerlendirilemedi. Klasör yollarını veya model yüklemesini kontrol edin.")
else:
    # 1. Binary Metrikler (Tehlike Tespiti)
    recall_danger = recall_score(t_bin, p_bin, pos_label=1, zero_division=0) * 100
    tn, fp, fn, tp = confusion_matrix(t_bin, p_bin, labels=[0, 1]).ravel()
    fpr_danger = (fp / (fp + tn)) * 100 if (fp + tn) > 0 else 0

    # 2. Multi-class Metrikler (Kategori Bazlı)
    accuracy_overall = accuracy_score(t_multi, p_multi) * 100

    # 3. Kategori Spesifik Recall
    cat_recalls = {}
    for cat in ["fire_smoke", "worker_fall", "forklift_accident", "intrusion"]:
        cat_t = [1 if x == cat else 0 for x in t_multi]
        cat_p = [1 if x == cat else 0 for x in p_multi]
        if sum(cat_t) > 0:
            cat_recalls[cat] = recall_score(cat_t, cat_p, pos_label=1, zero_division=0) * 100
        else:
            cat_recalls[cat] = 0.0

    # Tablo Verisi (Pandas DataFrame)
    model_short_name = MODEL_NAME.split('/')[-1]
    
    data = {
        "Görevler": [
            "**Tehlike Tespiti (Binary)**",
            "Tehlike Recall (Duyarlılık)",
            "Yanlış Alarm (FPR)",
            "**Kategori Bilişi (Multi-class)**",
            "Genel Doğruluk (Accuracy)",
            "**Spesifik Kategori Başarısı**",
            "fire_smoke Tespiti",
            "worker_fall Tespiti",
            "forklift_accident Tespiti",
            "intrusion Tespiti"
        ],
        model_short_name: [
            "", 
            f"{recall_danger:.1f}",
            f"{fpr_danger:.1f}",
            "", 
            f"{accuracy_overall:.1f}",
            "", 
            f"{cat_recalls.get('fire_smoke', 0):.1f}",
            f"{cat_recalls.get('worker_fall', 0):.1f}",
            f"{cat_recalls.get('forklift_accident', 0):.1f}",
            f"{cat_recalls.get('intrusion', 0):.1f}"
        ]
    }

    df = pd.DataFrame(data)
    print("\n" + "="*60)
    print(df.to_markdown(index=False))
    print("="*60)

    csv_filename = f"benchmark_{model_short_name}.csv"
    df.to_csv(csv_filename, index=False)
    print(f"\nSonuçlar yerel klasöre kaydedildi: '{csv_filename}'")