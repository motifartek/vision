//! Prompt varyantlarını golden dataset'e karşı ölçer.
//!
//! # Neden
//!
//! Prompt değişiklikleri bu projede dört kez doğru/yanlış farkı yarattı ve
//! her seferinde karar "denedik, iyi göründü" ile verildi. Ölçülemeyen prompt,
//! tahmin edilen prompt'tur.
//!
//! Bu modül aynı golden dataset'i birden çok katalogla koşup sonuçları yan
//! yana koyuyor. Böylece bir metin değişikliği "iyi hissettirdiği" için değil,
//! **olay eşleşmesini artırdığı** için savunulabiliyor.
//!
//! # Ölçüm neyi kapsıyor
//!
//! Ajanın tamamı: klip `stream`'den geliyor, istek gerçekten çıkarım servisine
//! gidiyor, cevap şartname raporuna çevriliyor. Yani ölçülen şey prompt'un
//! **uçtan uca** etkisi, izole bir metin karşılaştırması değil.
//!
//! Bu maliyetli: video başına bir çıkarım isteği, varyant başına tüm küme.
//! `--videos` ile küçültülebiliyor.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use motif_prompt::PromptRegistry;
use vision::agent::VisionAgent;
use vision::stream_client::StreamClient;
use vision::vlm::EvrenProvider;

use crate::dataset::GroundTruth;

/// Bir olayın "yakalandı" sayılması için izin verilen sapma.
///
/// `run` alt komutundaki tolerans ile aynı: model saniyenin altında hassas
/// değil, ama üç saniyeyi aşan sapma yanlış an demek.
const TOLERANS_MS: i64 = 3_000;

/// Tek videonun tek varyanttaki sonucu.
struct Sonuc {
    ad: String,
    eslesen: usize,
    gercek: usize,
    model_olay: usize,
    sema_gecerli: bool,
    aksiyon: usize,
    sapma_ms: Vec<i64>,
    sure_sn: f64,
    hata: Option<String>,
}

/// Bir varyantın tüm küme üzerindeki toplamı.
struct Ozet {
    ad: String,
    prompt_surumu: String,
    sonuclar: Vec<Sonuc>,
}

impl Ozet {
    fn eslesen(&self) -> usize {
        self.sonuclar.iter().map(|s| s.eslesen).sum()
    }
    fn gercek(&self) -> usize {
        self.sonuclar.iter().map(|s| s.gercek).sum()
    }
    fn sema_gecerli(&self) -> usize {
        self.sonuclar.iter().filter(|s| s.sema_gecerli).count()
    }
    fn bos_aksiyon(&self) -> usize {
        self.sonuclar.iter().filter(|s| s.aksiyon == 0).count()
    }
    fn ortalama_sapma(&self) -> f64 {
        let hepsi: Vec<i64> = self.sonuclar.iter().flat_map(|s| s.sapma_ms.clone()).collect();
        if hepsi.is_empty() {
            return 0.0;
        }
        hepsi.iter().map(|d| d.abs() as f64).sum::<f64>() / hepsi.len() as f64
    }
    fn ortalama_sure(&self) -> f64 {
        let calisan: Vec<f64> = self
            .sonuclar
            .iter()
            .filter(|s| s.hata.is_none())
            .map(|s| s.sure_sn)
            .collect();
        if calisan.is_empty() {
            return 0.0;
        }
        calisan.iter().sum::<f64>() / calisan.len() as f64
    }
}

/// `--variants` değerini ayrıştırır: `ad=dizin,ad=dizin`.
///
/// `gomulu` özel: ikiliye gömülü katalog, yani bugünkü davranış. Karşılaştırma
/// her zaman ona karşı yapılmalı.
fn varyantlari_coz(ham: Option<&str>) -> Result<Vec<(String, Option<PathBuf>)>> {
    let Some(ham) = ham else {
        return Ok(vec![("gomulu".to_string(), None)]);
    };

    let mut cikti = Vec::new();
    for parca in ham.split(',') {
        let parca = parca.trim();
        if parca.is_empty() {
            continue;
        }
        match parca.split_once('=') {
            Some((ad, dizin)) => cikti.push((ad.trim().to_string(), Some(PathBuf::from(dizin.trim())))),
            None if parca == "gomulu" => cikti.push(("gomulu".to_string(), None)),
            None => anyhow::bail!(
                "varyant biçimi `ad=dizin` olmalı (ya da `gomulu`): {parca}"
            ),
        }
    }
    if cikti.is_empty() {
        anyhow::bail!("en az bir varyant gerekli");
    }
    Ok(cikti)
}

/// Ground truth dosyalarını yükler.
fn kume_yukle(dizin: &Path, sinir: Option<usize>) -> Result<Vec<(String, GroundTruth)>> {
    let mut kume: Vec<(String, GroundTruth)> = Vec::new();
    for girdi in std::fs::read_dir(dizin)
        .with_context(|| format!("{} okunamadı", dizin.display()))?
        .flatten()
    {
        let yol = girdi.path();
        if yol.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let ad = yol
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        kume.push((ad, GroundTruth::load(&yol)?));
    }
    kume.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some(n) = sinir {
        kume.truncate(n);
    }
    if kume.is_empty() {
        anyhow::bail!("{} içinde ground truth bulunamadı", dizin.display());
    }
    Ok(kume)
}

/// Videoyu `stream`'e yükler ve kimliğini döndürür.
///
/// Zaten yüklüyse yeniden yüklemiyor: ölçüm tekrar tekrar koşuluyor ve her
/// koşuda depoyu şişirmenin anlamı yok.
async fn video_kimligi(
    stream_url: &str,
    dizin: &Path,
    dosya: &str,
) -> Result<String> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()?;

    #[derive(serde::Deserialize)]
    struct Kayit {
        id: String,
        original_name: String,
    }
    #[derive(serde::Deserialize)]
    struct Liste {
        videos: Vec<Kayit>,
    }

    let liste: Liste = http
        .get(format!("{stream_url}/v1/videos"))
        .send()
        .await
        .context("stream servisine ulaşılamadı")?
        .json()
        .await?;

    if let Some(k) = liste.videos.iter().find(|v| v.original_name == dosya) {
        return Ok(k.id.clone());
    }

    let baytlar = std::fs::read(dizin.join(dosya))
        .with_context(|| format!("{dosya} okunamadı"))?;
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(baytlar).file_name(dosya.to_string()),
    );

    #[derive(serde::Deserialize)]
    struct Yuklendi {
        id: String,
    }
    let y: Yuklendi = http
        .post(format!("{stream_url}/v1/videos"))
        .multipart(form)
        .send()
        .await?
        .json()
        .await?;
    Ok(y.id)
}

/// Bir varyantı tüm küme üzerinde koşar.
async fn varyanti_kosl(
    ad: &str,
    katalog: Option<&Path>,
    kume: &[(String, GroundTruth)],
    dataset_dizin: &Path,
    stream_url: &str,
) -> Result<Ozet> {
    let prompts = Arc::new(match katalog {
        Some(d) => PromptRegistry::from_dir(d)
            .with_context(|| format!("{ad} varyantının kataloğu"))?,
        None => PromptRegistry::embedded()?,
    });

    let stream = Arc::new(StreamClient::new(stream_url)?);
    let vlm = Arc::new(EvrenProvider::from_env()?);
    let ajan = VisionAgent::new(stream, vlm, prompts.clone());

    // Sürüm damgası: hangi metinle ölçtüğümüz raporda görünsün.
    let surum = prompts
        .render(
            motif_prompt::PromptKind::VisionIlkBakis,
            &motif_prompt::PromptContext::new(30_000),
        )
        .version;

    let mut sonuclar = Vec::new();

    for (video_ad, gt) in kume {
        let kimlik = match video_kimligi(stream_url, dataset_dizin, &gt.video).await {
            Ok(k) => k,
            Err(e) => {
                sonuclar.push(bos_sonuc(video_ad, gt, format!("yükleme: {e}")));
                continue;
            }
        };

        let basladi = Instant::now();
        match ajan.analyze(&kimlik).await {
            Ok(cikti) => {
                let r = &cikti.report;
                let tahmin: Vec<i64> = r.events.iter().map(|e| e.t_ms as i64).collect();

                let mut eslesen = 0;
                let mut sapma = Vec::new();
                for g in &gt.events {
                    let hedef = g.t_ms as i64;
                    if let Some(en_yakin) = tahmin
                        .iter()
                        .map(|t| t - hedef)
                        .min_by_key(|d| d.abs())
                        .filter(|d| d.abs() <= TOLERANS_MS)
                    {
                        eslesen += 1;
                        sapma.push(en_yakin);
                    }
                }

                sonuclar.push(Sonuc {
                    ad: video_ad.clone(),
                    eslesen,
                    gercek: gt.events.len(),
                    model_olay: r.events.len(),
                    sema_gecerli: sema_gecerli(r),
                    aksiyon: r.actions.len(),
                    sapma_ms: sapma,
                    sure_sn: basladi.elapsed().as_secs_f64(),
                    hata: None,
                });
            }
            Err(e) => sonuclar.push(bos_sonuc(video_ad, gt, e.to_string())),
        }
    }

    Ok(Ozet {
        ad: ad.to_string(),
        prompt_surumu: format!("v{} {}", surum.number, surum.hash),
        sonuclar,
    })
}

fn bos_sonuc(ad: &str, gt: &GroundTruth, hata: String) -> Sonuc {
    Sonuc {
        ad: ad.to_string(),
        eslesen: 0,
        gercek: gt.events.len(),
        model_olay: 0,
        sema_gecerli: false,
        aksiyon: 0,
        sapma_ms: Vec::new(),
        sure_sn: 0.0,
        hata: Some(hata),
    }
}

/// Şartname §5'in dört alanı dolu mu.
///
/// `actions` boş olması ayrıca sayılıyor: şartname §3 aksiyon önerisini açıkça
/// istiyor, boş liste maddeyi karşılamıyor demek.
fn sema_gecerli(r: &motif_event_sdk::AnalysisReport) -> bool {
    !r.summary.trim().is_empty() && !r.actions.is_empty()
}

/// `bench prompts` girişi.
pub fn calistir(
    dataset: &Path,
    variants: Option<&str>,
    videos: Option<usize>,
    export: Option<&Path>,
    stream_url: &str,
) -> Result<()> {
    // Dışa aktarım ölçümden bağımsız: yalnızca metni sabitler.
    if let Some(hedef) = export {
        let r = PromptRegistry::embedded()?;
        std::fs::write(hedef, r.export())
            .with_context(|| format!("{} yazılamadı", hedef.display()))?;
        println!("Katalog dışa aktarıldı: {}", hedef.display());
        if variants.is_none() {
            return Ok(());
        }
    }

    let varyantlar = varyantlari_coz(variants)?;
    let kume = kume_yukle(dataset, videos)?;

    println!(
        "{} video · {} varyant · tolerans ±{} sn\n",
        kume.len(),
        varyantlar.len(),
        TOLERANS_MS / 1000
    );

    let rt = tokio::runtime::Runtime::new()?;
    let mut ozetler = Vec::new();

    for (ad, katalog) in &varyantlar {
        println!("── {ad} ──");
        let ozet = rt.block_on(varyanti_kosl(
            ad,
            katalog.as_deref(),
            &kume,
            dataset,
            stream_url,
        ))?;

        for s in &ozet.sonuclar {
            match &s.hata {
                Some(h) => println!("  {:<24} HATA: {}", s.ad, h.chars().take(60).collect::<String>()),
                None => println!(
                    "  {:<24} olay {}/{}  model {:>2}  aksiyon {}  {:5.1} sn{}",
                    s.ad,
                    s.eslesen,
                    s.gercek,
                    s.model_olay,
                    s.aksiyon,
                    s.sure_sn,
                    if s.sema_gecerli { "" } else { "  ŞEMA EKSİK" }
                ),
            }
        }
        println!();
        ozetler.push(ozet);
    }

    karsilastir(&ozetler);
    Ok(())
}

/// Varyantları yan yana koyar.
fn karsilastir(ozetler: &[Ozet]) {
    println!("{}", "=".repeat(78));
    println!(
        "{:<14} {:>12} {:>10} {:>12} {:>10} {:>12}",
        "varyant", "olay", "şema", "sapma", "boş aksiyon", "süre"
    );
    println!("{}", "-".repeat(78));

    for o in ozetler {
        let oran = if o.gercek() == 0 {
            0
        } else {
            100 * o.eslesen() / o.gercek()
        };
        println!(
            "{:<14} {:>7}/{:<4} {:>6}/{:<3} {:>9.0} ms {:>10} {:>9.1} sn",
            o.ad,
            o.eslesen(),
            o.gercek(),
            o.sema_gecerli(),
            o.sonuclar.len(),
            o.ortalama_sapma(),
            o.bos_aksiyon(),
            o.ortalama_sure(),
        );
        println!("{:<14} %{oran}  ·  prompt {}", "", o.prompt_surumu);
    }
    println!("{}", "=".repeat(78));

    // Fark yorumu: tek varyantta karşılaştırma yok.
    if let (Some(temel), Some(son)) = (ozetler.first(), ozetler.last()) {
        if ozetler.len() > 1 {
            let fark = son.eslesen() as i64 - temel.eslesen() as i64;
            let yorum = match fark {
                0 => "olay eşleşmesi değişmedi".to_string(),
                d if d > 0 => format!("olay eşleşmesi {d} arttı"),
                d => format!("olay eşleşmesi {} azaldı", -d),
            };
            println!("\n{} → {}: {yorum}", temel.ad, son.ad);
            println!(
                "Not: model aynı girdiye koşudan koşuya farklı cevap verebiliyor; \n\
                 tek koşuluk bir fark tesadüf olabilir."
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varyant_bicimi_cozulur() {
        let v = varyantlari_coz(Some("gomulu,v2=/tmp/v2")).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, "gomulu");
        assert!(v[0].1.is_none());
        assert_eq!(v[1].0, "v2");
        assert_eq!(v[1].1.as_deref(), Some(Path::new("/tmp/v2")));
    }

    #[test]
    fn varyant_verilmezse_gomulu() {
        let v = varyantlari_coz(None).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "gomulu");
    }

    #[test]
    fn bozuk_varyant_reddedilir() {
        assert!(varyantlari_coz(Some("saçma")).is_err());
    }
}
