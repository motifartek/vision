//! Çıkarım servisi istemcisi ve karar sözleşmesi.
//!
//! Servis OpenAI uyumlu; video `data:video/mp4;base64,...` olarak `video_url`
//! alanında gidiyor. Modelden düz anlatı değil **karar** istiyoruz: ya bir
//! aralığa yakından bakmak istiyor, ya da raporu veriyor.
//!
//! # Neden OpenAI araç API'si kullanılmıyor
//!
//! Kullanılamıyor. `vlm` modelini sunan vLLM örneğinde araç ayrıştırıcı kurulu
//! değil; ölçüldü:
//!
//! ```text
//! tool_choice="required" -> 400  "requires --tool-call-parser to be set"
//! tool_choice="auto"     -> 400  "requires --enable-auto-tool-choice"
//! tool_choice="none"     -> 200, ama model kararı yine <tool_call>{...}</tool_call>
//!                                olarak content içinde düz metin döndürüyor
//! ```
//!
//! Yani model araç çağırmayı biliyor, sunucu onu yapılandırılmış `tool_calls`
//! alanına çevirmiyor. Bu yüzden karar **istemle istenen JSON** olarak alınıyor
//! ve burada ayrıştırılıyor. Modelin kendiliğinden ürettiği `<tool_call>`
//! sarmalı ve ``` çitleri de destekleniyor.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Modelin verdiği karar.
///
/// Deniz'in `feature/vision-orchestration` dalındaki ayrım korunuyor:
/// yakınlaştırmak bir seçenek, raporu bitirmek diğeri.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    /// Modelin belirli bir aralığı daha ayrıntılı görmek istemesi.
    ZoomRange { t0_ms: u64, t1_ms: u64 },
    /// Modelin analizi bitirip raporu vermesi.
    Report(RawReport),
}

/// Modelin ürettiği ham rapor. Zamanlar **klibin kendi saatiyle** gelir.
///
/// Kaynak videoya çevirme burada yapılmaz; ajan `ClipRef::to_source_ms` ile
/// çevirir. Sebebi ölçüldü: modele dönüşüm formülü verilse bile aritmetiği
/// güvenilir yapmıyor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawReport {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub events: Vec<RawEvent>,
    #[serde(default)]
    pub risk: String,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// Modelin bildirdiği tek olay.
///
/// Zaman `"MM:SS"` olarak isteniyor, milisaniye olarak değil. Sebebi ölçüldü:
/// milisaniye istendiğinde model koşudan koşuya tutarsız davranıyor — aynı
/// video için bir koşuda `12000` (doğru), başka bir koşuda `1000` (saniyeyi
/// milisaniye sanmış) döndürdü. `MM:SS` istendiğinde üç videonun üçünde de
/// zamanlar doğru çıktı.
///
/// `t_ms` yine de kabul ediliyor: model bazen ikisini birden veriyor ve
/// `time` yoksa elde kalan tek bilgi o oluyor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEvent {
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub t_ms: Option<u64>,
    pub event: String,
    #[serde(default)]
    pub severity: String,
}

impl RawEvent {
    /// Olayın klip içindeki zamanı, milisaniye.
    ///
    /// `time` varsa o kullanılır; yoksa `t_ms`'e düşülür. İkisi de yoksa 0.
    pub fn klip_ms(&self) -> u64 {
        self.time
            .as_deref()
            .and_then(mmss_ms)
            .or(self.t_ms)
            .unwrap_or(0)
    }
}

/// `"MM:SS"` ya da `"HH:MM:SS"` biçimini milisaniyeye çevirir.
pub fn mmss_ms(s: &str) -> Option<u64> {
    let parcalar: Vec<&str> = s.trim().split(':').collect();
    let sayilar: Option<Vec<u64>> = parcalar
        .iter()
        .map(|p| p.trim().parse::<f64>().ok().map(|x| x as u64))
        .collect();
    let sayilar = sayilar?;
    match sayilar.len() {
        2 => Some((sayilar[0] * 60 + sayilar[1]) * 1000),
        3 => Some((sayilar[0] * 3600 + sayilar[1] * 60 + sayilar[2]) * 1000),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VlmError {
    #[error("çıkarım servisine bağlanılamadı: {0}")]
    Transport(String),
    #[error("çıkarım servisi {status} döndü: {body}")]
    Status { status: u16, body: String },
    #[error("cevap çözümlenemedi: {0}")]
    Decode(String),
    #[error("modelin cevabından karar çıkarılamadı: {0}")]
    NoDecision(String),
}

#[async_trait::async_trait]
pub trait VlmProvider: Send + Sync {
    /// Bir klibi modele verip kararını döndürür.
    ///
    /// Metin **ikiye ayrılmış** geliyor: `prefix` videodan önce, `suffix`
    /// videodan sonra. Ayrım ön ek önbelleği için — servis üzerinde ölçüldü,
    /// `[metin, video, metin]` sıralaması destekleniyor ve sabit ön ek
    /// önbelleğe isabet ediyor.
    async fn analyze(
        &self,
        prefix: &str,
        suffix: &str,
        clip: &[u8],
    ) -> Result<Decision, VlmError>;
}

/// EVREN çıkarım servisi istemcisi.
pub struct EvrenProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl EvrenProvider {
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = std::env::var("EVREN_BASE_URL")
            .unwrap_or_else(|_| "https://evren-llmapi.ssyz.org.tr/v1".into());
        let model = std::env::var("EVREN_MODEL").unwrap_or_else(|_| "vlm".into());

        // Anahtar yalnızca ortam değişkeninden okunur. Depo halka açık ve jüri
        // tarafından izleniyor; anahtarın hiçbir dosyaya girmemesi gerekiyor.
        let api_key = std::env::var("EVREN_KEY").map_err(|_| {
            anyhow::anyhow!(
                "EVREN_KEY ortam değişkeni tanımlı değil. Anahtar depoya yazılmaz; \
                 kabuğa `export EVREN_KEY=...` ile verin."
            )
        })?;

        Ok(Self {
            // Video base64 olarak gittiği için istek büyük ve yanıt yavaş olabiliyor;
            // servis dokümantasyonunda soğuk çağrı 17,8 saniye ölçülmüş.
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(1800))
                .build()?,
            base_url,
            model,
            api_key,
        })
    }
}

#[async_trait::async_trait]
impl VlmProvider for EvrenProvider {
    async fn analyze(
        &self,
        prefix: &str,
        suffix: &str,
        clip: &[u8],
    ) -> Result<Decision, VlmError> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(clip);

        // Sıra bilinçli: sabit metin → video → değişken metin.
        //
        // Ön ek önbelleği ancak baştaki token dizisi birebir aynı kaldığında
        // isabet ediyor. Kayda özgü değerler videodan önce dursaydı ön ek her
        // videoda değişir ve önbellek hiç tutmazdı.
        let mut icerik = vec![
            json!({"type": "text", "text": prefix}),
            json!({
                "type": "video_url",
                "video_url": {"url": format!("data:video/mp4;base64,{b64}")}
            }),
        ];
        if !suffix.trim().is_empty() {
            icerik.push(json!({"type": "text", "text": suffix}));
        }

        let body = json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": icerik }],
            // Akıl yürütme açıkken dar bütçe boş cevap üretiyor; servis
            // dokümantasyonunun ilk uyarısı bu.
            "max_tokens": 2048,
            "temperature": 0.2
        });

        let res = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| VlmError::Transport(e.to_string()))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(VlmError::Status {
                status: status.as_u16(),
                body: body.chars().take(600).collect(),
            });
        }

        let value: Value = res
            .json()
            .await
            .map_err(|e| VlmError::Decode(e.to_string()))?;

        let icerik = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default();

        parse_decision(icerik)
    }
}

/// Modelin metin cevabından JSON gövdesini çıkarır.
///
/// Model cevabı üç biçimde sarmalayabiliyor: çıplak JSON, ```json çiti ya da
/// `<tool_call>` etiketi. Üçü de aynı yere varıyor: ilk `{` ile son `}` arası.
fn json_govdesi(metin: &str) -> Option<&str> {
    let i = metin.find('{')?;
    let j = metin.rfind('}')?;
    (j > i).then(|| &metin[i..=j])
}

/// Modelin cevabından kararı ayıklar.
///
/// Ayrı fonksiyon olmasının sebebi test edilebilirlik: ağ olmadan gerçek
/// cevap metinleriyle sınanabiliyor.
pub fn parse_decision(icerik: &str) -> Result<Decision, VlmError> {
    let govde = json_govdesi(icerik)
        .ok_or_else(|| VlmError::NoDecision(icerik.chars().take(200).collect()))?;

    let v: Value = serde_json::from_str(govde)
        .map_err(|e| VlmError::Decode(format!("{e} — gövde: {}", govde.chars().take(200).collect::<String>())))?;

    // Model bazen kararı `<tool_call>` biçiminde, argümanları `arguments`
    // altında veriyor. İkisini de aynı gövdeye indirgiyoruz.
    let v = if let Some(args) = v.get("arguments") {
        let ad = v.get("name").and_then(Value::as_str).unwrap_or("");
        let mut icra = args.clone();
        if ad == "zoom_range" && icra.get("zoom").is_none() {
            icra = json!({ "zoom": args });
        }
        icra
    } else {
        v
    };

    if let Some(z) = v.get("zoom").filter(|z| !z.is_null()) {
        let t0 = z["t0_ms"].as_u64().unwrap_or(0);
        let t1 = z["t1_ms"].as_u64().unwrap_or(0);
        // Ters ya da boş aralıkta klip üretilemez; yakınlaştırma yok sayılır ve
        // varsa rapora düşülür.
        if t1 > t0 {
            return Ok(Decision::ZoomRange {
                t0_ms: t0,
                t1_ms: t1,
            });
        }
    }

    if v.get("summary").is_some() || v.get("events").is_some() {
        let rapor: RawReport = serde_json::from_value(v)
            .map_err(|e| VlmError::Decode(format!("rapor: {e}")))?;
        return Ok(Decision::Report(rapor));
    }

    Err(VlmError::NoDecision(icerik.chars().take(200).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciplak_json_rapor_ayiklanir() {
        let d = parse_decision(
            r#"{"summary":"Forklift rafa çarptı",
                "events":[{"t_ms":12000,"event":"Raf devrildi","severity":"Yüksek"}],
                "risk":"Yüksek","actions":["Alanı boşalt"]}"#,
        )
        .unwrap();
        match d {
            Decision::Report(r) => {
                assert_eq!(r.summary, "Forklift rafa çarptı");
                assert_eq!(r.events[0].klip_ms(), 12_000);
                assert_eq!(r.actions, vec!["Alanı boşalt"]);
            }
            other => panic!("rapor beklenirken {other:?}"),
        }
    }

    #[test]
    fn kod_citi_icindeki_json_ayiklanir() {
        let d = parse_decision(
            "İşte analizim:\n```json\n{\"summary\":\"x\",\"events\":[],\"risk\":\"Orta\",\"actions\":[\"y\"]}\n```\nUmarım yardımcı olur.",
        )
        .unwrap();
        assert!(matches!(d, Decision::Report(r) if r.risk == "Orta"));
    }

    #[test]
    fn yakinlastirma_karari_ayiklanir() {
        let d = parse_decision(r#"{"zoom":{"t0_ms":12000,"t1_ms":15000}}"#).unwrap();
        assert!(matches!(
            d,
            Decision::ZoomRange { t0_ms: 12000, t1_ms: 15000 }
        ));
    }

    #[test]
    fn tool_call_sarmali_desteklenir() {
        // Sunucuda ayrıştırıcı olmadığı için model bunu düz metin döndürüyor.
        let d = parse_decision(
            "<tool_call>\n{\"name\": \"zoom_range\", \"arguments\": {\"t0_ms\": 3000, \"t1_ms\": 6000}}\n</tool_call>",
        )
        .unwrap();
        assert!(matches!(
            d,
            Decision::ZoomRange { t0_ms: 3000, t1_ms: 6000 }
        ));
    }

    #[test]
    fn ters_aralikta_yakinlastirma_yok_sayilir() {
        // Yakınlaştırma geçersiz ama rapor da varsa rapora düşülmeli.
        let d = parse_decision(
            r#"{"zoom":{"t0_ms":9000,"t1_ms":9000},"summary":"s","events":[],
                "risk":"Düşük","actions":["a"]}"#,
        )
        .unwrap();
        assert!(matches!(d, Decision::Report(_)));
    }

    #[test]
    fn bos_zoom_alani_raporu_engellemez() {
        // Model "zoom": null yazdığında rapor okunabilmeli.
        let d = parse_decision(
            r#"{"zoom":null,"summary":"s","events":[],"risk":"Orta","actions":["a"]}"#,
        )
        .unwrap();
        assert!(matches!(d, Decision::Report(_)));
    }

    #[test]
    fn mmss_zamani_okunur() {
        let d = parse_decision(
            r#"{"summary":"s","events":[{"time":"00:12","event":"a","severity":"Orta"}],
                "risk":"Orta","actions":["x"]}"#,
        )
        .unwrap();
        match d {
            Decision::Report(r) => assert_eq!(r.events[0].klip_ms(), 12_000),
            other => panic!("rapor beklenirken {other:?}"),
        }
    }

    #[test]
    fn mmss_varsa_t_ms_yerine_o_kullanilir() {
        // Model ikisini birden verdiğinde MM:SS güvenilir olan.
        let e: RawEvent = serde_json::from_str(
            r#"{"time":"00:33","t_ms":3300,"event":"a","severity":"Orta"}"#,
        )
        .unwrap();
        assert_eq!(e.klip_ms(), 33_000);
    }

    #[test]
    fn saat_iceren_bicim_de_okunur() {
        assert_eq!(mmss_ms("01:02:03"), Some(3_723_000));
        assert_eq!(mmss_ms("02:05"), Some(125_000));
        assert_eq!(mmss_ms("saçma"), None);
    }

    #[test]
    fn json_icermeyen_cevap_hata_doner() {
        let e = parse_decision("Bu videoda bir şey göremiyorum.").unwrap_err();
        assert!(matches!(e, VlmError::NoDecision(_)));
    }
}
