//! Video yükleme ve listeleme.

use std::path::{Path, PathBuf};

use axum::extract::multipart::Field;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::error::InferenceError;

/// Dosya adını güvenli hâle getirir: yalnız alfanümerik, `-`, `_` ve `.` kalır.
/// Path traversal (`../`) imkânsız hâle gelir.
fn sanitize_filename(raw: &str) -> String {
    let name = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video");

    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Medya kökünden video uzantılı dosyaları listeler.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "flv", "wmv", "m4v"];

fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Uzantısız kimlikten gerçek dosyayı bulur: `test3` → `test3.mkv`.
///
/// Rota kimliği (`/videos/<id>`) uzantı taşımıyor. Eskiden hem dashboard hem
/// gateway buna `.mp4` ekliyordu; mp4 dışında yüklenen her video listede
/// görünüp açılınca kırılıyordu. Tek doğru eşleme dosya sisteminde, o yüzden
/// burada.
///
/// Aynı kimliğe birden çok uzantı denk gelirse `VIDEO_EXTENSIONS` sırası karar
/// verir — ama yükleme bu durumu baştan reddediyor, yani normalde oluşmaz.
pub fn find_by_id(root: &Path, id: &str) -> Option<PathBuf> {
    let mut best: Option<(usize, PathBuf)> = None;

    // Kök ve **doğrudan altındaki dizinler** taranıyor. Eskiden yalnız kök
    // okunuyordu; medya kökü panelin düz `public/media` dizini olduğu sürece
    // bu yetiyordu. Kök `stream`in deposuna çevrilince dosyalar `raw/` altına
    // indi ve çıplak kimlikle gelen her istek — gateway'in `audio-events` ucu
    // dahil — "medya dosyası bulunamadı" almaya başladı.
    //
    // Tek seviye bilinçli: özyineleme, kökün altındaki her şeyi tarama
    // maliyetine ve beklenmedik dizinlere girme riskine sokardı.
    let mut dirs = vec![root.to_path_buf()];
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    for entry in dirs.iter().filter_map(|d| std::fs::read_dir(d).ok()).flatten().flatten() {
        let path = entry.path();
        if !path.is_file() || !is_video_file(&path) {
            continue;
        }
        if path.file_stem().and_then(|s| s.to_str()) != Some(id) {
            continue;
        }
        let rank = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|e| VIDEO_EXTENSIONS.iter().position(|v| *v == e.to_ascii_lowercase()))
            .unwrap_or(usize::MAX);

        if best.as_ref().map(|(r, _)| rank < *r).unwrap_or(true) {
            best = Some((rank, path));
        }
    }

    best.map(|(_, path)| path)
}

fn stem_of(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(name)
        .to_string()
}

#[derive(Debug, Serialize)]
pub struct VideoEntry {
    /// Uzantısız dosya adı — URL'de `/videos/<id>` olarak kullanılır.
    pub id: String,
    /// Uzantılı dosya adı.
    pub filename: String,
    /// Bayt cinsinden boyut.
    pub size: u64,
    /// Kapsayıcı başlığından okunan süre; okunamazsa `null`.
    pub duration_sec: Option<f32>,
}

fn entry_of(path: &Path) -> Option<VideoEntry> {
    Some(VideoEntry {
        id: path.file_stem()?.to_str()?.to_string(),
        filename: path.file_name()?.to_str()?.to_string(),
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        duration_sec: crate::audio::decode::probe_duration(path),
    })
}

/// `GET /v1/videos` — medya kökündeki video dosyalarını listeler.
pub async fn list_videos(
    State(state): State<std::sync::Arc<crate::api::AppState>>,
) -> Result<Json<Vec<VideoEntry>>, InferenceError> {
    let root = state.media_root.as_ref().ok_or_else(|| {
        InferenceError::Config(
            "SONIC_MEDIA_ROOT ayarli degil; video listesi kullanilamaz".into(),
        )
    })?;

    // Süre okuması dosya başına birkaç milisaniyelik senkron başlık okuması;
    // listeleme tamamen bloke eden havuzda koşuyor ki çalışan iş parçacığı
    // takılmasın.
    let root = root.clone();
    let entries = tokio::task::spawn_blocking(move || -> Result<Vec<VideoEntry>, std::io::Error> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&root)? {
            let path = entry?.path();
            if !path.is_file() || !is_video_file(&path) {
                continue;
            }
            if let Some(entry) = entry_of(&path) {
                entries.push(entry);
            }
        }
        entries.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(entries)
    })
    .await
    .map_err(|e| InferenceError::Config(format!("listeleme görevi tamamlanamadı: {e}")))?
    .map_err(InferenceError::Io)?;

    Ok(Json(entries))
}

/// `GET /v1/videos/:id` — uzantısız kimlikten dosya bilgisini döndürür.
///
/// Detay sayfası dosya adını buradan öğreniyor: hem `<video>` kaynağı hem de
/// çözümleme isteği gerçek uzantıyı taşımak zorunda.
pub async fn get_video(
    State(state): State<std::sync::Arc<crate::api::AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<VideoEntry>, InferenceError> {
    let root = state.media_root.as_ref().ok_or_else(|| {
        InferenceError::Config(
            "SONIC_MEDIA_ROOT ayarli degil; video listesi kullanilamaz".into(),
        )
    })?;

    let path = find_by_id(root, &id).ok_or_else(|| InferenceError::MediaNotFound(id.clone()))?;
    let entry = tokio::task::spawn_blocking(move || entry_of(&path))
        .await
        .map_err(|e| InferenceError::Config(format!("okuma görevi tamamlanamadı: {e}")))?
        .ok_or(InferenceError::MediaNotFound(id))?;

    Ok(Json(entry))
}

/// `DELETE /v1/videos/:id` — videoyu medya kökünden siler.
///
/// Yanlış yüklenen ya da bozuk bir dosyayı temizlemenin tek yolu dosya
/// sistemine gitmekti. Servis kimlik doğrulaması taşımadığı için bu uç nokta
/// yalnız 127.0.0.1'den ve dashboard origin'inden erişilebilir (bkz. `main.rs`
/// içindeki CORS kısıtı); dışarı açılacaksa gateway'in arkasına konmalı.
pub async fn delete_video(
    State(state): State<std::sync::Arc<crate::api::AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, InferenceError> {
    let root = state.media_root.as_ref().ok_or_else(|| {
        InferenceError::Config("SONIC_MEDIA_ROOT ayarli degil; silme kullanilamaz".into())
    })?;

    let path = find_by_id(root, &id).ok_or_else(|| InferenceError::MediaNotFound(id.clone()))?;
    fs::remove_file(&path).await.map_err(InferenceError::Io)?;
    tracing::info!(dosya = %path.display(), "video silindi");

    Ok(StatusCode::NO_CONTENT)
}

/// Alanı diske akıtır ve yazılan bayt sayısını döndürür.
///
/// `limit` 0 ise sınırsız — yükleme belleğe alınmadan akıtıldığı için 10 GB'lık
/// bir dosya da sorun değil ve "boyut sınırı yoktur" belgelenmiş bir davranış.
/// Yine de diski dolduran bir isteğin servisi kilitlemesini istemeyen kurulumlar
/// `SONIC_MAX_UPLOAD_BYTES` ile tavan koyabilsin.
async fn stream_to_file(
    mut field: Field<'_>,
    dest: &Path,
    limit: u64,
) -> Result<u64, InferenceError> {
    let mut file = fs::File::create(dest).await.map_err(InferenceError::Io)?;
    let mut total: u64 = 0;

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| InferenceError::Config(format!("dosya okunurken hata: {e}")))?
    {
        total += chunk.len() as u64;
        if limit > 0 && total > limit {
            // Gövde bitmeden yanıt dönmek bağlantıyı sıfırlıyor: istemci 413
            // yerine "ağ hatası" görüyor (ölçüldü, curl `HTTP 000`). Kalanı
            // diske yazmadan yutup düzgün bir yanıt dönüyoruz — bant genişliği
            // harcanıyor ama korunmak istenen şey disk, ve hata okunur oluyor.
            while field.chunk().await.ok().flatten().is_some() {}
            return Err(InferenceError::UploadTooLarge(limit));
        }
        file.write_all(&chunk).await.map_err(InferenceError::Io)?;
    }
    file.flush().await.map_err(InferenceError::Io)?;

    Ok(total)
}

/// `POST /v1/upload` — multipart/form-data ile video dosyası yükler.
///
/// Form alanı: `file` (zorunlu). Dosya `SONIC_MEDIA_ROOT` altına kaydedilir;
/// aynı **adlı** dosya varsa üzerine yazılır (aynı videoyu yeniden yüklemek
/// bağlantıyı koparmasın diye bilinçli), aynı **kimliği** farklı uzantıyla
/// kullanan bir dosya varsa istek reddedilir.
///
/// Yalnız `VIDEO_EXTENSIONS` içindeki uzantılar kabul edilir. Bu bir biçim
/// tercihinden fazlası: medya kökü aynı zamanda Next.js'in statik kökü
/// (`apps/dashboard/public/media`), yani buraya yazılan bir `.html` dosyası
/// dashboard'un **kendi origin'inde** `text/html` olarak servis ediliyordu —
/// doğrudan depolanmış XSS yolu (ölçüldü, 200 döndü).
pub async fn upload_video(
    State(state): State<std::sync::Arc<crate::api::AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), InferenceError> {
    let root = state.media_root.as_ref().ok_or_else(|| {
        InferenceError::Config(
            "SONIC_MEDIA_ROOT ayarli degil; yukleme kullanilamaz".into(),
        )
    })?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| InferenceError::Config(format!("multipart okuma hatasi: {e}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name != "file" {
            continue;
        }

        let original_name = field
            .file_name()
            .unwrap_or("video.mp4")
            .to_string();
        let safe_name = sanitize_filename(&original_name);

        if safe_name.is_empty() || safe_name == "." {
            return Err(InferenceError::Config("gecersiz dosya adi".into()));
        }

        let dest: PathBuf = root.join(&safe_name);
        if !is_video_file(&dest) {
            return Err(InferenceError::UnsupportedUpload(safe_name));
        }

        // Rota kimliği uzantısız olduğu için `film.mp4` ve `film.mkv` aynı
        // `/videos/film` adresine düşer; hangisinin açılacağı belirsiz kalırdı.
        let id = stem_of(&safe_name);
        if let Some(existing) = find_by_id(root, &id) {
            let existing_name = existing
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            if existing_name != safe_name {
                return Err(InferenceError::IdConflict(id, existing_name));
            }
        }

        // Önce `.part` dosyasına akıtılır, ancak tamamlanınca yerine konur.
        // Doğrudan hedefe yazmak, kesilen bir yüklemede diskte bozuk bir "video"
        // bırakıyordu: listede normal görünüyor, açılınca `moov atom not found`
        // veriyordu (medya klasöründeki 15 baytlık `test_dummy.mp4` bu yüzden).
        let temp = root.join(format!("{safe_name}.part"));
        let total_bytes = match stream_to_file(field, &temp, state.max_upload_bytes).await {
            Ok(bytes) => bytes,
            Err(err) => {
                let _ = fs::remove_file(&temp).await;
                return Err(err);
            }
        };

        if let Err(err) = fs::rename(&temp, &dest).await {
            let _ = fs::remove_file(&temp).await;
            return Err(InferenceError::Io(err));
        }

        tracing::info!(dosya = %safe_name, boyut = total_bytes, "video yuklendi");

        return Ok((
            StatusCode::CREATED,
            Json(json!({
                "id": id,
                "filename": safe_name,
                "size": total_bytes,
            })),
        ));
    }

    Err(InferenceError::Config(
        "'file' alani bulunamadi; multipart/form-data ile gonderin".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::sanitize_filename;

    #[test]
    fn sanitizes_paths() {
        assert_eq!(sanitize_filename("test.mp4"), "test.mp4");
        assert_eq!(sanitize_filename("my video (1).mp4"), "my_video__1_.mp4");
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("a/b/c.mp4"), "c.mp4");
        assert_eq!(sanitize_filename("türkçe dosya.mp4"), "t_rk_e_dosya.mp4");
    }
}
