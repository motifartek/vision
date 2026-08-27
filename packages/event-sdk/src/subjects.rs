//! NATS konu (subject) sabitleri.
//!
//! Konu adları asla elle string olarak yazılmaz; her zaman buradan alınır.

/// Ham video alındı ve nesne deposuna yazıldı.
pub const VIDEO_INGESTED: &str = "stream.video.ingested";

/// Bir grup kare çıkarıldı ve nesne deposuna yazıldı.
pub const FRAME_EXTRACTED: &str = "stream.frame.extracted";

/// AI katmanı bir risk tespit etti; gateway bunu SSE ile panele iletir.
pub const RISK_DETECTED: &str = "event.risk.detected";

/// Stream tool çağrılarının (pass 3) konu öneki. İstek/cevap desenidir.
pub const TOOL_PREFIX: &str = "stream.tool.";

/// Belirli bir tool için tam konu adını üretir.
///
/// ```
/// use motif_event_sdk::subjects;
/// assert_eq!(subjects::tool("zoom_range"), "stream.tool.zoom_range");
/// ```
pub fn tool(name: &str) -> String {
    format!("{TOOL_PREFIX}{name}")
}

/// Toolbox mikroservisi için sanal bir araç çalıştırma komutu
pub const TOOL_EXECUTE: &str = "toolbox.execute";
