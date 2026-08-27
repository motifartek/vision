//! Video klip üretimi: kesme, normalizasyon, ağır çekim.
//!
//! EVREN çıkarım servisi **kare kümesi kabul etmiyor** — en fazla iki görüntü,
//! `vlm` modeli ise görüntüyü tamamen reddediyor. Modele zamansal içerik
//! vermenin tek yolu video göndermek. Bu yüzden boru hattının çıktısı JPEG
//! listesi değil, **klip**.
//!
//! # Neden normalizasyon zorunlu
//!
//! Servisin çözücüsü AV1'i açamıyor. Ölçüldü: AV1 kodlu bir video HTTP 400
//! döndürdü ve hata gövdesi tek kare bile çıkarılamadığını gösterdi
//! (`frames_indices=[]`). Aynı video H.264'e çevrilince sorunsuz çalıştı.
//! Final test videolarının kodlaması bilinmediği için bu bir tercih değil,
//! gereklilik.
//!
//! # Ağır çekim neden var
//!
//! Servis videoyu **her zaman 2 fps** örneklüyor. Dar bir pencere göndermek
//! zamansal çözünürlüğü artırmıyor: 2 saniyelik klipten yine 4 kare çıkıyor.
//! Daha fazla detay isteniyorsa pencere yavaşlatılmalı — 2 saniyelik aralık
//! 20 saniyeye yayılırsa servis 40 kare örnekler, bu da orijinal ana göre
//! 20 fps eder.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use motif_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::preflight::ExternalTool;
use crate::probe::probe;

/// Çıkarım servisinin sabit örnekleme hızı.
///
/// Klipten kaç kare çıkacağını hesaplamak için gerekiyor; ajan ne kadar detay
/// aldığını bilmeli.
pub const SERVICE_SAMPLE_FPS: f64 = 2.0;

/// Servisin tek istekte alabileceği en fazla kare.
pub const SERVICE_MAX_FRAMES: u32 = 520;

/// Servisin kabul ettiği video codec'i.
const TARGET_CODEC: &str = "h264";

/// Klip üretim ayarları.
#[derive(Debug, Clone)]
pub struct ClipOptions {
    /// Uzun kenar sınırı. Verilmezse kaynak çözünürlüğü korunur.
    pub max_dim: Option<u32>,
    /// Zaman ölçeği. 1.0 gerçek zaman, 10.0 on kat ağır çekim.
    ///
    /// Servis sabit 2 fps örneklediği için detayı artırmanın tek yolu bu.
    pub time_scale: f32,
    /// x264 kalite (CRF): düşük daha iyi. 20 makul bir denge.
    pub crf: u8,
    /// Sesi at. Analiz için gereksiz, boyutu şişiriyor.
    pub drop_audio: bool,
}

impl Default for ClipOptions {
    fn default() -> Self {
        Self {
            max_dim: None,
            time_scale: 1.0,
            crf: 20,
            drop_audio: true,
        }
    }
}

/// Üretilmiş klip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub path: PathBuf,
    /// Kaynak videodaki başlangıç.
    pub t0_ms: u64,
    /// Kaynak videodaki bitiş.
    pub t1_ms: u64,
    /// Klibin kendi süresi. Ağır çekimde kaynak aralığından uzun olur.
    pub duration_ms: u64,
    pub time_scale: f32,
    pub size_bytes: u64,
    /// Servisin bu klipten çıkaracağı kare sayısı (2 fps).
    pub service_frames: u32,
    /// Kaynak aralığa göre etkin kare hızı.
    ///
    /// `time_scale` 1 iken 2 fps; 10 iken 20 fps. Ajanın ne kadar detay
    /// aldığını görmesi için taşınıyor.
    pub effective_fps: f64,
}

fn run(cmd: &mut Command, ne: &str) -> Result<()> {
    let output = cmd.output().map_err(|_| Error::MissingDependency {
        name: ExternalTool::Ffmpeg.binary().to_string(),
        hint: "ffmpeg kurulu ve PATH üzerinde olmalı".to_string(),
    })?;

    if !output.status.success() {
        return Err(Error::CommandFailed {
            command: ne.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or_default()
                .to_string(),
        });
    }
    Ok(())
}

/// Ortak x264 çıktı bayrakları.
///
/// `main` profili ve `yuv420p` bilinçli: en geniş uyumlu kombinasyon ve
/// servisin sorunsuz açtığı ölçülen biçim.
fn x264_args(cmd: &mut Command, opts: &ClipOptions) {
    cmd.args([
        "-c:v", "libx264",
        "-profile:v", "main",
        "-pix_fmt", "yuv420p",
        "-preset", "veryfast",
        "-crf", &opts.crf.to_string(),
        "-movflags", "+faststart",
    ]);
    if opts.drop_audio {
        cmd.arg("-an");
    }
}

/// Video filtre zincirini kurar (ağır çekim + ölçekleme).
fn filters(opts: &ClipOptions) -> Option<String> {
    let mut parts = Vec::new();

    if (opts.time_scale - 1.0).abs() > f32::EPSILON {
        // setpts zaman damgalarını çarpar: 10.0 on kat yavaşlatır.
        parts.push(format!("setpts={}*PTS", opts.time_scale));
    }
    if let Some(d) = opts.max_dim {
        // `min(iw,d)` büyütmeyi engelliyor: `force_original_aspect_ratio=decrease`
        // tek başına kutuya **sığdırır**, yani küçük videoyu kutuya kadar
        // şişirir. 400x282'lik bir kayıt 768x541'e çıkıyordu; hem boşuna
        // token hem de aşağıdaki hata.
        //
        // `force_divisible_by=2` ise zorunlu: yuv420p kroma altörneklemesi
        // çift boyut ister, x264 tek yükseklikte "height not divisible by 2"
        // deyip çıktıyı hiç yazmıyor. Bu hata gerçek bir golden dataset
        // videosunda analizi tamamen düşürdü.
        parts.push(format!(
            "scale=w='min(iw,{d})':h='min(ih,{d})'\
             :force_original_aspect_ratio=decrease:force_divisible_by=2"
        ));
    }

    (!parts.is_empty()).then(|| parts.join(","))
}

/// Videonun çıkarım servisi tarafından açılabilecek codec'te olup olmadığı.
pub fn needs_normalization(video: &Path) -> Result<bool> {
    Ok(probe(video)?.codec != TARGET_CODEC)
}

/// Videoyu H.264'e çevirir.
///
/// Zaten H.264 ise dosyayı kopyalamadan `Ok(None)` döner; gereksiz yeniden
/// kodlama hem zaman hem kalite kaybı.
pub fn normalize(video: &Path, out: &Path, opts: &ClipOptions) -> Result<Option<PathBuf>> {
    if !needs_normalization(video)? {
        return Ok(None);
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = Command::new(ExternalTool::Ffmpeg.binary());
    cmd.args(["-y", "-v", "error", "-i"]).arg(video);
    if let Some(f) = filters(&ClipOptions {
        time_scale: 1.0,
        ..opts.clone()
    }) {
        cmd.args(["-vf", &f]);
    }
    x264_args(&mut cmd, opts);
    cmd.arg(out);

    run(&mut cmd, "ffmpeg normalize")?;
    Ok(Some(out.to_path_buf()))
}

/// Kaynak videodan bir zaman aralığını klip olarak çıkarır.
///
/// Çıktı her zaman H.264'tür; kaynak ne olursa olsun servis açabilir.
pub fn extract_clip(
    video: &Path,
    t0_ms: u64,
    t1_ms: u64,
    out: &Path,
    opts: &ClipOptions,
) -> Result<Clip> {
    if !video.exists() {
        return Err(Error::NotFound(format!(
            "video dosyası yok: {}",
            video.display()
        )));
    }
    if t1_ms <= t0_ms {
        return Err(Error::Config(format!(
            "geçersiz aralık: t1 ({t1_ms}) t0'dan ({t0_ms}) büyük olmalı"
        )));
    }
    if opts.time_scale <= 0.0 {
        return Err(Error::Config("time_scale sıfırdan büyük olmalı".into()));
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let kaynak_sure = t1_ms - t0_ms;

    let mut cmd = Command::new(ExternalTool::Ffmpeg.binary());
    cmd.args(["-y", "-v", "error"])
        // -ss ve -t ikisi de girdiden ÖNCE. -ss burada anahtar kareye hızlı
        // atlıyor (modern ffmpeg yine de hassas arama yapıyor).
        //
        // -t'nin girdi tarafında olması şart: çıktı tarafına konursa süreyi
        // **setpts uygulandıktan sonra** kırpar. Ağır çekimde bu, 3 saniyelik
        // aralığın 24 saniyeye yayılıp hemen 3 saniyeye geri kesilmesi demekti
        // — klip ağır çekim olduğunu bildiriyor ama aslında olmuyordu.
        // Girdi tarafında ise kaynaktan okunan süreyi sınırlıyor, sonra
        // setpts onu serbestçe uzatıyor.
        .args(["-ss", &format!("{:.3}", t0_ms as f64 / 1000.0)])
        .args(["-t", &format!("{:.3}", kaynak_sure as f64 / 1000.0)])
        .arg("-i")
        .arg(video);

    if let Some(f) = filters(opts) {
        cmd.args(["-vf", &f]);
    }
    x264_args(&mut cmd, opts);
    cmd.arg(out);

    run(&mut cmd, "ffmpeg extract_clip")?;

    // Gerçekte ne ürettiğimizi ölçüyoruz; hesaplanan süre ile dosyanın süresi
    // arama hassasiyeti yüzünden birkaç kare kayabilir.
    let info = probe(out)?;
    let service_frames =
        ((info.duration_ms as f64 / 1000.0) * SERVICE_SAMPLE_FPS).round() as u32;

    Ok(Clip {
        path: out.to_path_buf(),
        t0_ms,
        t1_ms,
        duration_ms: info.duration_ms,
        time_scale: opts.time_scale,
        size_bytes: info.size_bytes,
        service_frames: service_frames.min(SERVICE_MAX_FRAMES),
        effective_fps: SERVICE_SAMPLE_FPS * opts.time_scale as f64,
    })
}

/// Verilen aralıktan istenen kare sayısını almak için gereken zaman ölçeği.
///
/// Servis sabit 2 fps örneklediği için tek değişken klibin süresi. `hedef`
/// kare istiyorsak klip `hedef / 2` saniye sürmeli.
pub fn scale_for_frames(aralik_ms: u64, hedef_kare: u32) -> f32 {
    if aralik_ms == 0 || hedef_kare == 0 {
        return 1.0;
    }
    let gereken_sure_sn = hedef_kare as f64 / SERVICE_SAMPLE_FPS;
    let kaynak_sure_sn = aralik_ms as f64 / 1000.0;
    (gereken_sure_sn / kaynak_sure_sn).max(1.0) as f32
}

/// Bir videonun tek istekte gönderilip gönderilemeyeceği.
///
/// 520 kare / 2 fps = 260 saniye. Aşan videolar parçalanmalı.
pub fn fits_in_one_request(duration_ms: u64) -> bool {
    let kare = (duration_ms as f64 / 1000.0) * SERVICE_SAMPLE_FPS;
    kare <= SERVICE_MAX_FRAMES as f64
}

/// Uzun videoyu servise sığacak parçalara böler.
///
/// Parçalar `overlap_ms` kadar örtüşüyor: sınıra denk gelen bir olay iki
/// parçaya da girsin, kesiğin tam ortasında kalıp kaybolmasın.
pub fn segment_plan(duration_ms: u64, overlap_ms: u64) -> Vec<(u64, u64)> {
    let max_ms = (SERVICE_MAX_FRAMES as f64 / SERVICE_SAMPLE_FPS * 1000.0) as u64;
    if duration_ms <= max_ms {
        return vec![(0, duration_ms)];
    }

    let adim = max_ms.saturating_sub(overlap_ms).max(1);
    let mut parcalar = Vec::new();
    let mut t = 0u64;

    while t < duration_ms {
        let bitis = (t + max_ms).min(duration_ms);
        parcalar.push((t, bitis));
        if bitis >= duration_ms {
            break;
        }
        t += adim;
    }
    parcalar
}

/// Klip üretim süresini ölçer (benchmark için).
pub fn timed_extract(
    video: &Path,
    t0_ms: u64,
    t1_ms: u64,
    out: &Path,
    opts: &ClipOptions,
) -> Result<(Clip, Duration)> {
    let basladi = Instant::now();
    let clip = extract_clip(video, t0_ms, t1_ms, out, opts)?;
    Ok((clip, basladi.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agir_cekim_filtresi_kurulur() {
        let normal = ClipOptions::default();
        assert!(filters(&normal).is_none(), "ölçek 1 iken filtre gereksiz");

        let yavas = ClipOptions {
            time_scale: 10.0,
            ..Default::default()
        };
        let f = filters(&yavas).unwrap();
        assert!(f.contains("setpts=10*PTS"));

        let ikisi = ClipOptions {
            time_scale: 4.0,
            max_dim: Some(768),
            ..Default::default()
        };
        let f = filters(&ikisi).unwrap();
        assert!(f.contains("setpts") && f.contains("force_divisible_by=2"));
    }

    /// Sentetik test videosu üretir. ffmpeg yoksa `None` döner.
    fn ornek_video(dir: &std::path::Path, saniye: u32) -> Option<std::path::PathBuf> {
        let yol = dir.join("kaynak.mp4");
        let cikti = Command::new(ExternalTool::Ffmpeg.binary())
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc=size=320x240:rate=30:duration={saniye}"))
            .args(["-c:v", "libx264", "-profile:v", "main", "-pix_fmt", "yuv420p"])
            .arg(&yol)
            .status()
            .ok()?;
        cikti.success().then_some(yol)
    }

    /// Ağır çekim gerçekten dosyaya yansıyor mu?
    ///
    /// Regresyon: `-t` çıktı seçeneği olarak veriliyordu ve `setpts` uzattıktan
    /// **sonra** kırpıyordu. Klip `time_scale: 8.0` bildiriyor ama süresi
    /// kaynakla aynı kalıyordu; yani yakınlaştırma hiç çalışmıyordu.
    /// Filtre metnini sınamak bunu yakalayamaz, üretilen süreyi ölçmek gerekir.
    #[test]
    fn agir_cekim_klibin_suresini_gercekten_uzatir() {
        let dir = std::env::temp_dir().join("motif-clip-agir-cekim");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let Some(kaynak) = ornek_video(&dir, 6) else {
            eprintln!("ffmpeg yok, test atlandı");
            return;
        };

        let hedef = dir.join("yavas.mp4");
        let klip = extract_clip(
            &kaynak,
            2_000,
            5_000,
            &hedef,
            &ClipOptions {
                time_scale: 8.0,
                ..Default::default()
            },
        )
        .unwrap();

        // 3 saniyelik aralık 8 kat yavaşlayınca ~24 saniye olmalı.
        assert!(
            (23_000..=25_000).contains(&klip.duration_ms),
            "ağır çekim uygulanmamış: {} ms (beklenen ~24000)",
            klip.duration_ms
        );
        // Servis 2 fps örneklüyor: 24 sn -> ~48 kare. Gerçek zamanda 6 olurdu.
        assert!(
            klip.service_frames >= 45,
            "servis kare sayısı beklenenden az: {}",
            klip.service_frames
        );
        assert_eq!(klip.t1_ms - klip.t0_ms, 3_000, "kaynak aralık korunmalı");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Tek boyutlu ve küçük video klibi kırmamalı.
    ///
    /// Regresyon: `scale=...:force_original_aspect_ratio=decrease` küçük videoyu
    /// kutuya kadar büyütüyordu; 400x282 kayıt 768x541 oluyor, 541 tek olduğu
    /// için x264 "height not divisible by 2" deyip çıktıyı hiç yazmıyordu.
    /// Golden dataset'teki `JNH-RPABpdA` bu yüzden analiz edilemiyordu.
    #[test]
    fn kucuk_ve_tek_boyutlu_video_klibe_donusur() {
        let dir = std::env::temp_dir().join("motif-clip-tek-boyut");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // 400x282: hem max_dim'den küçük hem yüksekliği çift ama ölçeklenince
        // tek sayıya düşen bir oran.
        let yol = dir.join("kucuk.mp4");
        let durum = Command::new(ExternalTool::Ffmpeg.binary())
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=size=400x282:rate=20:duration=4")
            .args(["-c:v", "libx264", "-profile:v", "main", "-pix_fmt", "yuv420p"])
            .arg(&yol)
            .status();
        let Ok(s) = durum else {
            eprintln!("ffmpeg yok, test atlandı");
            return;
        };
        if !s.success() {
            eprintln!("kaynak üretilemedi, test atlandı");
            return;
        }

        let hedef = dir.join("cikti.mp4");
        let klip = extract_clip(
            &yol,
            0,
            4_000,
            &hedef,
            &ClipOptions {
                max_dim: Some(768),
                ..Default::default()
            },
        )
        .expect("küçük/tek boyutlu video klibe dönüşmeli");

        assert!(klip.size_bytes > 0, "çıktı dosyası boş");
        let bilgi = probe(&hedef).unwrap();
        assert_eq!(bilgi.width % 2, 0, "genişlik çift olmalı");
        assert_eq!(bilgi.height % 2, 0, "yükseklik çift olmalı");
        // Büyütme yapılmamalı.
        assert!(bilgi.width <= 400, "video büyütülmüş: {}", bilgi.width);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Gerçek zamanlı kesmede süre kaynakla aynı kalmalı.
    #[test]
    fn gercek_zamanli_klip_kaynak_suresini_korur() {
        let dir = std::env::temp_dir().join("motif-clip-gercek-zaman");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let Some(kaynak) = ornek_video(&dir, 6) else {
            eprintln!("ffmpeg yok, test atlandı");
            return;
        };

        let hedef = dir.join("normal.mp4");
        let klip = extract_clip(&kaynak, 1_000, 4_000, &hedef, &ClipOptions::default()).unwrap();

        assert!(
            (2_800..=3_200).contains(&klip.duration_ms),
            "kesilen süre yanlış: {} ms",
            klip.duration_ms
        );
        assert!((klip.time_scale - 1.0).abs() < 0.01);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kare_hedefinden_olcek_hesaplanir() {
        // 2 sn'lik aralıktan 40 kare istiyorsak klip 20 sn sürmeli -> 10x
        assert!((scale_for_frames(2_000, 40) - 10.0).abs() < 0.01);
        // 10 sn'lik aralıktan 20 kare zaten gerçek zamanda çıkıyor
        assert!((scale_for_frames(10_000, 20) - 1.0).abs() < 0.01);
        // Daha az kare istemek videoyu hızlandırmaz; alt sınır 1.0
        assert!((scale_for_frames(10_000, 4) - 1.0).abs() < 0.01);
    }

    #[test]
    fn tek_istege_sigma_siniri() {
        assert!(fits_in_one_request(259_000));
        assert!(fits_in_one_request(260_000));
        assert!(!fits_in_one_request(261_000));
    }

    #[test]
    fn kisa_video_tek_parca() {
        assert_eq!(segment_plan(60_000, 5_000), vec![(0, 60_000)]);
    }

    #[test]
    fn uzun_video_ortusen_parcalara_bolunur() {
        let parcalar = segment_plan(600_000, 10_000); // 10 dk, 10 sn örtüşme
        assert!(parcalar.len() >= 3);

        // Her parça servise sığmalı
        for (a, b) in &parcalar {
            assert!(fits_in_one_request(b - a), "parça {a}-{b} sığmıyor");
        }
        // Baştan sona kapsanmalı
        assert_eq!(parcalar.first().unwrap().0, 0);
        assert_eq!(parcalar.last().unwrap().1, 600_000);
        // Ardışık parçalar örtüşmeli: sınırdaki olay kaybolmasın
        for w in parcalar.windows(2) {
            assert!(w[1].0 < w[0].1, "parçalar örtüşmüyor: {:?}", w);
        }
    }

    #[test]
    fn gecersiz_aralik_reddedilir() {
        let yok = Path::new("olmayan.mp4");
        assert!(extract_clip(yok, 0, 1000, Path::new("x.mp4"), &ClipOptions::default()).is_err());
    }
}
