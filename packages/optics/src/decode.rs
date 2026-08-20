//! ffmpeg'den ham gri kare akışı.
//!
//! ffmpeg alt süreç olarak çalıştırılır ve çözülmüş kareler boru üzerinden
//! ham piksel olarak okunur. Kare başına tam `width * height` bayt gelir.
//!
//! # Neden akış, neden hepsini belleğe almıyoruz
//!
//! Bellek kullanımı **video uzunluğundan bağımsız** kalmalı (KPI). 10 dakikalık
//! bir video 160x90 gride bile 9000 kare eder; hepsini tutmak ~130 MB. Kareler
//! tek tek verilip tüketildiği için tepe bellek tek karelik kalıyor.
//!
//! # ffmpeg süreç açma maliyeti
//!
//! Ölçüldü (`optics spawn-cost`, 15 örnek): **~20 ms**. Yani alt süreç yaklaşımı
//! pratikte bedava. 2 dakikalık 720p video 1.34 sn'de çözülüyor, gerçek zamanın
//! ~90 katı.
//!
//! Bu sayı önemli çünkü ses tarafında (`feature/audio`) ffmpeg alt süreci için
//! ~960 ms ölçülmüş ve bu yüzden süreç içi çözmeye (symphonia) geçilmişti.
//! Aradaki fark büyük ihtimatle soğuk/sıcak başlangıç: ikili bir kez okunduktan
//! sonra işletim sistemi önbelleğinden geliyor. Video tarafında saf Rust bir
//! H.264 çözücü alternatifi zaten yok, dolayısıyla bu ölçüm ffmpeg kararını
//! doğruluyor.
//!
//! Yine de tasarım kuralı geçerli: **video başına ffmpeg çağrısı sayısı sabit
//! tutulur.** Kareler tek tek değil toplu çıkarılıp önbelleğe alınır — 20 ms
//! bile kare başına ödendiğinde 30 kare için 0.6 sn eder.

use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use motif_core::{Error, Result};

use crate::preflight::ExternalTool;
use crate::types::{AnalysisConfig, AnalysisFrame};

/// Çözülmüş gri karelerin akışı.
///
/// [`Iterator`] olarak tüketilir. Erken bırakılırsa (`take(n)` gibi) ffmpeg
/// süreci [`Drop`] içinde sonlandırılır; arkada asılı süreç kalmaz.
pub struct GrayFrames {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stderr: Option<JoinHandle<String>>,
    cfg: AnalysisConfig,
    buf: Vec<u8>,
    index: u32,
    done: bool,
}

impl GrayFrames {
    /// Çözücünün çalıştığı yapılandırma.
    pub fn config(&self) -> AnalysisConfig {
        self.cfg
    }

    /// Şu ana kadar üretilen kare sayısı.
    pub fn frames_produced(&self) -> u32 {
        self.index
    }

    /// Biriken stderr çıktısını toplar. Süreç bittikten sonra anlamlıdır.
    fn take_stderr(&mut self) -> String {
        self.stderr
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default()
    }
}

impl Iterator for GrayFrames {
    type Item = Result<AnalysisFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match read_full(&mut self.stdout, &mut self.buf) {
            // Kare sınırında temiz bitiş: akış normal şekilde tükendi.
            Ok(0) => {
                self.done = true;
                match self.child.wait() {
                    Ok(status) if status.success() => None,
                    Ok(status) => {
                        let stderr = self.take_stderr();
                        Some(Err(Error::CommandFailed {
                            command: format!("ffmpeg (çıkış kodu {status})"),
                            stderr,
                        }))
                    }
                    Err(err) => Some(Err(Error::Io(err))),
                }
            }

            // Tam kare okundu.
            Ok(n) if n == self.buf.len() => {
                let frame = AnalysisFrame {
                    index: self.index,
                    t_ms: self.cfg.timestamp_ms(self.index),
                    data: self.buf.clone(),
                };
                self.index += 1;
                Some(Ok(frame))
            }

            // Yarım kare: akış kare sınırında bitmedi, video bozuk ya da kesik.
            Ok(n) => {
                self.done = true;
                let stderr = self.take_stderr();
                Some(Err(Error::InvalidVideo(format!(
                    "akış kare sınırında bitmedi: {} baytlık karede {} bayt okundu. ffmpeg: {}",
                    self.buf.len(),
                    n,
                    stderr.trim()
                ))))
            }

            Err(err) => {
                self.done = true;
                Some(Err(Error::Io(err)))
            }
        }
    }
}

impl Drop for GrayFrames {
    fn drop(&mut self) {
        // Erken bırakıldıysa ffmpeg hâlâ çözüyor olabilir; boruyu kapatmak tek
        // başına yetmeyebileceği için süreci açıkça sonlandırıyoruz.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Tampon dolana ya da akış bitene kadar okur.
///
/// [`Read::read_exact`] kaç bayt okunduğunu söylemediği için elle yazıldı:
/// "temiz bitiş" (0 bayt) ile "yarım kare" (0 < n < tampon) ayrımını
/// yapabilmemiz gerekiyor.
fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Videoyu analiz çözünürlüğünde gri karelere çözer.
///
/// Kullanılan komut:
///
/// ```text
/// ffmpeg -nostdin -v error -i <path> -an \
///   -vf "fps=<F>,scale=<W>:<H>,format=gray" \
///   -f rawvideo -pix_fmt gray -
/// ```
///
/// `fps` filtresi çıktıyı **sabit kare hızına** zorlar. Bu sayede zaman hesabı
/// `t = index / analysis_fps` değişken kare hızlı videolarda bile kesin kalır;
/// kare başına ayrı zaman damgası taşımamıza gerek kalmıyor.
pub fn decode_gray(path: &Path, cfg: AnalysisConfig) -> Result<GrayFrames> {
    if !path.exists() {
        return Err(Error::NotFound(format!(
            "video dosyası yok: {}",
            path.display()
        )));
    }

    let filter = format!(
        "fps={},scale={}:{},format=gray",
        cfg.analysis_fps, cfg.width, cfg.height
    );

    let mut child = Command::new(ExternalTool::Ffmpeg.binary())
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(path)
        .args([
            "-an", "-vf", &filter, "-f", "rawvideo", "-pix_fmt", "gray", "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|_| Error::MissingDependency {
            name: ExternalTool::Ffmpeg.binary().to_string(),
            hint: "ffmpeg'i kurup PATH'e ekleyin.".to_string(),
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::InvalidVideo("ffmpeg stdout borusu açılamadı".into()))?;

    // stderr ayrı bir thread'de boşaltılıyor. Boşaltılmazsa boru tamponu
    // dolduğunda ffmpeg yazmaya çalışırken bloke olur, biz de kare beklerken
    // bloke oluruz. Klasik kilitlenme.
    let stderr = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut out = String::new();
            let _ = pipe.read_to_string(&mut out);
            out
        })
    });

    Ok(GrayFrames {
        child,
        stdout: BufReader::with_capacity(cfg.frame_bytes() * 4, stdout),
        stderr,
        cfg,
        buf: vec![0u8; cfg.frame_bytes()],
        index: 0,
        done: false,
    })
}

/// ffmpeg'in sabit süreç açma maliyetini ölçer.
///
/// `ffmpeg -version` çalıştırıp bitmesini bekler; ölçtüğü şey süreç yaratma +
/// ikilinin belleğe yüklenmesi + çıkıştır. Yani **iş yapmadan önceki taban
/// maliyet**. Pass 3'ün gecikme bütçesi doğrudan bu sayıya bağlı.
pub fn measure_spawn_overhead(samples: u32) -> Result<Duration> {
    if samples == 0 {
        return Err(Error::Config("örnek sayısı sıfır olamaz".into()));
    }

    let mut total = Duration::ZERO;
    for _ in 0..samples {
        let start = Instant::now();
        let output = Command::new(ExternalTool::Ffmpeg.binary())
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| Error::MissingDependency {
                name: ExternalTool::Ffmpeg.binary().to_string(),
                hint: "ffmpeg'i kurup PATH'e ekleyin.".to_string(),
            })?;
        if !output.status.success() {
            return Err(Error::CommandFailed {
                command: "ffmpeg -version".into(),
                stderr: String::new(),
            });
        }
        total += start.elapsed();
    }

    Ok(total / samples)
}
