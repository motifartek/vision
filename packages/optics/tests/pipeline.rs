//! Uçtan uca boru hattı testleri.
//!
//! Test videosu ffmpeg'in `testsrc2` kaynağıyla üretilir. Bu kaynağın üzerinde
//! **görünür bir kare sayacı ve saat** vardır; üretilen dosya elle açılıp
//! `optics decode` çıktısındaki zaman damgalarıyla gözle karşılaştırılabilir.
//! Zaman hesabını doğrulamanın en temiz yolu bu.
//!
//! ffmpeg kurulu değilse testler sessizce atlanır; CI'da kurulu olmalı.

use std::path::{Path, PathBuf};
use std::process::Command;

use motif_optics::{
    build_profile, decode_gray, extract_gray_at, extract_jpegs, probe, select_frames,
    AnalysisConfig, ExtractOptions, SamplingConfig,
};

/// Test videosunu tutan geçici dosya; düşerken kendini siler.
struct TestVideo {
    path: PathBuf,
}

impl Drop for TestVideo {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn ffmpeg_var_mi() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `saniye` uzunluğunda, `fps` kare hızında bir test videosu üretir.
fn test_videosu_uret(ad: &str, saniye: u32, fps: u32) -> TestVideo {
    let path = std::env::temp_dir().join(format!("motif-optics-{}-{}.mp4", ad, std::process::id()));

    let status = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=size=320x180:rate={fps}"))
        .args(["-t", &saniye.to_string(), "-pix_fmt", "yuv420p"])
        .arg(&path)
        .status()
        .expect("ffmpeg çalıştırılamadı");

    assert!(status.success(), "test videosu üretilemedi");
    TestVideo { path }
}

fn yol(v: &TestVideo) -> &Path {
    &v.path
}

#[test]
fn probe_video_ozelliklerini_dogru_okur() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    let video = test_videosu_uret("probe", 4, 30);
    let info = probe(yol(&video)).expect("probe başarısız");

    assert_eq!(info.width, 320);
    assert_eq!(info.height, 180);
    assert!(
        (info.fps - 30.0).abs() < 0.1,
        "kare hızı 30 bekleniyordu, {} geldi",
        info.fps
    );
    assert!(
        (3_900..=4_200).contains(&info.duration_ms),
        "süre ~4000 ms bekleniyordu, {} geldi",
        info.duration_ms
    );
    assert!(info.size_bytes > 0);
    assert_eq!(info.codec, "h264");
}

#[test]
fn probe_olmayan_dosyada_hata_verir() {
    let sonuc = probe(Path::new("kesinlikle-olmayan-bir-dosya.mp4"));
    assert!(sonuc.is_err());
}

#[test]
fn decode_beklenen_sayida_kare_uretir() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    let video = test_videosu_uret("decode", 4, 30);
    let cfg = AnalysisConfig::default(); // 15 fps, 160x90

    let kareler: Vec<_> = decode_gray(yol(&video), cfg)
        .expect("decode başlatılamadı")
        .collect::<Result<Vec<_>, _>>()
        .expect("kare çözme hatası");

    // 4 saniye x 15 fps = 60 kare. ffmpeg sınırda bir kare oynatabilir.
    assert!(
        (58..=62).contains(&kareler.len()),
        "~60 kare bekleniyordu, {} geldi",
        kareler.len()
    );

    for kare in &kareler {
        assert_eq!(
            kare.data.len(),
            cfg.frame_bytes(),
            "kare tamponu {} bayt olmalı",
            cfg.frame_bytes()
        );
    }
}

#[test]
fn zaman_damgalari_artan_ve_sabit_arali() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    let video = test_videosu_uret("zaman", 3, 30);
    let cfg = AnalysisConfig::default();

    let kareler: Vec<_> = decode_gray(yol(&video), cfg)
        .expect("decode başlatılamadı")
        .collect::<Result<Vec<_>, _>>()
        .expect("kare çözme hatası");

    assert_eq!(kareler[0].index, 0);
    assert_eq!(kareler[0].t_ms, 0);

    for (i, kare) in kareler.iter().enumerate() {
        assert_eq!(kare.index, i as u32, "sıra numarası atlanmış");
        // 15 fps -> kare başına tam 66.67 ms; yuvarlama toleransı 1 ms.
        let beklenen = (i as f64 * 1000.0 / cfg.analysis_fps).round() as u64;
        assert!(
            kare.t_ms.abs_diff(beklenen) <= 1,
            "kare {i}: {} ms bekleniyordu, {} geldi",
            beklenen,
            kare.t_ms
        );
    }

    // Zaman kesin olarak artmalı.
    for pencere in kareler.windows(2) {
        assert!(pencere[1].t_ms > pencere[0].t_ms, "zaman geriye gitti");
    }
}

#[test]
fn erken_birakilan_akis_asili_surec_birakmaz() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    // Uzun bir video: erken bırakıldığında ffmpeg hâlâ çözüyor olacak.
    let video = test_videosu_uret("erken", 20, 30);
    let cfg = AnalysisConfig::default();

    let alinan: Vec<_> = decode_gray(yol(&video), cfg)
        .expect("decode başlatılamadı")
        .take(5)
        .collect::<Result<Vec<_>, _>>()
        .expect("kare çözme hatası");

    assert_eq!(alinan.len(), 5);
    // Drop burada çalışır. Süreç sonlandırılmazsa test takılır ya da
    // geçici dosya silinemez.
}

#[test]
fn atlama_dogru_kareye_gidiyor() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    // Faz planında işaretlenen risk: `-ss` girdiden önce kullanıldığında
    // ffmpeg anahtar kareye atlayıp ileri çözer. Hızlıdır ama bazı kodeklerde
    // kayabilir. Varsayılmayıp ölçülüyor.
    //
    // Yöntem: sıralı çözmeden gelen kare ile aynı zaman damgasına atlayarak
    // alınan kare karşılaştırılıyor. testsrc2 deseni sürekli hareket ettiği
    // için yanlış kareye gidilirse fark büyük olur.
    let video = test_videosu_uret("atlama", 6, 30);
    let cfg = AnalysisConfig::default();

    let kareler: Vec<_> = decode_gray(yol(&video), cfg)
        .expect("decode başlatılamadı")
        .collect::<Result<Vec<_>, _>>()
        .expect("kare çözme hatası");

    // Videonun ortalarından birkaç nokta dene.
    for &i in &[15usize, 40, 65] {
        let beklenen = &kareler[i];
        let atlanan = extract_gray_at(yol(&video), beklenen.t_ms, cfg)
            .expect("atlayarak kare çıkarılamadı");

        assert_eq!(atlanan.len(), beklenen.data.len());

        let toplam_fark: u64 = beklenen
            .data
            .iter()
            .zip(atlanan.iter())
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum();
        let ortalama_fark = toplam_fark as f64 / beklenen.data.len() as f64;

        assert!(
            ortalama_fark < 6.0,
            "t={} ms: atlama farklı kareye gitmiş olabilir, ortalama piksel farkı {:.2}",
            beklenen.t_ms,
            ortalama_fark
        );
    }
}

#[test]
fn uctan_uca_ornekleme_ve_kare_cikarma() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    let video = test_videosu_uret("uctanuca", 8, 30);
    let cfg = AnalysisConfig::default();

    let profile = build_profile(yol(&video), cfg).expect("profil çıkarılamadı");
    assert!(!profile.is_empty());

    let selection = select_frames(
        &profile,
        SamplingConfig {
            budget: 8,
            uniform_prior: 0.25,
            // testsrc2 sürekli hareketli; tekrar elemesi bu testin konusu değil.
            dedup_hamming: 0,
            force_scene_cuts: true,
            subtract_noise_floor: true,
        },
    )
    .expect("örnekleme başarısız");

    assert!(!selection.frames.is_empty());

    // Kapsama garantisi gerçek veride de tutmalı.
    let ortalama_aralik = profile.duration_ms as f64 / 8.0;
    let sinir = ortalama_aralik / 0.25;
    assert!(
        (selection.max_gap_ms as f64) <= sinir * 1.05,
        "boşluk garantisi tutmadı: {} ms > {:.0} ms",
        selection.max_gap_ms,
        sinir
    );

    let cikti = std::env::temp_dir().join(format!("motif-kareler-{}", std::process::id()));
    let sonuc = extract_jpegs(
        yol(&video),
        &selection.timestamps(),
        &cikti,
        &ExtractOptions::default(),
    )
    .expect("kare çıkarılamadı");

    assert_eq!(sonuc.frames.len(), selection.frames.len());
    for kare in &sonuc.frames {
        let meta = std::fs::metadata(&kare.path).expect("çıkarılan kare diskte yok");
        assert!(meta.len() > 0, "çıkarılan kare boş");
    }

    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn zaman_damgasi_bindirmesi_kareyi_degistirir() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    let video = test_videosu_uret("bindirme", 3, 30);
    let cikti = std::env::temp_dir().join(format!("motif-bindirme-{}", std::process::id()));

    let dusuz = extract_jpegs(
        yol(&video),
        &[1_000],
        &cikti.join("duz"),
        &ExtractOptions::default(),
    )
    .expect("düz kare çıkarılamadı");

    let bindirmeli = extract_jpegs(
        yol(&video),
        &[1_000],
        &cikti.join("bindirmeli"),
        &ExtractOptions {
            timestamp_overlay: true,
            ..Default::default()
        },
    );

    match bindirmeli {
        Ok(bindirmeli) => {
            let a = std::fs::read(&dusuz.frames[0].path).unwrap();
            let b = std::fs::read(&bindirmeli.frames[0].path).unwrap();
            assert_ne!(a, b, "bindirme kareyi değiştirmemiş");
        }
        Err(err) => {
            // Fontu olmayan bir makinede test engel olmamalı, ama sessiz de
            // geçmemeli: hata mesajı fontla ilgili olmalı.
            let mesaj = err.to_string();
            assert!(
                mesaj.contains("font"),
                "beklenmeyen bindirme hatası: {mesaj}"
            );
            eprintln!("bindirme atlandı: {mesaj}");
        }
    }

    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn cozunurluk_ayari_kare_boyutunu_belirler() {
    if !ffmpeg_var_mi() {
        eprintln!("ffmpeg yok, test atlandı");
        return;
    }

    let video = test_videosu_uret("cozunurluk", 2, 30);
    let cfg = AnalysisConfig {
        analysis_fps: 10.0,
        width: 64,
        height: 36,
    };

    let kareler: Vec<_> = decode_gray(yol(&video), cfg)
        .expect("decode başlatılamadı")
        .collect::<Result<Vec<_>, _>>()
        .expect("kare çözme hatası");

    assert!(
        (18..=22).contains(&kareler.len()),
        "~20 kare bekleniyordu, {} geldi",
        kareler.len()
    );
    assert_eq!(kareler[0].data.len(), 64 * 36);
}
