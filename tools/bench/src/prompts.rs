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

/// Bir varyantın tüm küme üzerindeki **tek koşuluk** toplamı.
///
/// Varyant adı burada tutulmuyor; kimlik `VaryantOzeti`'nde.
struct Ozet {
    prompt_surumu: String,
    sonuclar: Vec<Sonuc>,
}

/// Bir varyantın tekrarlı koşumlarının tamamı.
///
/// # Neden tekrar
///
/// Model aynı girdiye koşudan koşuya farklı cevap veriyor. Tek koşuluk bir
/// fark, değişikliğin etkisi de olabilir modelin oynaklığı da — ikisi tek
/// ölçümle ayrılamıyor. Tekrarlı koşum, **terazinin kendi gürültüsünü**
/// ölçüyor: bir varyantın kendi koşumları arasındaki yayılım, iki varyant
/// arasındaki farkın anlamlı sayılabilmesi için aşılması gereken eşik.
struct VaryantOzeti {
    ad: String,
    prompt_surumu: String,
    kosumlar: Vec<Ozet>,
}

impl VaryantOzeti {
    /// Koşum başına yakalanan olay sayısı.
    fn eslesenler(&self) -> Vec<usize> {
        self.kosumlar.iter().map(|o| o.eslesen()).collect()
    }

    /// Ground truth olay sayısı; koşumlar arasında sabit.
    fn gercek(&self) -> usize {
        self.kosumlar.first().map(|o| o.gercek()).unwrap_or(0)
    }

    fn ortalama_eslesen(&self) -> f64 {
        ortalama(&self.eslesenler().iter().map(|&x| x as f64).collect::<Vec<_>>())
    }

    /// En kötü ve en iyi koşum. Yayılım bu ikisinin farkı.
    fn aralik(&self) -> (usize, usize) {
        let e = self.eslesenler();
        (
            e.iter().copied().min().unwrap_or(0),
            e.iter().copied().max().unwrap_or(0),
        )
    }

    /// Rapor üretemeyen koşum-video sayısı.
    ///
    /// Ayrı sayılıyor çünkü boş cevap ile yanlış cevap aynı şey değil: biri
    /// boru hattının düşmesi, diğeri modelin yanılması.
    fn hatali(&self) -> usize {
        self.kosumlar
            .iter()
            .flat_map(|o| o.sonuclar.iter())
            .filter(|s| s.hata.is_some())
            .count()
    }

    fn toplam_sonuc(&self) -> usize {
        self.kosumlar.iter().map(|o| o.sonuclar.len()).sum()
    }

    fn ortalama_model_olay(&self) -> f64 {
        let hepsi: Vec<f64> = self
            .kosumlar
            .iter()
            .map(|o| o.sonuclar.iter().map(|s| s.model_olay).sum::<usize>() as f64)
            .collect();
        ortalama(&hepsi)
    }

    fn ortalama_sapma(&self) -> f64 {
        ortalama(&self.kosumlar.iter().map(|o| o.ortalama_sapma()).collect::<Vec<_>>())
    }

    fn ortalama_sure(&self) -> f64 {
        ortalama(&self.kosumlar.iter().map(|o| o.ortalama_sure()).collect::<Vec<_>>())
    }

    /// Şartname §5'in dört alanını taşıyan cevap sayısı.
    ///
    /// Recall'dan ayrı tutuluyor: olayları kaçıran ama biçimi doğru bir cevap
    /// ile biçimi bozuk bir cevap aynı şey değil. İkincisi jüri tarafında
    /// ayrıştırılamaz.
    fn sema_gecerli(&self) -> usize {
        self.kosumlar.iter().map(|o| o.sema_gecerli()).sum()
    }

    /// Aksiyon önerisi boş dönen cevap sayısı (şartname §3 bunu istiyor).
    fn bos_aksiyon(&self) -> usize {
        self.kosumlar.iter().map(|o| o.bos_aksiyon()).sum()
    }
}

/// Boş liste "yok" yazar; `- kararlı (0): ` gibi yarım satır kalmasın.
fn liste(v: &[String]) -> String {
    if v.is_empty() {
        "yok".to_string()
    } else {
        v.join(", ")
    }
}

fn ortalama(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().sum::<f64>() / v.len() as f64
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
/// `taze` verilirse mevcut kayıt aranmaz, her zaman yeni yükleme yapılır.
///
/// # Neden taze koşum şart
///
/// Yakınlaştırma bütçesi (`max_zooms_per_video`) **video başına** tutuluyor.
/// Aynı kaydı koşular arasında yeniden kullanmak bütçeyi biriktiriyor: ilk
/// koşu 8 hakla başlıyor, üçüncü koşu `429 zoom_limit_exceeded` alıyor.
///
/// Bu ölçüldü ve tekrarlı koşumun ilk sonucunda görüldü. Koşuları bu hâlde
/// karşılaştırmak yanlış olurdu — sonraki koşum sistematik olarak dezavantajlı
/// başlıyor, yayılım şişiyor ve iki varyant sırayla ölçüldüğünde ikincisi
/// haksız yere kötü görünüyor.
async fn video_kimligi(
    stream_url: &str,
    dizin: &Path,
    dosya: &str,
    taze: bool,
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

    if !taze {
        if let Some(k) = liste.videos.iter().find(|v| v.original_name == dosya) {
            return Ok(k.id.clone());
        }
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

/// Ölçüm için yüklenmiş kaydı siler.
///
/// Taze koşumda her video her koşuda yeniden yükleniyor; temizlenmezse tek bir
/// üç koşuluk ölçüm depoya kümenin üç katını bırakır. Başarısızlık yutuluyor:
/// temizlik yapılamaması ölçümü geçersiz kılmaz.
async fn videoyu_sil(http: &reqwest::Client, stream_url: &str, kimlik: &str) {
    let _ = http
        .delete(format!("{stream_url}/v1/videos/{kimlik}"))
        .send()
        .await;
}

/// Bir varyantı tüm küme üzerinde koşar.
#[allow(clippy::too_many_arguments)]
async fn varyanti_kosl(
    ad: &str,
    katalog: Option<&Path>,
    kume: &[(String, GroundTruth)],
    dataset_dizin: &Path,
    stream_url: &str,
    taze: bool,
    parca_ms: Option<u64>,
) -> Result<Ozet> {
    let prompts = Arc::new(match katalog {
        Some(d) => PromptRegistry::from_dir(d)
            .with_context(|| format!("{ad} varyantının kataloğu"))?,
        None => PromptRegistry::embedded()?,
    });

    let stream = Arc::new(StreamClient::new(stream_url)?);
    let vlm = Arc::new(EvrenProvider::from_env()?);
    let ajan = VisionAgent::new(stream, vlm, prompts.clone()).with_parca_ms(parca_ms);

    // Sürüm damgası: hangi metinle ölçtüğümüz raporda görünsün.
    let surum = prompts
        .render(
            motif_prompt::PromptKind::VisionIlkBakis,
            &motif_prompt::PromptContext::new(30_000),
        )
        .version;

    // Temizlik için: taze koşumda yüklenen kayıtlar koşu sonunda siliniyor.
    let temizlik = reqwest::Client::new();
    let mut sonuclar = Vec::new();

    for (video_ad, gt) in kume {
        let kimlik = match video_kimligi(stream_url, dataset_dizin, &gt.video, taze).await {
            Ok(k) => k,
            Err(e) => {
                sonuclar.push(bos_sonuc(video_ad, gt, format!("yükleme: {e}")));
                continue;
            }
        };

        let basladi = Instant::now();
        match ajan.analyze(&kimlik, None, None).await {
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

        if taze {
            videoyu_sil(&temizlik, stream_url, &kimlik).await;
        }
    }

    let _ = ad;
    Ok(Ozet {
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
#[allow(clippy::too_many_arguments)]
pub fn calistir(
    dataset: &Path,
    variants: Option<&str>,
    videos: Option<usize>,
    export: Option<&Path>,
    stream_url: &str,
    tekrar: usize,
    rapor: Option<&Path>,
    parca_boylari: Option<&str>,
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

    let temel = varyantlari_coz(variants)?;
    let kume = kume_yukle(dataset, videos)?;

    // Parça boyu verilmişse her boy ayrı varyant oluyor. Ayrı `bench`
    // koşularında karşılaştırmak yanlış olurdu: gürültü bandı oturumlar
    // arasında değişiyor (Faz 4'te ölçüldü, 1'e karşı 5 olay).
    let varyantlar: Vec<(String, Option<PathBuf>, Option<u64>)> = match parca_boylari {
        None => temel.into_iter().map(|(a, d)| (a, d, None)).collect(),
        Some(ham) => {
            let mut c = Vec::new();
            for parca in ham.split(',') {
                let ms: u64 = parca
                    .trim()
                    .parse()
                    .with_context(|| format!("parça boyu sayı olmalı: {parca}"))?;
                for (a, d) in &temel {
                    c.push((format!("{a}-parca{}", ms / 1000), d.clone(), Some(ms)));
                }
            }
            c
        }
    };
    let tekrar = tekrar.max(1);

    println!(
        "{} video · {} varyant · {} koşu · tolerans ±{} sn\n",
        kume.len(),
        varyantlar.len(),
        tekrar,
        TOLERANS_MS / 1000
    );

    let rt = tokio::runtime::Runtime::new()?;
    let mut ozetler = Vec::new();

    for (ad, katalog, parca) in &varyantlar {
        println!("── {ad} ──");
        let mut kosumlar = Vec::new();

        for k in 1..=tekrar {
            if tekrar > 1 {
                println!("  koşu {k}/{tekrar}");
            }
            // Tekrarlı ölçümde her koşu taze video ister: yakınlaştırma
            // bütçesi video başına tutuluyor ve koşular arasında birikiyor.
            let ozet = rt.block_on(varyanti_kosl(
                ad,
                katalog.as_deref(),
                &kume,
                dataset,
                stream_url,
                tekrar > 1,
                *parca,
            ))?;

            for s in &ozet.sonuclar {
                match &s.hata {
                    Some(h) => println!(
                        "    {:<24} HATA: {}",
                        s.ad,
                        h.chars().take(58).collect::<String>()
                    ),
                    None => println!(
                        "    {:<24} olay {}/{}  model {:>2}  aksiyon {}  {:5.1} sn{}",
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
            kosumlar.push(ozet);
        }

        let surum = kosumlar
            .first()
            .map(|o| o.prompt_surumu.clone())
            .unwrap_or_default();
        println!();
        ozetler.push(VaryantOzeti {
            ad: ad.clone(),
            prompt_surumu: surum,
            kosumlar,
        });
    }

    karsilastir(&ozetler);

    if let Some(hedef) = rapor {
        let metin = rapor_metni(&ozetler, kume.len(), tekrar);
        if let Some(ana) = hedef.parent() {
            std::fs::create_dir_all(ana).ok();
        }
        std::fs::write(hedef, metin)
            .with_context(|| format!("{} yazılamadı", hedef.display()))?;
        println!("\nRapor yazıldı: {}", hedef.display());
    }

    Ok(())
}

/// Varyantları yan yana koyar.
fn karsilastir(ozetler: &[VaryantOzeti]) {
    println!("{}", "=".repeat(92));
    println!(
        "{:<12} {:>14} {:>10} {:>10} {:>10} {:>9} {:>9} {:>8}",
        "varyant", "olay (ort)", "yayılım", "hata", "şema", "boş aks.", "sapma", "süre"
    );
    println!("{}", "-".repeat(92));

    for o in ozetler {
        let g = o.gercek();
        let oran = if g == 0 {
            0.0
        } else {
            100.0 * o.ortalama_eslesen() / g as f64
        };
        let (en_az, en_cok) = o.aralik();
        println!(
            "{:<12} {:>9.1}/{:<4} {:>6}–{:<3} {:>5}/{:<4} {:>5}/{:<4} {:>9} {:>6.0} ms {:>5.1} sn",
            o.ad,
            o.ortalama_eslesen(),
            g,
            en_az,
            en_cok,
            o.hatali(),
            o.toplam_sonuc(),
            o.sema_gecerli(),
            o.toplam_sonuc(),
            o.bos_aksiyon(),
            o.ortalama_sapma(),
            o.ortalama_sure(),
        );
        println!("{:<12} %{oran:.0}  ·  prompt {}", "", o.prompt_surumu);
    }
    println!("{}", "=".repeat(92));

    // Terazinin gürültüsü: bir varyantın kendi koşumları arasındaki yayılım.
    // İki varyant arasındaki fark bunu aşmıyorsa fark iddia edilemez.
    let esik = ozetler
        .iter()
        .map(|o| {
            let (a, b) = o.aralik();
            b - a
        })
        .max()
        .unwrap_or(0);

    if ozetler.iter().any(|o| o.kosumlar.len() > 1) {
        println!(
            "\nTeraz gürültüsü: bir varyantın kendi koşumları arasında en fazla \
             {esik} olay oynadı."
        );
        println!(
            "Bir değişikliğin etkisi ancak bu yayılımı **aşarsa** iddia edilebilir."
        );
    }

    // Fark yorumu: tek varyantta karşılaştırma yok.
    if let (Some(temel), Some(son)) = (ozetler.first(), ozetler.last()) {
        if ozetler.len() > 1 {
            let fark = son.ortalama_eslesen() - temel.ortalama_eslesen();
            let yorum = if fark.abs() <= esik as f64 {
                format!(
                    "fark {fark:+.1} olay — gürültü bandının ({esik}) içinde, \
                     ANLAMLI DEĞİL"
                )
            } else if fark > 0.0 {
                format!("olay eşleşmesi {fark:+.1} arttı — bant dışı")
            } else {
                format!("olay eşleşmesi {fark:.1} azaldı — bant dışı")
            };
            println!("\n{} → {}: {yorum}", temel.ad, son.ad);
        }
    }
}

/// Commit'lenebilir ölçüm raporu.
///
/// Konsol çıktısı koşuyu yapanda kalıyor; karar bu dosyaya dayanacaksa
/// depoda durmalı. Biçim kasten sade markdown: diff'i okunabilir olsun.
fn rapor_metni(ozetler: &[VaryantOzeti], video_sayisi: usize, tekrar: usize) -> String {
    use std::fmt::Write;
    let mut s = String::new();

    let _ = writeln!(s, "# Prompt ölçümü\n");
    let _ = writeln!(
        s,
        "{video_sayisi} video · {tekrar} koşu · tolerans ±{} sn\n",
        TOLERANS_MS / 1000
    );

    let _ = writeln!(
        s,
        "| varyant | olay (ort/toplam) | yayılım | hata | model olayı | sapma | süre |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|");
    for o in ozetler {
        let (a, b) = o.aralik();
        let _ = writeln!(
            s,
            "| `{}` | {:.1}/{} | {}–{} | {}/{} | {:.1} | {:.0} ms | {:.1} sn |",
            o.ad,
            o.ortalama_eslesen(),
            o.gercek(),
            a,
            b,
            o.hatali(),
            o.toplam_sonuc(),
            o.ortalama_model_olay(),
            o.ortalama_sapma(),
            o.ortalama_sure(),
        );
    }

    let esik = ozetler
        .iter()
        .map(|o| {
            let (a, b) = o.aralik();
            b - a
        })
        .max()
        .unwrap_or(0);

    let _ = writeln!(s, "\n## Terazi gürültüsü\n");
    if tekrar > 1 {
        let _ = writeln!(
            s,
            "Bir varyantın kendi koşumları arasında en fazla **{esik} olay** oynadı. \
             Bir değişikliğin etkisi ancak bu yayılımı aşarsa iddia edilebilir; \
             bandın içindeki fark ölçüm gürültüsüdür."
        );
    } else {
        let _ = writeln!(
            s,
            "Tek koşu yapıldı; yayılım bilinmiyor. Karşılaştırma için `--tekrar 3` \
             kullanın, aksi hâlde farkın gürültü olup olmadığı söylenemez."
        );
    }

    // Toplam yayılım tek bir videodan geliyor olabilir; o zaman "gürültü"
    // aslında o videonun oynaklığıdır ve geri kalan küme karşılaştırılabilir.
    // Bunu görmeden bandı tüm kümeye mal etmek yanıltıcı olurdu.
    if tekrar > 1 {
        let _ = writeln!(s, "\n## Video kararlılığı\n");
        let _ = writeln!(
            s,
            "Koşumlar arasında sonucu değişmeyen videolar karşılaştırmada \
             güvenilir taban; değişenler bandı tek başına açabiliyor.\n"
        );
        for o in ozetler {
            let Some(ilk) = o.kosumlar.first() else { continue };
            let mut kararli = Vec::new();
            let mut oynak = Vec::new();
            let mut hep_hata = Vec::new();

            for (i, temel) in ilk.sonuclar.iter().enumerate() {
                let degerler: Vec<Option<usize>> = o
                    .kosumlar
                    .iter()
                    .map(|k| k.sonuclar.get(i).and_then(|x| x.hata.is_none().then_some(x.eslesen)))
                    .collect();

                if degerler.iter().all(|d| d.is_none()) {
                    hep_hata.push(temel.ad.clone());
                } else if degerler.windows(2).all(|w| w[0] == w[1]) {
                    kararli.push(temel.ad.clone());
                } else {
                    let en_az = degerler.iter().flatten().min().copied().unwrap_or(0);
                    let en_cok = degerler.iter().flatten().max().copied().unwrap_or(0);
                    oynak.push(format!("{} ({en_az}–{en_cok})", temel.ad));
                }
            }

            let _ = writeln!(s, "**`{}`**\n", o.ad);
            let _ = writeln!(s, "- kararlı ({}): {}", kararli.len(), liste(&kararli));
            let _ = writeln!(s, "- oynak ({}): {}", oynak.len(), liste(&oynak));
            let _ = writeln!(
                s,
                "- her koşuda hata ({}): {}\n",
                hep_hata.len(),
                liste(&hep_hata)
            );
        }
    }

    // Yanlış alarm: ground truth'u sıfır olan videolar.
    //
    // Bu ölçüt **temiz**: eşleştirme belirsizliği yok, üretilen her olay
    // yanlış. Recall'ın aksine sentetik kümenin semantik zayıflığından
    // etkilenmiyor — "olay yok" demek için sahneyi anlamak gerekmiyor.
    let olaysiz_var = ozetler.iter().any(|o| {
        o.kosumlar
            .first()
            .is_some_and(|k| k.sonuclar.iter().any(|s| s.gercek == 0))
    });
    if olaysiz_var {
        let _ = writeln!(s, "\n## Yanlış alarm (olaysız kayıtlar)\n");
        let _ = writeln!(
            s,
            "Ground truth'u sıfır olan kayıtlarda üretilen her olay yanlıştır. \
             Eşleştirme belirsizliği olmadığı için bu ölçüt recall'dan daha \
             güvenilir.\n"
        );
        let _ = writeln!(s, "| varyant | video | koşu başına üretilen olay |");
        let _ = writeln!(s, "|---|---|---|");
        for o in ozetler {
            let Some(ilk) = o.kosumlar.first() else { continue };
            for (i, temel) in ilk.sonuclar.iter().enumerate() {
                if temel.gercek != 0 {
                    continue;
                }
                let sayilar: Vec<String> = o
                    .kosumlar
                    .iter()
                    .map(|k| match k.sonuclar.get(i) {
                        Some(x) if x.hata.is_none() => x.model_olay.to_string(),
                        _ => "hata".into(),
                    })
                    .collect();
                let _ = writeln!(s, "| `{}` | {} | {} |", o.ad, temel.ad, sayilar.join(", "));
            }
        }
    }

    let _ = writeln!(s, "\n## Koşu başına ayrıntı\n");
    let _ = writeln!(
        s,
        "Hücreler `yakalanan/gerçek (modelin ürettiği)` biçiminde.\n"
    );
    for o in ozetler {
        let _ = writeln!(s, "### `{}` — prompt {}\n", o.ad, o.prompt_surumu);
        let _ = writeln!(s, "| video | {} |", (1..=o.kosumlar.len())
            .map(|i| format!("koşu {i}"))
            .collect::<Vec<_>>()
            .join(" | "));
        let _ = writeln!(s, "|---|{}", "---|".repeat(o.kosumlar.len()));

        // Video adları koşumlar arasında aynı sırada; ilkinden alınıyor.
        if let Some(ilk) = o.kosumlar.first() {
            for (i, temel) in ilk.sonuclar.iter().enumerate() {
                let hucreler: Vec<String> = o
                    .kosumlar
                    .iter()
                    .map(|k| match k.sonuclar.get(i) {
                        Some(x) => match &x.hata {
                            Some(_) => "**hata**".to_string(),
                            None => format!("{}/{} ({})", x.eslesen, x.gercek, x.model_olay),
                        },
                        None => "—".to_string(),
                    })
                    .collect();
                let _ = writeln!(s, "| {} | {} |", temel.ad, hucreler.join(" | "));
            }
        }
        let _ = writeln!(s);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sahte_sonuc(ad: &str, eslesen: usize, gercek: usize, hata: bool) -> Sonuc {
        Sonuc {
            ad: ad.into(),
            eslesen,
            gercek,
            model_olay: eslesen,
            sema_gecerli: !hata,
            aksiyon: 1,
            sapma_ms: vec![100],
            sure_sn: 1.0,
            hata: hata.then(|| "düştü".to_string()),
        }
    }

    fn sahte_varyant(ad: &str, kosum_eslesenleri: &[usize]) -> VaryantOzeti {
        VaryantOzeti {
            ad: ad.into(),
            prompt_surumu: "v1 abc".into(),
            kosumlar: kosum_eslesenleri
                .iter()
                .map(|&e| Ozet {
                    prompt_surumu: "v1 abc".into(),
                    sonuclar: vec![sahte_sonuc("v", e, 10, false)],
                })
                .collect(),
        }
    }

    /// Faz 0'ın çekirdeği: yayılım raporlanıyor mu.
    #[test]
    fn yayilim_en_az_ve_en_cok_kosumdan_gelir() {
        let v = sahte_varyant("gomulu", &[4, 7, 5]);
        assert_eq!(v.aralik(), (4, 7));
        assert!((v.ortalama_eslesen() - 16.0 / 3.0).abs() < 1e-9);
    }

    /// Hatalı koşum sessizce düşmemeli: paydada kalmalı, ayrıca sayılmalı.
    #[test]
    fn hatali_kosum_paydada_kalir() {
        let o = Ozet {
            prompt_surumu: "v1".into(),
            sonuclar: vec![sahte_sonuc("a", 2, 3, false), sahte_sonuc("b", 0, 4, true)],
        };
        let v = VaryantOzeti {
            ad: "x".into(),
            prompt_surumu: "v1".into(),
            kosumlar: vec![o],
        };
        // Ground truth 7; hatalı video 0 eşleşmeyle sayılıyor, atılmıyor.
        assert_eq!(v.gercek(), 7);
        assert_eq!(v.ortalama_eslesen(), 2.0);
        assert_eq!(v.hatali(), 1);
        assert_eq!(v.toplam_sonuc(), 2);
    }

    #[test]
    fn tek_kosumda_yayilim_sifir() {
        let v = sahte_varyant("gomulu", &[5]);
        assert_eq!(v.aralik(), (5, 5));
    }

    /// Rapor gürültü bandını yazmalı; karar ona dayanacak.
    #[test]
    fn rapor_gurultu_bandini_yaziyor() {
        let m = rapor_metni(&[sahte_varyant("gomulu", &[4, 7, 5])], 1, 3);
        assert!(m.contains("Terazi gürültüsü"));
        assert!(m.contains("**3 olay**"), "yayılım 7-4=3 yazılmalı: {m}");
    }

    /// Tek koşuda rapor bunu açıkça söylemeli, sessiz kalmamalı.
    #[test]
    fn tek_kosuda_rapor_uyariyor() {
        let m = rapor_metni(&[sahte_varyant("gomulu", &[5])], 1, 1);
        assert!(m.contains("--tekrar 3"));
    }

    /// Kararlılık ayrımı: aynı sonucu veren video ile savrulan ayrılmalı.
    ///
    /// Taban ölçümde toplam yayılımın **tamamı** tek bir videodan geldi;
    /// bandı tüm kümeye mal etmek yanıltıcı olurdu.
    #[test]
    fn kararli_ve_oynak_videolar_ayriliyor() {
        let kosum = |a: usize, b: usize, c: bool| Ozet {
            prompt_surumu: "v1".into(),
            sonuclar: vec![
                sahte_sonuc("sabit", a, 2, false),
                sahte_sonuc("savrulan", b, 6, false),
                sahte_sonuc("dusen", 0, 2, c),
            ],
        };
        let v = VaryantOzeti {
            ad: "gomulu".into(),
            prompt_surumu: "v1".into(),
            kosumlar: vec![kosum(2, 1, true), kosum(2, 6, true)],
        };

        let m = rapor_metni(&[v], 3, 2);
        assert!(m.contains("kararlı (1): sabit"), "{m}");
        assert!(m.contains("savrulan (1–6)"), "{m}");
        assert!(m.contains("her koşuda hata (1): dusen"), "{m}");
    }

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
