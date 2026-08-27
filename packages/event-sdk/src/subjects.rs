//! NATS konu (subject) sabitleri.
//!
//! Konu adlarÄ± asla elle string olarak yazÄ±lmaz; her zaman buradan alÄ±nÄ±r.

/// Ham video alÄ±ndÄ± ve nesne deposuna yazÄ±ldÄ±.
pub const VIDEO_INGESTED: &str = "stream.video.ingested";

/// Bir grup kare Ã§Ä±karÄ±ldÄ± ve nesne deposuna yazÄ±ldÄ±.
pub const FRAME_EXTRACTED: &str = "stream.frame.extracted";

/// AI katmanÄ± bir risk tespit etti; gateway bunu SSE ile panele iletir.
pub const RISK_DETECTED: &str = "event.risk.detected";

/// Stream tool Ã§aÄŸrÄ±larÄ±nÄ±n (pass 3) konu Ã¶neki. Ä°stek/cevap desenidir.
pub const TOOL_PREFIX: &str = "stream.tool.";

/// Belirli bir tool iÃ§in tam konu adÄ±nÄ± Ã¼retir.
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
