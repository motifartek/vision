//! MotifAI ortak çekirdeği.
//!
//! Bu crate ağ, veritabanı veya medya bilmez. Sadece tüm servislerin
//! paylaştığı tipleri, hata yüzeyini ve telemetri kurulumunu barındırır.

pub mod error;
pub mod ids;


pub use error::{Error, Result};
pub use ids::VideoId;
