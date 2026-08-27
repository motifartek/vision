//! Video analiz ajanı — kütüphane yüzeyi.
//!
//! Servis `main.rs`'de; ajanın kendisi kütüphane olarak da kullanılabiliyor.
//! Sebebi ölçüm: `tools/bench` prompt varyantlarını karşılaştırırken ajanı
//! doğrudan kuruyor. HTTP üzerinden ölçmek varyant seçimi için servise durum
//! taşımayı gerektirirdi; ölçüm aracının kendi ajanını kurması hem daha
//! dürüst hem daha basit.

pub mod agent;
pub mod api;
pub mod stream_client;
pub mod vlm;
