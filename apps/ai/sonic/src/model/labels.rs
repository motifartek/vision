use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::error::InferenceError;
use crate::safety::{self, Category, Severity};

/// 527 AudioSet sınıfının Türkçe karşılıkları; ikiliye gömülü, çalışma anında
/// dosya bağımlılığı yok.
const LABELS_TR: &str = include_str!("../../data/labels_tr.csv");

/// AudioSet ontolojisinden bir sınıf (class_labels_indices.csv satırı),
/// iş güvenliği katmanındaki karşılığıyla birlikte.
///
/// Önem derecesi burada taşınıyor çünkü arayüz onu **eşikten bağımsız** olarak
/// bilmek zorunda: zaman çizelgesi bloklarını `frames` verisinden çiziyor ve
/// orada yalnızca sınıf indeksi var. Önem yalnızca `safety.events` içinde
/// olsaydı, kullanıcı eşiği indirdiğinde beliren bloklar renksiz kalırdı.
/// Liste sabit ve sayfa ömrü boyunca bir kez indiriliyor; ek maliyeti yok.
#[derive(Debug, Clone, Serialize)]
pub struct ClassLabel {
    pub index: usize,
    pub mid: String,
    pub display_name: String,
    pub display_name_tr: Option<String>,
    /// Güvenlik sınıfı değilse yok — 527 sınıfın 57'sinde dolu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
}

impl ClassLabel {
    /// Türkçe karşılık yoksa özgün İngilizce ada düşer.
    pub fn display_name_tr(&self) -> &str {
        self.display_name_tr.as_deref().unwrap_or(&self.display_name)
    }
}

pub fn load(path: &Path) -> Result<Vec<ClassLabel>, InferenceError> {
    let translations = load_translations()?;

    let mut reader = csv::Reader::from_path(path)
        .map_err(|e| InferenceError::Config(format!("{}: {e}", path.display())))?;

    let mut labels = Vec::with_capacity(527);
    for record in reader.records() {
        let record = record.map_err(|e| InferenceError::Config(e.to_string()))?;
        let display_name = record.get(2).unwrap_or_default().to_string();
        let safety = safety::lookup(&display_name);
        labels.push(ClassLabel {
            index: record
                .get(0)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| InferenceError::Config("sınıf haritasında bozuk index".into()))?,
            mid: record.get(1).unwrap_or_default().to_string(),
            display_name_tr: translations.get(&display_name).cloned(),
            severity: safety.map(|c| c.severity),
            category: safety.map(|c| c.category),
            display_name,
        });
    }

    if labels.len() != 527 {
        tracing::warn!(
            adet = labels.len(),
            "beklenen 527 AudioSet sınıfı yerine farklı sayıda etiket yüklendi"
        );
    }

    let missing = labels.iter().filter(|l| l.display_name_tr.is_none()).count();
    if missing > 0 {
        tracing::warn!(adet = missing, "Türkçe karşılığı olmayan sınıf var, İngilizceye düşülüyor");
    }

    Ok(labels)
}

fn load_translations() -> Result<HashMap<String, String>, InferenceError> {
    let mut reader = csv::Reader::from_reader(LABELS_TR.as_bytes());
    let mut map = HashMap::with_capacity(527);
    for record in reader.records() {
        let record = record.map_err(|e| InferenceError::Config(e.to_string()))?;
        if let (Some(en), Some(tr)) = (record.get(0), record.get(1)) {
            map.insert(en.to_string(), tr.to_string());
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turkish_map_is_complete_and_parses() {
        let map = load_translations().expect("labels_tr.csv ayrıştırılmalı");
        assert_eq!(map.len(), 527, "527 sınıfın tamamı çevrilmiş olmalı");
        // Virgüllü adların tırnaklaması doğru mu?
        assert_eq!(map.get("Baby cry, infant cry").map(String::as_str), Some("Bebek ağlaması"));
        assert_eq!(map.get("Gunshot, gunfire").map(String::as_str), Some("Silah sesi"));
        assert_eq!(map.get("Shatter").map(String::as_str), Some("Cam kırılması"));
    }

    /// Güvenlik tablosu sınıfları İngilizce adla arıyor. Addaki tek harflik bir
    /// sapma kuralı **sessizce** devre dışı bırakır: hata vermez, sadece o ses
    /// bir daha asla tetiklenmez. Bu test o sessizliği gürültüye çevirir.
    #[test]
    fn safety_classes_map_to_real_audioset_labels() {
        let map = load_translations().expect("labels_tr.csv ayrıştırılmalı");
        let missing: Vec<&str> = crate::safety::SAFETY_CLASSES
            .iter()
            .map(|c| c.en)
            .filter(|en| !map.contains_key(*en))
            .collect();
        assert!(missing.is_empty(), "AudioSet'te karşılığı olmayan güvenlik sınıfı adı: {missing:?}");
    }
}
