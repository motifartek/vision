//! Override deposu ve doğrulama.
//!
//! Katalog doğruluk kaynağı; depo yalnızca **üstüne binen** düzenlemeleri
//! tutuyor (tasarım §K2). Depo yoksa, düşmüşse ya da kayıt geçersizse sistem
//! gömülüye düşer ve çalışmaya devam eder.
//!
//! # Neden önbellek var
//!
//! `render` **eşzamanlı ve hatasız** kalmak zorunda: prompt üretimi analizi
//! düşüremez. Bu yüzden override'lar açılışta ve her yazmadan sonra belleğe
//! alınıyor; render depoya hiç dokunmuyor. Veritabanı analiz sırasında ölse
//! bile son bilinen metinle çalışmaya devam ediyor.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::render;

/// Arayüzden yapılmış tek bir düzenleme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptOverride {
    pub id: String,
    pub agent: String,
    pub fragment: String,
    pub text: String,
    pub author: String,
    /// RFC 3339. Kimin ne zaman değiştirdiği izlenebilir olmalı.
    pub updated_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("depo hatası: {0}")]
    Backend(String),
}

/// Override deposu.
///
/// Trait olmasının sebebi test edilebilirlik: prompt çözümlemesi veritabanı
/// olmadan sınanabilmeli. Üretimde Postgres, testlerde bellek içi.
#[async_trait::async_trait]
pub trait PromptStore: Send + Sync {
    async fn list(&self) -> Result<Vec<PromptOverride>, StoreError>;
    async fn put(&self, o: PromptOverride) -> Result<(), StoreError>;
    async fn delete(&self, agent: &str, fragment: &str) -> Result<(), StoreError>;
}

/// Bellek içi depo — testler ve veritabanısız çalıştırma için.
#[derive(Default)]
pub struct MemoryStore {
    kayitlar: std::sync::Mutex<BTreeMap<(String, String), PromptOverride>>,
}

#[async_trait::async_trait]
impl PromptStore for MemoryStore {
    async fn list(&self) -> Result<Vec<PromptOverride>, StoreError> {
        Ok(self.kayitlar.lock().unwrap().values().cloned().collect())
    }
    async fn put(&self, o: PromptOverride) -> Result<(), StoreError> {
        self.kayitlar
            .lock()
            .unwrap()
            .insert((o.agent.clone(), o.fragment.clone()), o);
        Ok(())
    }
    async fn delete(&self, agent: &str, fragment: &str) -> Result<(), StoreError> {
        self.kayitlar
            .lock()
            .unwrap()
            .remove(&(agent.to_string(), fragment.to_string()));
        Ok(())
    }
}

/// Bir override'ın neden reddedildiği.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("'{0}' parçası düzenlenemez: çıktı sözleşmesine bağlı")]
    NotEditable(String),
    #[error("'{0}' kataloğda yok")]
    UnknownFragment(String),
    #[error("tanınmayan yer tutucu: {{{0}}}")]
    UnknownPlaceholder(String),
    #[error("güvenilmez bölge ayracı metinde geçemez")]
    DelimiterInText,
    #[error("metin çok uzun: {0} karakter, sınır {1}")]
    TooLong(usize, usize),
    #[error("metin boş")]
    Empty,
}

/// Tek bir parça için üst sınır.
///
/// Bağlamı şişirmemesi ve kazara yapıştırılmış bir dokümanın prompt'a
/// girmemesi için.
const AZAMI_PARCA: usize = 8_192;

/// Bir override'ı kaydetmeden **ve** kullanmadan önce denetler.
///
/// İki kapı bilinçli: kaydetme sırasında reddetmek kullanıcıya hemen geri
/// bildirim veriyor, kullanım sırasında denetlemek ise elle veritabanına
/// yazılmış bozuk bir kaydın sisteme sızmasını engelliyor.
pub(crate) fn dogrula(
    fragment_adi: &str,
    metin: &str,
    duzenlenebilir_mi: Option<bool>,
) -> Result<(), ValidationError> {
    match duzenlenebilir_mi {
        None => return Err(ValidationError::UnknownFragment(fragment_adi.to_string())),
        Some(false) => return Err(ValidationError::NotEditable(fragment_adi.to_string())),
        Some(true) => {}
    }

    if metin.trim().is_empty() {
        return Err(ValidationError::Empty);
    }
    let uzunluk = metin.chars().count();
    if uzunluk > AZAMI_PARCA {
        return Err(ValidationError::TooLong(uzunluk, AZAMI_PARCA));
    }

    // Yer tutucular katalogdakiyle aynı kurallara tabi.
    render::yer_tutuculari_dogrula(fragment_adi, metin)
        .map_err(|e| match e {
            crate::PromptError::UnknownPlaceholder { placeholder, .. } => {
                ValidationError::UnknownPlaceholder(placeholder)
            }
            _ => ValidationError::UnknownPlaceholder("?".into()),
        })?;

    // Ayraç taklidi: override üzerinden güvenilmez bölgeyi kapatma yolu
    // açılmamalı.
    if metin.contains(crate::context::BAGLAM_BASI) || metin.contains(crate::context::BAGLAM_SONU) {
        return Err(ValidationError::DelimiterInText);
    }

    Ok(())
}

/// Render edilmiş prompt'un çıktı sözleşmesini hâlâ tarif ettiğini denetler.
///
/// Parça bazlı doğrulama yetmiyor: biri `sozlesme`'yi silemez ama `rol`'ü
/// şemayı bozacak biçimde değiştirebilir. Bu kapı, üretilen metnin
/// ayrıştırılabilir kalmasını garanti ediyor.
pub(crate) fn sozlesme_duruyor_mu(prompt: &str) -> bool {
    ["summary", "events", "risk", "actions"]
        .iter()
        .all(|a| prompt.contains(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duzenlenemez_parca_reddedilir() {
        // §K3: sözleşme koda bağlı; değiştirilirse çıktı kırılır.
        let e = dogrula("sozlesme", "yeni metin", Some(false)).unwrap_err();
        assert_eq!(e, ValidationError::NotEditable("sozlesme".into()));
    }

    #[test]
    fn bilinmeyen_parca_reddedilir() {
        assert!(matches!(
            dogrula("olmayan", "x", None),
            Err(ValidationError::UnknownFragment(_))
        ));
    }

    #[test]
    fn taninmayan_yer_tutucu_reddedilir() {
        assert!(matches!(
            dogrula("rol", "merhaba {bilinmeyen}", Some(true)),
            Err(ValidationError::UnknownPlaceholder(_))
        ));
    }

    #[test]
    fn taninan_yer_tutucu_kabul_edilir() {
        assert!(dogrula("kayit_bilgisi", "Uzunluk {sure}.", Some(true)).is_ok());
    }

    #[test]
    fn ayrac_iceren_metin_reddedilir() {
        // Aksi hâlde override, güvenilmez bölgeyi kapatmanın yolu olurdu.
        let metin = format!("zararsız {}", crate::context::BAGLAM_SONU);
        assert_eq!(
            dogrula("rol", &metin, Some(true)).unwrap_err(),
            ValidationError::DelimiterInText
        );
    }

    #[test]
    fn bos_ve_asiri_uzun_metin_reddedilir() {
        assert_eq!(dogrula("rol", "   ", Some(true)).unwrap_err(), ValidationError::Empty);
        let uzun = "a".repeat(AZAMI_PARCA + 1);
        assert!(matches!(
            dogrula("rol", &uzun, Some(true)),
            Err(ValidationError::TooLong(_, _))
        ));
    }

    #[test]
    fn sozlesme_isaretleri_aranir() {
        assert!(sozlesme_duruyor_mu(
            r#"{"summary":"","events":[],"risk":"","actions":[]}"#
        ));
        assert!(!sozlesme_duruyor_mu("yalnızca özet ver"));
    }

    #[tokio::test]
    async fn bellek_deposu_yazar_okur_siler() {
        let s = MemoryStore::default();
        let o = PromptOverride {
            id: "1".into(),
            agent: "vision".into(),
            fragment: "rol".into(),
            text: "yeni rol".into(),
            author: "fatih".into(),
            updated_at: "2026-08-27T00:00:00Z".into(),
        };
        s.put(o.clone()).await.unwrap();
        assert_eq!(s.list().await.unwrap(), vec![o]);

        s.delete("vision", "rol").await.unwrap();
        assert!(s.list().await.unwrap().is_empty());
    }
}
