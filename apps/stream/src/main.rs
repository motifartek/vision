//! `apps/stream` — video alma ve dinamik kare örnekleme servisi.
//!
//! Görevi: yüklenen bir videoyu, **güvenlikle ilgili tüm olayları
//! koruyacak şekilde mümkün olan en az kareye** indirmek ve bu yeteneği
//! ajana çağrılabilir araçlar olarak sunmak.
//!
//! Şu an Faz 0 iskeletidir: yalnızca telemetriyi kurar ve harici
//! bağımlılıkları doğrular. Boru hattı Faz 1-3'te `motif-optics` içinde,
//! servis katmanı Faz 6-7'de burada oluşacak.
//!
//! Yol haritası: `documents/architecture/stream-phase-plan.md`

use anyhow::Result;
use motif_optics::check_dependencies;

fn main() -> Result<()> {
    motif_core::telemetry::init("stream=debug,motif_optics=debug");

    // Eksik bir bağımlılık ilk video yüklendiğinde değil, açılışta anlaşılsın.
    let tools = check_dependencies()?;
    for (tool, version) in tools {
        tracing::info!(tool = tool.binary(), %version, "harici bağımlılık hazır");
    }

    tracing::warn!(
        "stream servisi henüz uygulanmadı (Faz 6). \
         Boru hattını denemek için: cargo run -p motif-optics --bin optics -- --help"
    );

    Ok(())
}
