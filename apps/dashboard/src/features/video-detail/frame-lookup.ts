import type { Frame } from "./audio-analysis"

/**
 * Verilen ana denk gelen pencereyi ikili aramayla bulur.
 *
 * **Önemli:** `frame.t` pencerenin *başlangıcı*, ama pencere `[t, t+window]`
 * aralığını anlatıyor — yani tahminin ağırlık merkezi `t + window/2`. Bunu
 * hesaba katmazsak etiketler videonun yarım pencere kadar gerisinde kalır
 * (`dengeli` profilde tam 1 saniye). `centerOffset` = window_sec / 2.
 *
 * Doğrusal tarama 2000+ pencerede her karede yapılamaz, o yüzden ikili arama.
 */
export function frameAt(
  frames: Frame[] | undefined,
  time: number,
  centerOffset = 0,
): Frame | null {
  if (!frames || frames.length === 0) return null
  const t = time - centerOffset
  let lo = 0
  let hi = frames.length - 1
  if (t <= frames[0].t) return frames[0]
  if (t >= frames[hi].t) return frames[hi]

  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (frames[mid].t === t) return frames[mid]
    if (frames[mid].t < t) lo = mid + 1
    else hi = mid - 1
  }

  // hi < lo; hangisi t'ye daha yakınsa
  const before = frames[hi] ?? frames[0]
  const after = frames[lo] ?? frames[frames.length - 1]
  return t - before.t <= after.t - t ? before : after
}
