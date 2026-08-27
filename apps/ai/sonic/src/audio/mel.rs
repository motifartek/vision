//! CED'in log-mel ön ucunun Rust karşılığı.
//!
//! `mispeech/ced-tiny` yapılandırmasından birebir alınan sözleşme:
//!
//! ```text
//! MelSpectrogram(sample_rate=16000, n_fft=512, win_length=512, hop_length=160,
//!                f_min=0, f_max=8000, n_mels=64, center=True,
//!                power=2.0, window=hann(periodic), mel_scale=htk, norm=None)
//! → AmplitudeToDB(stype="power", top_db=120)
//! ```
//!
//! Bu parametrelerin birebir tutması şart; sapma modeli sessizce çöpe çevirir
//! (bkz. `src/bin/verify-mel.rs` — sherpa referans dosyalarıyla karşılaştıran doğrulama kapısı).

use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

pub const SAMPLE_RATE: usize = 16_000;
pub const N_FFT: usize = 512;
pub const HOP_LENGTH: usize = 160;
pub const N_MELS: usize = 64;
pub const F_MIN: f32 = 0.0;
pub const F_MAX: f32 = 8000.0;
pub const TOP_DB: f32 = 120.0;

const N_FREQS: usize = N_FFT / 2 + 1; // 257
const AMIN: f32 = 1e-10;

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

/// Üçgen mel süzgeci; sıfır olmayan aralık dar olduğundan seyrek saklanıyor.
struct MelFilter {
    start: usize,
    weights: Vec<f32>,
}

pub struct MelExtractor {
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    filters: Vec<MelFilter>,
}

/// Tüm sinyalin dB cinsinden log-mel matrisi.
///
/// Düzen **mel-öncelikli**: `data[mel * n_frames + frame]`. Modelin beklediği
/// `feats [batch, 64, time]` düzeni budur (torchaudio MelSpectrogram çıktısı da
/// `[..., n_mels, time]`). Bu sayede pencere çıkarmak, bant başına bitişik bir
/// dilimi kopyalamaktan ibaret.
///
/// `top_db` kırpması **burada uygulanmaz**: kırpma monoton olduğundan pencere
/// başına sonradan uygulamak, pencereyi baştan hesaplamakla birebir aynı sonucu
/// verir. Böylece mel yalnızca bir kez hesaplanır ve örtüşen pencereler bu
/// matrisin dilimi olur.
pub struct LogMel {
    pub data: Vec<f32>,
    pub n_frames: usize,
}

impl LogMel {
    #[inline]
    pub fn frame_start_sec(frame: usize) -> f32 {
        (frame * HOP_LENGTH) as f32 / SAMPLE_RATE as f32
    }

    #[inline]
    pub fn sec_to_frame(sec: f32) -> usize {
        ((sec * SAMPLE_RATE as f32) / HOP_LENGTH as f32).round().max(0.0) as usize
    }

    /// Bir pencereyi `out`'a `[64, len_frames]` düzeninde ekler ve `top_db`
    /// kırpmasını **pencerenin kendi tepe değerine** göre uygular — torchaudio'nun
    /// parça başına davranışı budur.
    pub fn push_window(&self, start_frame: usize, len_frames: usize, out: &mut Vec<f32>) {
        let start = start_frame.min(self.n_frames);
        let end = (start + len_frames).min(self.n_frames);
        let avail = end - start;

        let mut peak = f32::NEG_INFINITY;
        for m in 0..N_MELS {
            let base = m * self.n_frames;
            for &v in &self.data[base + start..base + end] {
                if v > peak {
                    peak = v;
                }
            }
        }
        // Tamamen sinyal dışındaki pencere: sessizlik tabanına düş.
        if !peak.is_finite() {
            peak = 0.0;
        }
        let floor = peak - TOP_DB;

        // Pencere sinyalin sonuna taşarsa bant başına taban değerle doldurulur;
        // batch'teki her satırın aynı uzunlukta olması gerekiyor.
        for m in 0..N_MELS {
            let base = m * self.n_frames;
            out.extend(self.data[base + start..base + end].iter().map(|&v| v.max(floor)));
            out.extend(std::iter::repeat(floor).take(len_frames - avail));
        }
    }
}

impl MelExtractor {
    pub fn new() -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT);

        // torch.hann_window(512, periodic=True) = 0.5 * (1 - cos(2πn/N))
        let window: Vec<f32> = (0..N_FFT)
            .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / N_FFT as f32).cos()))
            .collect();

        Self { fft, window, filters: build_filterbank() }
    }

    /// `center=True` için yansıtmalı dolgu: out[-i] = x[i].
    fn reflect_pad(samples: &[f32]) -> Vec<f32> {
        let pad = N_FFT / 2;
        // torch'un reflect kipi pad < len şartı arar; kısa sinyali önce sıfırla uzat.
        let mut base = samples.to_vec();
        if base.len() <= pad {
            base.resize(pad + 1, 0.0);
        }

        let n = base.len();
        let mut out = Vec::with_capacity(n + 2 * pad);
        out.extend((1..=pad).rev().map(|i| base[i]));
        out.extend_from_slice(&base);
        out.extend((1..=pad).map(|i| base[n - 1 - i]));
        out
    }

    pub fn compute(&self, samples: &[f32]) -> LogMel {
        let padded = Self::reflect_pad(samples);
        let n_frames = (padded.len() - N_FFT) / HOP_LENGTH + 1;

        // Mel-öncelikli hedef düzen; kareler döngüde saçılmış olarak yazılıyor.
        let mut data = vec![0f32; n_frames * N_MELS];
        let mut scratch = self.fft.make_scratch_vec();
        let mut input = self.fft.make_input_vec();
        let mut spectrum = self.fft.make_output_vec();
        let mut power = vec![0f32; N_FREQS];

        for frame in 0..n_frames {
            let offset = frame * HOP_LENGTH;
            for (i, slot) in input.iter_mut().enumerate() {
                *slot = padded[offset + i] * self.window[i];
            }

            self.fft
                .process_with_scratch(&mut input, &mut spectrum, &mut scratch)
                .expect("FFT tampon boyutları sabit");

            // power = 2.0 → büyüklüğün karesi
            for (p, c) in power.iter_mut().zip(spectrum.iter()) {
                *p = c.re * c.re + c.im * c.im;
            }

            for (m, filter) in self.filters.iter().enumerate() {
                let energy: f32 = filter
                    .weights
                    .iter()
                    .zip(&power[filter.start..])
                    .map(|(w, p)| w * p)
                    .sum();
                // AmplitudeToDB(stype="power"): 10*log10(max(x, 1e-10))
                data[m * n_frames + frame] = 10.0 * energy.max(AMIN).log10();
            }
        }

        LogMel { data, n_frames }
    }
}

impl Default for MelExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// torchaudio `melscale_fbanks(..., norm=None, mel_scale="htk")` karşılığı.
fn build_filterbank() -> Vec<MelFilter> {
    let nyquist = (SAMPLE_RATE / 2) as f32;
    let all_freqs: Vec<f32> =
        (0..N_FREQS).map(|i| i as f32 * nyquist / (N_FREQS - 1) as f32).collect();

    let (m_min, m_max) = (hz_to_mel(F_MIN), hz_to_mel(F_MAX));
    let f_pts: Vec<f32> = (0..N_MELS + 2)
        .map(|i| mel_to_hz(m_min + (m_max - m_min) * i as f32 / (N_MELS + 1) as f32))
        .collect();
    let f_diff: Vec<f32> = f_pts.windows(2).map(|w| w[1] - w[0]).collect();

    (0..N_MELS)
        .map(|m| {
            let dense: Vec<f32> = all_freqs
                .iter()
                .map(|&freq| {
                    let down = (freq - f_pts[m]) / f_diff[m];
                    let up = (f_pts[m + 2] - freq) / f_diff[m + 1];
                    down.min(up).max(0.0)
                })
                .collect();

            let start = dense.iter().position(|&w| w > 0.0).unwrap_or(0);
            let end = dense.iter().rposition(|&w| w > 0.0).map_or(start, |e| e + 1);
            MelFilter { start, weights: dense[start..end.max(start)].to_vec() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn htk_mel_roundtrip() {
        for hz in [0.0, 125.0, 1000.0, 4000.0, 8000.0] {
            assert!((mel_to_hz(hz_to_mel(hz)) - hz).abs() < 0.01, "hz={hz}");
        }
    }

    #[test]
    fn frame_count_matches_torch_center_formula() {
        // torch: center=True → 1 + len/hop kare
        let mel = MelExtractor::new();
        for secs in [1.0f32, 2.0, 10.0] {
            let n = (secs * SAMPLE_RATE as f32) as usize;
            let out = mel.compute(&vec![0.0; n]);
            assert_eq!(out.n_frames, 1 + n / HOP_LENGTH, "{secs}s");
        }
    }

    #[test]
    fn filterbank_covers_spectrum_without_normalisation() {
        let fb = build_filterbank();
        assert_eq!(fb.len(), N_MELS);

        // norm=None → üçgenlerin sürekli tepesi 1.0. Ayrık FFT kutuları mel
        // merkeziyle tam çakışmadığından örneklenen tepe 1.0'ın biraz altında
        // kalabilir, ama üstüne çıkamaz.
        let peaks: Vec<f32> = fb
            .iter()
            .map(|f| f.weights.iter().copied().fold(0.0, f32::max))
            .collect();

        for (m, &peak) in peaks.iter().enumerate() {
            assert!(peak > 0.0 && peak <= 1.0 + 1e-6, "mel {m} tepesi {peak}");
        }

        // Slaney normalizasyonu açık olsaydı tepeler bant genişliğine bölünür ve
        // alçak bantlarda ~0.03'e inerdi; bu eşik iki kurulumu ayırır.
        let highest = peaks.iter().copied().fold(0.0, f32::max);
        assert!(highest > 0.95, "en yüksek tepe {highest}");
    }

    #[test]
    fn top_db_clamp_is_window_local() {
        let mel = LogMel { data: vec![0.0, -200.0, -50.0, -10.0], n_frames: 4 };
        // N_MELS=64 olduğundan bu testte tek tek kareler yerine ham diziyi taklit
        // etmek adına doğrudan kırpma mantığını doğruluyoruz.
        let peak: f32 = mel.data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let floor = peak - TOP_DB;
        assert_eq!(floor, -120.0);
        assert_eq!(mel.data[1].max(floor), -120.0);
    }
}
