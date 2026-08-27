use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Pencere/adım profilleri.
/// Çıkarım maliyeti ≈ pencere × (süre/adım); uzun pencere orantılı adımla daha ucuzdur.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct WindowProfile {
    pub name: &'static str,
    pub window_sec: f32,
    pub hop_sec: f32,
}

pub const PROFILES: [WindowProfile; 3] = [
    WindowProfile { name: "hassas",   window_sec: 1.0,  hop_sec: 0.25 },
    WindowProfile { name: "dengeli",  window_sec: 2.0,  hop_sec: 0.5  },
    WindowProfile { name: "isabetli", window_sec: 10.0, hop_sec: 5.0  },
];

pub const DEFAULT_PROFILE: &str = "dengeli";

#[allow(dead_code)] // faz 2'de /v1/audio/analyze profil seçiminde kullanılacak
pub fn profile(name: &str) -> Option<WindowProfile> {
    PROFILES.iter().copied().find(|p| p.name == name)
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Dinleme adresi. Servisin kendi kimlik doğrulaması yok, o yüzden
    /// varsayılan hâlâ `127.0.0.1:SONIC_PORT`.
    ///
    /// Ama konteynerde loopback'e bağlanmak servisi tümüyle kör ediyor:
    /// yayımlanan port da `http://sonic:8081` de konteynerin dışından gelir,
    /// ikisi de loopback'e düşmez. Bu yüzden adres `SONIC_BIND` ile açılabilir
    /// (compose `0.0.0.0:8081` veriyor); dışarıya kapıyı yayımlanan portu
    /// loopback'e sabitleyerek kapatıyoruz, sürecin kendi soketiyle değil.
    pub bind: SocketAddr,
    pub models_dir: PathBuf,
    pub model: String,
    pub prefer_int8: bool,
    pub intra_threads: usize,
    /// Tek ONNX çağrısındaki pencere sayısı. GPU'da büyük batch, başlatma
    /// maliyetini pencerelere bölerek asıl kazancı sağlar.
    pub batch_size: usize,
    /// Ayarlıysa ONNX çağrısı bu adresteki `model-host` sürecine taşınır ve
    /// bu süreçte model yüklenmez.
    ///
    /// Sebebi DirectML: Windows DirectX 12 API'si olduğu için Linux
    /// konteynerinde çalışmıyor, ama sonic konteynerde koşuyor. Kartı
    /// kullanmanın tek yolu tensör→tensör çağrısını host'a taşımak. Çözme,
    /// log-mel, bölütleme ve güvenlik kuralları konteynerde kalıyor —
    /// ölçüldüğüne göre toplam sürenin %94'ü zaten bu tek çağrıda geçiyor.
    ///
    /// Ayarlı değilse bugünkü davranış birebir korunur.
    pub model_host: Option<String>,
    pub media_root: Option<PathBuf>,
    /// Yükleme tavanı, bayt. `0` = sınırsız (varsayılan): dosya belleğe
    /// alınmadan diske akıtıldığı için büyük videolar sorun değil. Diski
    /// koruması gereken kurulumlar buraya bir tavan koyabilir.
    pub max_upload_bytes: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let models_dir = std::env::var_os("SONIC_MODELS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models"));

        // GPU derlemelerinde fp32 varsayılır; int8 CPU'ya özgüdür.
        let gpu = cfg!(any(feature = "cuda", feature = "tensorrt", feature = "directml"));
        let default_int8 = !gpu;
        // GPU'da batch 256, CED-Base ile 6 GB'lık bir karta sığmıyor: ölçüldü,
        // DirectML `HRESULT 0x887A0006` (cihaz askıda) ile düşüyor. 64 hem
        // mütevazı kartlarda güvenli hem de başlatma maliyetini yeterince
        // amorti ediyor. Bol VRAM'li makinede SONIC_BATCH ile yükseltin.
        let default_batch = if gpu { 64 } else { 32 };

        // Bozuk bir `SONIC_BIND` sessizce loopback'e düşerse konteynerdeki
        // arıza tam olarak düzeltmeye çalıştığımız arıza olur — servis ayakta
        // görünür, kimse erişemez. Bu yüzden burada gürültülü duruyoruz.
        let bind = match std::env::var("SONIC_BIND") {
            Ok(raw) => raw.parse::<SocketAddr>().unwrap_or_else(|e| {
                panic!("SONIC_BIND çözümlenemedi ({raw}): {e}. Beklenen biçim: 0.0.0.0:8081")
            }),
            Err(_) => SocketAddr::from((Ipv4Addr::LOCALHOST, env_parse("SONIC_PORT", 8081))),
        };

        Self {
            bind,
            models_dir,
            // Varsayılan CED-Base: aynı videoda Tiny'nin ürettiği yanlış
            // pozitifleri (At %81, Kalp atışı, Hapşırık, Baykuş) hiç üretmiyor,
            // gerçek sesleri ise koruyor. Hız bedeli var ama 9 dakikalık video
            // yine 7 saniyenin altında bitiyor.
            // Hız öncelikliyse: SONIC_MODEL=ced-tiny
            model: std::env::var("SONIC_MODEL").unwrap_or_else(|_| "ced-base".into()),
            prefer_int8: env_parse("SONIC_INT8", default_int8),
            intra_threads: env_parse(
                "SONIC_THREADS",
                std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            ),
            batch_size: env_parse("SONIC_BATCH", default_batch),
            // Sondaki `/` temizleniyor: `http://host:8082/` verildiğinde
            // istekler `//v1/infer` olurdu.
            model_host: std::env::var("SONIC_MODEL_HOST")
                .ok()
                .map(|v| v.trim().trim_end_matches('/').to_string())
                .filter(|v| !v.is_empty()),
            media_root: std::env::var_os("SONIC_MEDIA_ROOT").map(PathBuf::from),
            max_upload_bytes: env_parse("SONIC_MAX_UPLOAD_BYTES", 0),
        }
    }
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
