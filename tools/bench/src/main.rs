//! `bench` — ölçüm harness'ı.
//!
//! Şartname §4 katılımcıların **kendi metriklerini tanımlamasını** ve
//! sonuçları raporlamasını zorunlu tutuyor. Bu araç o sayıları üretir.
//!
//! Ana metrik **event coverage recall**: ground-truth olayların yüzde kaçının
//! ±1 sn içinde seçilmiş bir karesi var. Değeri şurada: VLM'e hiç dokunmadan
//! örnekleme tarafını ölçer. Recall düşükse sorun örneklemededir, modelde
//! değil — ve tersi. İki başarısızlığı birbirinden ayırır.
//!
//! Faz 4'te doldurulacak. Yol haritası:
//! `documents/architecture/stream-phase-plan.md`

use anyhow::Result;

fn main() -> Result<()> {
    println!("bench: Faz 4'te uygulanacak (event coverage recall, α süpürmesi, baseline karşılaştırması)");
    Ok(())
}
