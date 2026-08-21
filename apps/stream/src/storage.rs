//! Nesne deposu soyutlaması.
//!
//! Servis nesnelerin **nerede durduğunu** bilmez, yalnızca anahtarlarını bilir.
//! Bu ayrım sayesinde test arayüzü ve benchmark hiçbir altyapı ayağa
//! kaldırmadan çalışıyor; MinIO geldiğinde tek yapılacak [`BlobStore`]'un
//! ikinci bir gerçeklemesini bağlamak.
//!
//! Şu an tek gerçekleme [`LocalStore`] (yerel dosya sistemi). S3/MinIO
//! gerçeklemesi aynı arayüzün arkasına girecek — bkz. `documents/features/
//! stream-service.md`.

use std::path::{Component, Path, PathBuf};

use motif_core::{Error, Result};

/// Nesne deposu.
///
/// Anahtarlar `raw/<id>.mp4`, `frames/<id>/<t>.jpg` gibi eğik çizgiyle ayrılmış
/// yollardır. Nesne deposu semantiği: dizin diye bir şey yok, sadece anahtar.
pub trait BlobStore: Send + Sync {
    /// Anahtarın altında yatan yerel dosya yolu.
    ///
    /// ffmpeg'in doğrudan dosyaya erişmesi gerekiyor: videoyu belleğe alıp
    /// borudan beslemek, iki dakikalık dosyada gereksiz bir kopya demek.
    /// S3 gerçeklemesinde bu, önce yerele indirmeyi gerektirecek.
    fn local_path(&self, key: &str) -> Result<PathBuf>;

    fn put(&self, key: &str, bytes: &[u8]) -> Result<()>;
    fn get(&self, key: &str) -> Result<Vec<u8>>;
    fn exists(&self, key: &str) -> bool;
    /// Verilen önekle başlayan anahtarlar.
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Anahtarın dizin dışına taşmadığını doğrular.
///
/// `..` ve mutlak yol bileşenleri reddedilir. Anahtarlar dışarıdan (yükleme
/// adı, tool isteği) geldiği için bu kontrol isteğe bağlı değil.
fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(Error::Config("boş nesne anahtarı".into()));
    }

    let path = Path::new(key);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(Error::Config(format!(
                    "geçersiz nesne anahtarı: {key}"
                )))
            }
        }
    }

    Ok(())
}

/// Yerel dosya sistemi üzerinde nesne deposu.
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn resolve(&self, key: &str) -> Result<PathBuf> {
        validate_key(key)?;
        Ok(self.root.join(key))
    }
}

impl BlobStore for LocalStore {
    fn local_path(&self, key: &str) -> Result<PathBuf> {
        let path = self.resolve(key)?;
        if !path.exists() {
            return Err(Error::NotFound(key.to_string()));
        }
        Ok(path)
    }

    fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Önce geçici dosyaya yaz, sonra taşı: yarım yazılmış bir nesne asla
        // görünmesin. Yükleme kesilirse tüketici bozuk kare okumamalı.
        let temp = path.with_extension("part");
        std::fs::write(&temp, bytes)?;
        std::fs::rename(&temp, &path)?;
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.resolve(key)?;
        std::fs::read(&path).map_err(|_| Error::NotFound(key.to_string()))
    }

    fn exists(&self, key: &str) -> bool {
        self.resolve(key).map(|p| p.exists()).unwrap_or(false)
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let dir = self.resolve(prefix)?;
        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".part") {
                continue;
            }
            keys.push(format!("{}/{name}", prefix.trim_end_matches('/')));
        }

        keys.sort();
        Ok(keys)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let path = self.resolve(key)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gecici_depo() -> (LocalStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("motif-store-{}", uuid::Uuid::new_v4()));
        (LocalStore::new(&dir).unwrap(), dir)
    }

    #[test]
    fn yaz_oku_listele_sil() {
        let (store, dir) = gecici_depo();

        store.put("frames/abc/000001000.jpg", b"birinci").unwrap();
        store.put("frames/abc/000002000.jpg", b"ikinci").unwrap();

        assert_eq!(store.get("frames/abc/000001000.jpg").unwrap(), b"birinci");
        assert!(store.exists("frames/abc/000002000.jpg"));

        let keys = store.list("frames/abc").unwrap();
        assert_eq!(keys.len(), 2);
        // Sıfır dolgulu anahtarlar sözlük sırasında kronolojik gelir.
        assert!(keys[0].ends_with("000001000.jpg"));

        store.delete("frames/abc/000001000.jpg").unwrap();
        assert!(!store.exists("frames/abc/000001000.jpg"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dizin_disina_cikan_anahtar_reddedilir() {
        let (store, dir) = gecici_depo();

        for kotu in [
            "../gizli.txt",
            "frames/../../gizli.txt",
            "/etc/passwd",
            "",
        ] {
            assert!(
                store.put(kotu, b"x").is_err(),
                "kabul edilmemeliydi: {kotu}"
            );
        }

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn olmayan_anahtar_notfound_verir() {
        let (store, dir) = gecici_depo();

        assert!(matches!(
            store.get("yok/olan.jpg"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            store.local_path("yok/olan.jpg"),
            Err(Error::NotFound(_))
        ));
        assert!(store.list("hic/olmayan").unwrap().is_empty());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn yarim_yazilan_nesne_listelenmez() {
        let (store, dir) = gecici_depo();

        store.put("raw/a.mp4", b"tam").unwrap();
        std::fs::write(dir.join("raw").join("b.mp4.part"), b"yarim").unwrap();

        let keys = store.list("raw").unwrap();
        assert_eq!(keys, vec!["raw/a.mp4"]);

        let _ = std::fs::remove_dir_all(dir);
    }
}
