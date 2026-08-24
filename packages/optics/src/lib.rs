//! Görsel işleme ve medya araç takımı.
//!
//! Bu crate ağ, nesne deposu veya mesaj kuyruğu bilmez: **dosya girer, veri
//! çıkar**. Bu sayede `apps/stream` ayağa kaldırılmadan tek başına test
//! edilebilir ve benchmark'lar altyapısız koşar.
//!
//! # Tasarım ilkesi
//!
//! > Boru hattı **nereye bakılacağına** karar verir;
//! > **ne olduğuna** sadece model karar verir.
//!
//! Buradaki hiçbir modül "kaza oldu" demez. Sadece kanıt üretir: nerede hareket
//! var, hangi kareler birbirinin aynısı, hangi anlar sahne değişimi. Yorumu VLM
//! yapar.
//!
//! # Boru hattı
//!
//! 1. [`probe`] — ffprobe ile metadata
//! 2. [`decode`] — ffmpeg'den ham gri kare akışı
//! 3. [`motion`] — kare farkı ile hareket eğrisi, sahne kesiti, parmak izi
//! 4. [`sample`] — hareket ekseninde adaptif örnekleme
//! 5. [`extract`] — seçilen karelerin tam kalitede çıkarılması
//! 6. [`render`] — hareket eğrisinin SVG görselleştirmesi

pub mod decode;
pub mod extract;
pub mod motion;
pub mod preflight;
pub mod probe;
pub mod render;
pub mod sample;
pub mod types;

pub use decode::{decode_gray, measure_spawn_overhead, GrayFrames};
pub use extract::{extract_gray_at, extract_jpegs, CropBox, ExtractOptions, ExtractedFrame};
pub use motion::{analyze_frames, build_profile, hamming_distance, MotionProfile, MotionSample};
pub use preflight::{check_dependencies, ExternalTool};
pub use probe::probe;
pub use render::{motion_chart, ChartOptions};
pub use sample::{select_frames, SamplingConfig, Selection, SelectedFrame, SelectionReason};
pub use types::{AnalysisConfig, AnalysisFrame, VideoInfo};
