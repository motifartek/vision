//! ffmpeg'den ham gri kare akışı.
//!
//! **Faz 1'de doldurulacak.** Planlanan komut:
//!
//! ```text
//! ffmpeg -v error -i input.mp4 \
//!   -vf "fps=15,scale=160:90,format=gray" \
//!   -f rawvideo -pix_fmt gray -
//! ```
//!
//! Kare başına tam `width * height` bayt gelir; `read_exact` ile blok blok
//! okunur. Çözücü bir `Iterator<Item = Result<AnalysisFrame>>` döndürmeli —
//! tüm videoyu belleğe almadan akıtmak için. **Bellek kullanımı video
//! uzunluğundan bağımsız kalmalı** (KPI).

// Faz 1: pub fn decode_gray(path: &Path, cfg: AnalysisConfig) -> Result<impl Iterator<Item = Result<AnalysisFrame>>>
