//! Prompt kataloğu.
//!
//! Modele giden her metnin **tek doğruluk kaynağı**. Tasarım gerekçeleri
//! `documents/architecture/prompt-system.md` içinde; burada yalnızca uygulama.
//!
//! # Neden bir crate
//!
//! Prompt'lar bu projede yük taşıyor: metin değişiklikleri dört kez doğru/yanlış
//! farkı yarattı (olay zamanının `MM:SS` istenmesi, kamera saati kuralı, ağır
//! çekim formülünün işe yaramaması, şemanın isteme taşınması). Buna rağmen iki
//! ayrı yerde, sürümsüz ve ölçümsüz duruyorlardı — `apps/stream/src/payload.rs`
//! panelde başka bir metin gösterirken `apps/ai/vision/src/agent.rs` modele
//! başkasını gönderiyordu.
//!
//! # Tasarım kuralları
//!
//! - Katalog ikiliye gömülü: depo doğruluk kaynağı, jüri klonlayınca aynı
//!   prompt'la çalışır.
//! - Bağlam **tipli**; string anahtarlı map yok. Eksik anahtar çalışma anında
//!   sessizce boş string üretir ve kimse fark etmez.
//! - Genel şablon dili yok. Koşullar ve sıralama burada, Rust'ta; TOML yalnız
//!   metin taşır.
//! - [`PromptRegistry::render`] **hata döndürmez**. Prompt üretimi analizi
//!   düşüremez.

use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

mod context;
mod render;

pub use context::{PromptContext, UntrustedText};

/// Gömülü katalog. Derleme zamanında ikiliye giriyor.
const VISION_TEMPLATE: &str = include_str!("../templates/vision.toml");

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("katalog ayrıştırılamadı ({agent}): {source}")]
    Parse {
        agent: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("{agent} kataloğunda '{fragment}' parçası yok")]
    MissingFragment { agent: String, fragment: String },
    #[error("'{fragment}' parçasında tanınmayan yer tutucu: {{{placeholder}}}")]
    UnknownPlaceholder {
        fragment: String,
        placeholder: String,
    },
}

/// Hangi prompt isteniyor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// Kaydın tamamına ilk bakış.
    VisionIlkBakis,
    /// Ajanın istediği aralığın klibi.
    VisionYakinlastirma,
}

impl PromptKind {
    pub fn agent(self) -> &'static str {
        match self {
            PromptKind::VisionIlkBakis | PromptKind::VisionYakinlastirma => "vision",
        }
    }
}

/// Katalogdaki tek parça.
#[derive(Debug, Clone, Deserialize)]
pub struct Fragment {
    /// Arayüzden değiştirilebilir mi.
    ///
    /// `false` olanlar koda bağlıdır — `sozlesme` gibi. Ayrıştırıcı onları
    /// bekliyor; değiştirilirse şartnamenin puanladığı çıktı kırılır.
    #[serde(default)]
    pub editable: bool,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Meta {
    agent: String,
    version: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    meta: Meta,
    fragment: BTreeMap<String, Fragment>,
}

/// Prompt'un nereden geldiği. İzlenebilirlik için raporda taşınıyor.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PromptSource {
    /// Gömülü katalog.
    Embedded,
    /// Veritabanı override'ı (Faz 6).
    Override { id: String, author: String },
}

/// Üretilen prompt'un kimliği.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptVersion {
    pub agent: String,
    pub number: u32,
    /// Render edilmiş metnin içerik özeti. Override'lar da buna yansır, yani
    /// iki koşunun aynı metinle çalışıp çalışmadığı buradan anlaşılır.
    pub hash: String,
    pub source: PromptSource,
}

/// Modele gidecek hâli.
///
/// İki parça: `prefix` videodan **önce**, `suffix` videodan **sonra**.
/// Ayrım ön ek önbelleği için (bkz. tasarım §K8) — sabit metin önde kalırsa
/// aynı klip üzerinden tekrarlı sorular önbelleğe isabet eder.
///
/// Ön ek yer tutucu taşımaz; kayda özgü her şey son ektedir. Bu ayrım
/// olmadan önbellek her videoda ıskalıyordu.
#[derive(Debug, Clone)]
pub struct RenderedPrompt {
    pub prefix: String,
    pub suffix: String,
    pub version: PromptVersion,
}

impl RenderedPrompt {
    /// Tek metin hâli.
    ///
    /// Son ek boşken bugünkü tek parçalı isteğin aynısını verir.
    pub fn joined(&self) -> String {
        if self.suffix.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}\n\n{}", self.prefix, self.suffix)
        }
    }
}

/// Katalog + (ileride) override deposu.
pub struct PromptRegistry {
    catalogs: BTreeMap<String, Catalog>,
}

impl PromptRegistry {
    /// Gömülü katalogları yükler.
    ///
    /// Bozuk katalog **açılışta** hata verir; servis kalkmaz. Sessizce bozuk
    /// prompt göndermekten iyidir.
    pub fn embedded() -> Result<Self, PromptError> {
        // Ajan başına bir katalog. Şimdilik yalnız vision; orchestrator
        // eklendiğinde bu listeye girecek.
        let mut catalogs = BTreeMap::new();
        let katalog: Catalog =
            toml::from_str(VISION_TEMPLATE).map_err(|source| PromptError::Parse {
                agent: "vision".into(),
                source,
            })?;
        catalogs.insert(katalog.meta.agent.clone(), katalog);

        let registry = Self { catalogs };
        registry.dogrula()?;
        Ok(registry)
    }

    /// Her parçanın yer tutucularının tanınır olduğunu açılışta doğrular.
    fn dogrula(&self) -> Result<(), PromptError> {
        for katalog in self.catalogs.values() {
            for (ad, parca) in &katalog.fragment {
                render::yer_tutuculari_dogrula(ad, &parca.text)?;
            }
        }
        Ok(())
    }

    /// Bir ajanın parçalarını verir. Arayüz ve doğrulama için.
    pub fn fragments(&self, agent: &str) -> Option<&BTreeMap<String, Fragment>> {
        self.catalogs.get(agent).map(|k| &k.fragment)
    }

    fn parca(&self, agent: &str, ad: &str) -> Result<&Fragment, PromptError> {
        self.catalogs
            .get(agent)
            .and_then(|k| k.fragment.get(ad))
            .ok_or_else(|| PromptError::MissingFragment {
                agent: agent.into(),
                fragment: ad.into(),
            })
    }

    /// Prompt'u üretir.
    ///
    /// Hata döndürmez: eksik parça ya da render sorunu loglanır ve o parça
    /// atlanır. Prompt üretimi analizi düşüremez.
    pub fn render(&self, kind: PromptKind, ctx: &PromptContext) -> RenderedPrompt {
        let agent = kind.agent();

        // Hangi parça, hangi sırada, hangi tarafta. Koşullar burada —
        // şablonda değil (§K5).
        //
        // Ön ek videodan önce gider ve **yer tutucu taşımaz**: ön ek önbelleği
        // ancak birebir aynı kalırsa isabet ediyor. Kayda özgü her şey son eke.
        let (on_ek, son_ek): (Vec<&str>, Vec<&str>) = match kind {
            PromptKind::VisionIlkBakis => (
                vec!["rol", "zaman_kurali", "sozlesme"],
                vec!["kayit_bilgisi"],
            ),
            PromptKind::VisionYakinlastirma => {
                let mut son = vec!["pencere_bilgisi"];
                if ctx.agir_cekimde() {
                    son.push("agir_cekim");
                }
                (
                    vec![
                        "yakinlastirma_talimati",
                        "yakinlastirma_zaman_kurali",
                        "sozlesme",
                    ],
                    son,
                )
            }
        };

        let prefix = self.birlestir(agent, &on_ek, ctx, kind);
        let suffix = self.birlestir(agent, &son_ek, ctx, kind);

        let version = PromptVersion {
            agent: agent.to_string(),
            number: self
                .catalogs
                .get(agent)
                .map(|k| k.meta.version)
                .unwrap_or(0),
            // Özet ikisini birden kapsıyor: iki koşunun aynı metinle çalışıp
            // çalışmadığı buradan anlaşılmalı, yalnız ön ekten değil.
            hash: ozet(&format!("{prefix}\u{1e}{suffix}")),
            source: PromptSource::Embedded,
        };

        RenderedPrompt {
            prefix,
            suffix,
            version,
        }
    }

    fn birlestir(
        &self,
        agent: &str,
        adlar: &[&str],
        ctx: &PromptContext,
        kind: PromptKind,
    ) -> String {
        let mut parcalar: Vec<String> = Vec::new();

        for &ad in adlar {
            let parca = match self.parca(agent, ad) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(hata = %e, "prompt parçası atlandı");
                    continue;
                }
            };
            match render::doldur(ad, &parca.text, ctx) {
                Ok(metin) => parcalar.push(metin),
                Err(e) => tracing::error!(hata = %e, "prompt parçası render edilemedi"),
            }
        }

        render::bicimlendir(kind, &parcalar)
    }
}

fn ozet(metin: &str) -> String {
    let mut h = Sha256::new();
    h.update(metin.as_bytes());
    format!("{:x}", h.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gomulu_katalog_yuklenir() {
        let r = PromptRegistry::embedded().expect("gömülü katalog geçerli olmalı");
        let parcalar = r.fragments("vision").expect("vision kataloğu");
        assert!(parcalar.contains_key("rol"));
        assert!(parcalar.contains_key("sozlesme"));
    }

    #[test]
    fn sozlesme_duzenlenemez_isaretli() {
        // §K3: şema koda bağlı; arayüzden değiştirilirse çıktı kırılır.
        let r = PromptRegistry::embedded().unwrap();
        let s = &r.fragments("vision").unwrap()["sozlesme"];
        assert!(!s.editable, "sözleşme düzenlenebilir işaretlenmiş");
    }

    #[test]
    fn surum_ozeti_metne_bagli() {
        let r = PromptRegistry::embedded().unwrap();
        let a = r.render(PromptKind::VisionIlkBakis, &PromptContext::new(35_000));
        let b = r.render(PromptKind::VisionIlkBakis, &PromptContext::new(60_000));
        assert_ne!(a.version.hash, b.version.hash, "farklı metin, farklı özet");

        let c = r.render(PromptKind::VisionIlkBakis, &PromptContext::new(35_000));
        assert_eq!(a.version.hash, c.version.hash, "aynı metin, aynı özet");
    }

    /// Ölçülmüş hata: model kameranın bastığı "14:26:11" saatini yazıyordu.
    /// Kural prompt'tan düşerse aynı hata geri gelir.
    #[test]
    fn kamera_saati_kurali_her_istemde_var() {
        let r = PromptRegistry::embedded().unwrap();

        let genel = r.render(PromptKind::VisionIlkBakis, &PromptContext::new(35_000));
        assert!(genel
            .joined()
            .contains("Kameranın görüntü üzerine bastığı saati"));

        let yakin = r.render(
            PromptKind::VisionYakinlastirma,
            &PromptContext::new(35_000).with_clip(test_clip(8.0)),
        );
        // Yakınlaştırmada kural farklı: zamanlar klibin saatiyle isteniyor.
        assert!(yakin.joined().contains("BU KLİBİN başından itibaren"));
    }

    /// Ağır çekim uyarısı yalnız gerçekten yavaşlatılmış klipte çıkmalı.
    ///
    /// Ölçülmüştü: modele dönüşüm formülü verilse bile aritmetiği yapmıyor,
    /// o yüzden "kaynak kayda çevirmeye çalışma" cümlesi kritik.
    #[test]
    fn agir_cekim_uyarisi_kosullu() {
        let r = PromptRegistry::embedded().unwrap();

        let yavas = r
            .render(
                PromptKind::VisionYakinlastirma,
                &PromptContext::new(35_000).with_clip(test_clip(8.0)),
            )
            .joined();
        assert!(yavas.contains("ağır çekimde"));
        assert!(yavas.contains("çevirmeye çalışma"));

        let normal = r
            .render(
                PromptKind::VisionYakinlastirma,
                &PromptContext::new(35_000).with_clip(test_clip(1.0)),
            )
            .joined();
        assert!(!normal.contains("ağır çekimde"));
    }

    /// Şartnamenin dört anahtarı istemde tarif edilmeli; ayrıştırıcı bunu
    /// bekliyor ve düşerse çıktının kendisi kırılır.
    #[test]
    fn sozlesme_dort_anahtari_tarif_ediyor() {
        let r = PromptRegistry::embedded().unwrap();
        let metin = r
            .render(PromptKind::VisionIlkBakis, &PromptContext::new(35_000))
            .joined();
        for anahtar in ["summary", "events", "risk", "actions"] {
            assert!(metin.contains(anahtar), "{anahtar} istemde yok");
        }
    }

    fn test_clip(scale: f32) -> motif_event_sdk::ClipRef {
        motif_event_sdk::ClipRef {
            t0_ms: 12_000,
            t1_ms: 15_000,
            object_key: "clips/x.mp4".into(),
            duration_ms: (3_000.0 * scale) as u64,
            time_scale: scale,
            service_frames: 47,
            effective_fps: 2.0 * scale as f64,
        }
    }

    /// Ön ek önbelleğinin tek şartı: ön ek her çağrıda **aynı** olmalı.
    #[test]
    fn on_ek_videodan_bagimsiz() {
        let r = PromptRegistry::embedded().unwrap();
        let a = r.render(PromptKind::VisionIlkBakis, &PromptContext::new(35_000));
        let b = r.render(PromptKind::VisionIlkBakis, &PromptContext::new(600_000));

        assert_eq!(a.prefix, b.prefix, "ön ek videoya göre değişiyor");
        assert_ne!(a.suffix, b.suffix, "süre son ekte taşınmalı");
    }

    /// Ön ekte yer tutucu kalırsa önbellek her videoda ıskalar.
    #[test]
    fn on_ekte_yer_tutucu_yok() {
        let r = PromptRegistry::embedded().unwrap();
        let ctx = PromptContext::new(35_000).with_clip(test_clip(8.0));

        for kind in [PromptKind::VisionIlkBakis, PromptKind::VisionYakinlastirma] {
            let p = r.render(kind, &ctx);
            for yt in ["{sure}", "{t0}", "{t1}", "{olcek}"] {
                assert!(!p.prefix.contains(yt), "{kind:?} ön ekinde {yt} kalmış");
            }
        }
    }

    /// Yakınlaştırmada kayda özgü değerler son ekte olmalı.
    #[test]
    fn yakinlastirma_degerleri_son_ekte() {
        let r = PromptRegistry::embedded().unwrap();
        let p = r.render(
            PromptKind::VisionYakinlastirma,
            &PromptContext::new(35_000).with_clip(test_clip(8.0)),
        );
        assert!(p.suffix.contains("00:12"), "pencere son ekte değil");
        assert!(p.suffix.contains("8 kat"), "ağır çekim oranı son ekte değil");
        assert!(p.prefix.contains("Yalnızca JSON"), "sözleşme ön ekte olmalı");
    }
}
