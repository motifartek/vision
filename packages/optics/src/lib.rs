//! Görsel işleme ve medya araç takımı.
//!
//! Bu crate ağ, nesne deposu veya mesaj kuyruğu bilmez: **dosya girer,
//! veri çıkar**. Bu sayede `apps/stream` ayağa kaldırılmadan tek başına
//! test edilebilir ve benchmark'lar altyapısız koşar.
//!
//! # Tasarım ilkesi
//!
//! > Boru hattı **nereye bakılacağına** karar verir;
//! > **ne olduğuna** sadece model karar verir.
//!
//! Buradaki hiçbir modül "kaza oldu" demez. Sadece kanıt üretir:
//! nerede hareket var, hangi kareler birbirinin aynısı, hangi anlar
//! sahne değişimi. Yorumu VLM yapar.
//!
//! # Boru hattı
//!
//! 1. [`probe`] — ffprobe ile metadata (Faz 1)
//! 2. [`decode`] — ffmpeg'den ham gri kare akışı (Faz 1)
//! 3. `motion` — kare farkı ile hareket eğrisi (Faz 2)
//! 4. `sample` — hareket ekseninde adaptif örnekleme (Faz 3)

pub mod decode;
pub mod preflight;
pub mod probe;
pub mod types;

pub use preflight::{check_dependencies, ExternalTool};
pub use types::{AnalysisConfig, AnalysisFrame, VideoInfo};
