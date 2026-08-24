use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Bir videonun sistem genelindeki kimliği.
///
/// Newtype olması bilinçli: `String` alan fonksiyonlara yanlışlıkla bir
/// nesne anahtarı ya da dosya adı geçirmeyi derleme zamanında engeller.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VideoId(String);

impl VideoId {
    /// Yeni rastgele bir kimlik üretir.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Bu videonun ham dosyasının nesne deposundaki anahtarı.
    pub fn raw_object_key(&self, extension: &str) -> String {
        format!("raw/{}.{}", self.0, extension)
    }

    /// Belirli bir zaman damgasındaki karenin nesne anahtarı.
    ///
    /// Sıfır dolgulu ms kullanılır; böylece anahtarlar sözlük sırasına
    /// göre listelendiğinde kronolojik sırada gelir.
    pub fn frame_object_key(&self, t_ms: u64) -> String {
        format!("frames/{}/{:09}.jpg", self.0, t_ms)
    }

    /// Bu videonun hareket profilinin nesne anahtarı.
    pub fn profile_object_key(&self) -> String {
        format!("profiles/{}.json", self.0)
    }
}

impl Default for VideoId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for VideoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for VideoId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl FromStr for VideoId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesne_anahtarlari_kronolojik_siralanir() {
        let id = VideoId::from("abc".to_string());
        let mut keys = vec![
            id.frame_object_key(120_000),
            id.frame_object_key(9_200),
            id.frame_object_key(15_200),
        ];
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "frames/abc/000009200.jpg",
                "frames/abc/000015200.jpg",
                "frames/abc/000120000.jpg",
            ]
        );
    }

    #[test]
    fn kimlik_seffaf_serilesir() {
        let id = VideoId::from("xyz".to_string());
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"xyz\"");
    }
}
