//! Video yükleme ve listeleme.

use std::path::{Path, PathBuf};

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

#[derive(Debug, Serialize)]
pub struct VideoEntry {
    /// Uzantısız dosya adı — URL'de `/videos/<id>` olarak kullanılır.
    pub id: String,
    /// Uzantılı dosya adı.
    pub filename: String,
    /// Bayt cinsinden boyut.
    pub size: u64,
}

/// `GET /v1/videos` — medya kökündeki video dosyalarını listeler.
pub async fn list_videos(
    State(state): State<std::sync::Arc<crate::api::AppState>>,
) -> Result<Json<Vec<VideoEntry>>, InferenceError> {
    let root = state.media_root.as_ref().ok_or_else(|| {
        InferenceError::Config(
            "INFERENCE_MEDIA_ROOT ayarli degil; video listesi kullanilamaz".into(),
        )
    })?;

    let mut entries = Vec::new();
    let mut dir = fs::read_dir(root).await.map_err(InferenceError::Io)?;

    while let Some(entry) = dir.next_entry().await.map_err(InferenceError::Io)? {
        let path = entry.path();
        if !path.is_file() || !is_video_file(&path) {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let id = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
        entries.push(VideoEntry { id, filename, size });
    }

    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(Json(entries))
}

/// `POST /v1/upload` — multipart/form-data ile video dosyası yükler.
///
/// Form alanı: `file` (zorunlu). Dosya `INFERENCE_MEDIA_ROOT` altına kaydedilir.
/// Aynı isimde dosya varsa üzerine yazmaz, hata döndürür.
pub async fn upload_video(
    State(state): State<std::sync::Arc<crate::api::AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), InferenceError> {
    let root = state.media_root.as_ref().ok_or_else(|| {
        InferenceError::Config(
            "INFERENCE_MEDIA_ROOT ayarli degil; yukleme kullanilamaz".into(),
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

        // Dosyayı parça parça akıtarak diske yaz (RAM tüketmez, varsa üzerine yazar)
        let mut file = fs::File::create(&dest)
            .await
            .map_err(InferenceError::Io)?;

        let mut total_bytes = 0usize;
        let mut field = field;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| InferenceError::Config(format!("dosya okunurken hata: {e}")))?
        {
            total_bytes += chunk.len();
            file.write_all(&chunk).await.map_err(InferenceError::Io)?;
        }
        file.flush().await.map_err(InferenceError::Io)?;

        let id = Path::new(&safe_name)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(&safe_name)
            .to_string();

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
