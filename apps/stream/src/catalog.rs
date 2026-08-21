//! Video kaydı: yüklenmiş videoların kalıcı kütüğü.
//!
//! Kayıtlar nesne deposunda `meta/<id>.json` olarak durur. Bellekte ayrı bir
//! kütük tutulmuyor: servis yeniden başladığında test arayüzü yüklenen
//! videoları kaybetmesin diye tek doğruluk kaynağı depo.

use chrono::{DateTime, Utc};
use motif_core::{Error, Result, VideoId};
use motif_optics::VideoInfo;
use serde::{Deserialize, Serialize};

use crate::storage::BlobStore;

/// Yüklenmiş bir videonun kütük kaydı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRecord {
    pub id: VideoId,
    /// Kullanıcının yüklediği dosya adı. Yalnızca gösterim için.
    pub original_name: String,
    /// Ham videonun nesne anahtarı.
    pub object_key: String,
    pub info: VideoInfo,
    pub uploaded_at: DateTime<Utc>,
}

impl VideoRecord {
    fn meta_key(id: &VideoId) -> String {
        format!("meta/{id}.json")
    }

    pub fn save(&self, store: &dyn BlobStore) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        store.put(&Self::meta_key(&self.id), &bytes)
    }

    pub fn load(store: &dyn BlobStore, id: &VideoId) -> Result<Self> {
        let bytes = store.get(&Self::meta_key(id))?;
        serde_json::from_slice(&bytes).map_err(Error::from)
    }

    pub fn exists(store: &dyn BlobStore, id: &VideoId) -> bool {
        store.exists(&Self::meta_key(id))
    }
}

/// Kütükteki tüm videolar, en yeniden eskiye.
pub fn list(store: &dyn BlobStore) -> Result<Vec<VideoRecord>> {
    let mut records = Vec::new();

    for key in store.list("meta")? {
        // Bozuk ya da yarım bir kayıt tüm listelemeyi düşürmemeli.
        match store
            .get(&key)
            .and_then(|b| serde_json::from_slice::<VideoRecord>(&b).map_err(Error::from))
        {
            Ok(record) => records.push(record),
            Err(err) => tracing::warn!(%key, %err, "kütük kaydı okunamadı, atlanıyor"),
        }
    }

    records.sort_by_key(|r| std::cmp::Reverse(r.uploaded_at));
    Ok(records)
}

/// Bir videoya ait tüm nesneleri siler.
pub fn delete(store: &dyn BlobStore, id: &VideoId) -> Result<()> {
    for key in store.list(&format!("frames/{id}")).unwrap_or_default() {
        let _ = store.delete(&key);
    }
    let _ = store.delete(&id.profile_object_key());

    if let Ok(record) = VideoRecord::load(store, id) {
        let _ = store.delete(&record.object_key);
    }

    store.delete(&VideoRecord::meta_key(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalStore;

    fn depo() -> (LocalStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("motif-catalog-{}", uuid::Uuid::new_v4()));
        (LocalStore::new(&dir).unwrap(), dir)
    }

    fn kayit(ad: &str) -> VideoRecord {
        let id = VideoId::new();
        VideoRecord {
            object_key: id.raw_object_key("mp4"),
            id,
            original_name: ad.into(),
            info: VideoInfo {
                duration_ms: 20_000,
                fps: 30.0,
                width: 640,
                height: 360,
                size_bytes: 1024,
                codec: "h264".into(),
            },
            uploaded_at: Utc::now(),
        }
    }

    #[test]
    fn kayit_yazilir_ve_geri_okunur() {
        let (store, dir) = depo();
        let r = kayit("saha.mp4");

        r.save(&store).unwrap();
        assert!(VideoRecord::exists(&store, &r.id));

        let geri = VideoRecord::load(&store, &r.id).unwrap();
        assert_eq!(geri.id, r.id);
        assert_eq!(geri.original_name, "saha.mp4");
        assert_eq!(geri.info.fps, 30.0);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn listeleme_en_yeniden_eskiye_sirali() {
        let (store, dir) = depo();

        let mut eski = kayit("eski.mp4");
        eski.uploaded_at = Utc::now() - chrono::Duration::hours(2);
        eski.save(&store).unwrap();

        let yeni = kayit("yeni.mp4");
        yeni.save(&store).unwrap();

        let hepsi = list(&store).unwrap();
        assert_eq!(hepsi.len(), 2);
        assert_eq!(hepsi[0].original_name, "yeni.mp4");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bozuk_kayit_listelemeyi_dusurmez() {
        let (store, dir) = depo();

        kayit("saglam.mp4").save(&store).unwrap();
        store.put("meta/bozuk.json", b"{ bu gecerli json degil").unwrap();

        let hepsi = list(&store).unwrap();
        assert_eq!(hepsi.len(), 1, "bozuk kayıt atlanmalı, hata verilmemeli");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn silme_tum_nesneleri_temizler() {
        let (store, dir) = depo();
        let r = kayit("saha.mp4");
        r.save(&store).unwrap();
        store.put(&r.object_key, b"video").unwrap();
        store.put(&r.id.frame_object_key(1000), b"kare").unwrap();
        store.put(&r.id.profile_object_key(), b"{}").unwrap();

        delete(&store, &r.id).unwrap();

        assert!(!VideoRecord::exists(&store, &r.id));
        assert!(!store.exists(&r.object_key));
        assert!(!store.exists(&r.id.frame_object_key(1000)));
        assert!(!store.exists(&r.id.profile_object_key()));

        let _ = std::fs::remove_dir_all(dir);
    }
}
