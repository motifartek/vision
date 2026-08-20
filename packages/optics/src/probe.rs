//! ffprobe ile video metadata çıkarma.
//!
//! **Faz 1'de doldurulacak.** Planlanan komut:
//!
//! ```text
//! ffprobe -v error -select_streams v:0 \
//!   -show_entries stream=width,height,r_frame_rate,codec_name \
//!   -show_entries format=duration,size -of json input.mp4
//! ```
//!
//! Dikkat: `r_frame_rate` `"30000/1001"` gibi bir kesir olarak gelir,
//! pay/payda ayrıştırılıp float'a çevrilmelidir.

// Faz 1: pub fn probe(path: &Path) -> Result<VideoInfo>
