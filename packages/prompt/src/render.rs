//! Yer tutucu doldurma ve parça birleştirme.
//!
//! Genel amaçlı bir şablon motoru yok (tasarım §K5). Yalnızca sabit bir yer
//! tutucu kümesi tanınıyor; tanınmayan biri **hata** — sessiz boş değer değil.

use motif_event_sdk::format_timestamp;

use crate::{PromptContext, PromptError, PromptKind};

/// Tanınan yer tutucular. Bu listenin dışındaki her `{...}` hatadır.
const YER_TUTUCULAR: &[&str] = &["sure", "t0", "t1", "olcek", "isitsel", "onceki", "tools"];

/// Bir metindeki yer tutucuların tanınır olduğunu doğrular.
///
/// Katalog yüklenirken çağrılıyor: bozuk bir yer tutucu açılışta yakalanmalı,
/// analiz sırasında değil.
pub(crate) fn yer_tutuculari_dogrula(fragment: &str, metin: &str) -> Result<(), PromptError> {
    for ad in yer_tutuculari_bul(metin) {
        if !YER_TUTUCULAR.contains(&ad.as_str()) {
            return Err(PromptError::UnknownPlaceholder {
                fragment: fragment.to_string(),
                placeholder: ad,
            });
        }
    }
    Ok(())
}

/// `{ad}` biçimindeki yer tutucuların adlarını çıkarır.
///
/// JSON örnekleri de süslü parantez içeriyor (`{"summary": ...}`), o yüzden
/// yalnızca **harf, rakam ve alt çizgiden** oluşan içerikler yer tutucu
/// sayılıyor. `{"summary"` ya da `{t0_ms: <başlangıç>}` bu süzgece takılmıyor.
fn yer_tutuculari_bul(metin: &str) -> Vec<String> {
    let mut bulunan = Vec::new();
    let baytlar: Vec<char> = metin.chars().collect();
    let mut i = 0;

    while i < baytlar.len() {
        if baytlar[i] != '{' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let mut ad = String::new();
        while j < baytlar.len() && baytlar[j] != '}' {
            ad.push(baytlar[j]);
            j += 1;
        }
        if j < baytlar.len()
            && !ad.is_empty()
            && ad.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            bulunan.push(ad);
        }
        i = j + 1;
    }
    bulunan
}

/// Yer tutucuları bağlamdan doldurur.
pub(crate) fn doldur(
    fragment: &str,
    metin: &str,
    ctx: &PromptContext,
) -> Result<String, PromptError> {
    yer_tutuculari_dogrula(fragment, metin)?;

    let mut cikti = metin.to_string();

    if cikti.contains("{sure}") {
        cikti = cikti.replace("{sure}", &format_timestamp(ctx.duration_ms));
    }
    if cikti.contains("{tools}") {
        let tools_text = ctx.tools.as_deref().unwrap_or("Aktif bir dış araç bulunmamaktadır.");
        cikti = cikti.replace("{tools}", tools_text);
    }
    // Güvenilmez metin ayraçlı bölgeye sarılarak giriyor. Kaçırma
    // `UntrustedText::new` içinde yapıldı; burada yalnız yerleştiriliyor.
    if cikti.contains("{isitsel}") {
        let bolge = ctx
            .audio
            .as_ref()
            .map(|a| a.bolgeye_sar())
            .unwrap_or_default();
        cikti = cikti.replace("{isitsel}", &bolge);
    }
    if cikti.contains("{onceki}") {
        let bolge = ctx
            .prior
            .as_ref()
            .map(|p| p.bolgeye_sar())
            .unwrap_or_default();
        cikti = cikti.replace("{onceki}", &bolge);
    }
    if let Some(clip) = ctx.clip.as_ref() {
        cikti = cikti
            .replace("{t0}", &format_timestamp(clip.t0_ms))
            .replace("{t1}", &format_timestamp(clip.t1_ms))
            // `{:.0}` ile aynı: 8.0 -> "8"
            .replace("{olcek}", &format!("{:.0}", clip.time_scale));
    }

    Ok(cikti)
}

/// Parçaları prompt'a çevirir.
///
/// Ayırıcılar bugünkü metinle **birebir** aynı olacak şekilde seçildi; bu faz
/// saf tekilleştirme, davranış değişmemeli:
///
/// - Parçalar arasında boş satır (`\n\n`)
/// - `agir_cekim` bir öncekine **boşlukla** yapışır: bugünkü kodda ağır çekim
///   cümlesi pencere bilgisinin ardına aynı paragrafta ekleniyor
/// - `sozlesme` kendi başında zaten `\n\n` ile başlıyor, o yüzden araya ayrıca
///   boşluk konmuyor
pub(crate) fn bicimlendir(_kind: PromptKind, parcalar: &[String]) -> String {
    let mut cikti = String::new();

    for parca in parcalar {
        if cikti.is_empty() {
            cikti.push_str(parca);
            continue;
        }
        if parca.starts_with(' ') || parca.starts_with('\n') {
            // Ağır çekim (boşlukla başlar) ve sözleşme (satır sonuyla başlar)
            // kendi ayırıcısını taşıyor.
            cikti.push_str(parca);
        } else {
            cikti.push_str("\n\n");
            cikti.push_str(parca);
        }
    }

    cikti
}

#[cfg(test)]
mod tests {
    use super::*;
    use motif_event_sdk::ClipRef;

    #[test]
    fn json_ornekleri_yer_tutucu_sayilmaz() {
        // Sözleşme metni JSON örneği içeriyor; bunlar yer tutucu değil.
        let metin = r#"{"summary": "x", "events": [{"time": "MM:SS"}]}"#;
        assert!(yer_tutuculari_bul(metin).is_empty());
        assert!(yer_tutuculari_dogrula("sozlesme", metin).is_ok());
    }

    #[test]
    fn acili_parantezli_ornek_de_takilmaz() {
        let metin = r#"{"zoom": {"t0_ms": <başlangıç>, "t1_ms": <bitiş>}}"#;
        assert!(yer_tutuculari_dogrula("sozlesme", metin).is_ok());
    }

    #[test]
    fn gercek_yer_tutucular_bulunur() {
        let ad = yer_tutuculari_bul("Kayıt {sure} uzunluğunda, {t0} – {t1}");
        assert_eq!(ad, vec!["sure", "t0", "t1"]);
    }

    #[test]
    fn taninmayan_yer_tutucu_hata() {
        let e = yer_tutuculari_dogrula("rol", "merhaba {bilinmeyen}").unwrap_err();
        assert!(matches!(e, PromptError::UnknownPlaceholder { .. }));
    }

    #[test]
    fn olcek_tam_sayi_yazilir() {
        let ctx = PromptContext::new(0).with_clip(ClipRef {
            t0_ms: 12_000,
            t1_ms: 15_000,
            object_key: "c".into(),
            duration_ms: 24_000,
            time_scale: 8.0,
            service_frames: 47,
            effective_fps: 16.0,
        });
        let out = doldur("agir_cekim", "{olcek} kat, {t0} – {t1}", &ctx).unwrap();
        assert_eq!(out, "8 kat, 00:12 – 00:15");
    }

    #[test]
    fn kendi_ayiricisi_olan_parca_yapisir() {
        let out = bicimlendir(
            PromptKind::VisionYakinlastirma,
            &["Pencere.".into(), " Ağır çekim.".into(), "Talimat.".into()],
        );
        assert_eq!(out, "Pencere. Ağır çekim.\n\nTalimat.");
    }
}
