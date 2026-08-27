"use client"

import { useEffect, useMemo, useRef } from "react"
import type { Heatmap, MotionSample } from "./vision-analysis"

/**
 * Hareket ısı haritasını videonun üstüne bindirir.
 *
 * Amaç boru hattının **neye baktığını** görünür kılmak: sistem kaza tespit
 * etmiyor, hareket ölçüyor. Kaydın hangi bölgesinin neden seçildiği ancak
 * böyle denetlenebiliyor.
 *
 * Çizim oynatmayla eşzamanlı ama React'e uğramıyor — `subscribe` her karede ham
 * zamanı veriyor, kare başına render çok pahalı olurdu.
 */
export function MotionOverlay({
  heatmap,
  subscribe,
  opacity = 0.5,
}: {
  heatmap: Heatmap | null
  subscribe: (fn: (t: number) => void) => () => void
  /** 0 = kapalı. Kullanıcı görüntüyü boğmasın diye ayarlanabilir. */
  opacity?: number
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const opacityRef = useRef(opacity)
  opacityRef.current = opacity

  /**
   * Izgaranın ölçeği.
   *
   * `grid` sunucuda 0–255 aralığında üretiliyor ama gerçek kayıtlarda değerler
   * çok küçük kalıyor: bir depo kaydında çarpışma anında bile en yüksek hücre
   * 255 üzerinden 4'tü. Doğrudan `v/255` kullanmak ısı haritasını tümüyle
   * görünmez yapıyor, `v`'yi 0–1 saymak ise 1'in üstündeki her şeyi tam
   * parlaklığa kırpıp kareyi yıkıyor — ilk deneme tam olarak böyle oldu.
   *
   * Bu yüzden ölçek **profilin kendi dağılımından** çıkarılıyor: yüzde 99'luk
   * dilim tam parlaklık sayılıyor. Böylece sessiz kareler karanlık kalırken
   * hareketli anlarda dağılım okunabiliyor.
   */
  const olcek = useMemo(() => {
    if (!heatmap || heatmap.frames.length === 0) return 1
    const hepsi: number[] = []
    // Her kareyi taramak uzun kayıtlarda pahalı; seyrek örnekleme yeterli.
    const adim = Math.max(1, Math.floor(heatmap.frames.length / 120))
    for (let i = 0; i < heatmap.frames.length; i += adim) {
      for (const v of heatmap.frames[i].grid) if (v > 0) hepsi.push(v)
    }
    if (hepsi.length === 0) return 1
    hepsi.sort((a, b) => a - b)
    return Math.max(1, hepsi[Math.floor(hepsi.length * 0.99)])
  }, [heatmap])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !heatmap || heatmap.frames.length === 0) return

    const ctx = canvas.getContext("2d")
    if (!ctx) return

    const { grid_w: gw, grid_h: gh, frames } = heatmap

    /** Zamana en yakın örnek. İkili arama: profil binlerce örnek taşıyabiliyor. */
    const yakinOrnek = (tMs: number): MotionSample => {
      let lo = 0
      let hi = frames.length - 1
      while (lo < hi) {
        const orta = (lo + hi) >> 1
        if (frames[orta].t_ms < tMs) lo = orta + 1
        else hi = orta
      }
      const sonra = frames[lo]
      const once = frames[Math.max(0, lo - 1)]
      return Math.abs(once.t_ms - tMs) <= Math.abs(sonra.t_ms - tMs) ? once : sonra
    }

    let son: MotionSample | null = null

    const ciz = (tSaniye: number) => {
      const ornek = yakinOrnek(tSaniye * 1000)
      const alfa = opacityRef.current

      if (ornek === son && canvas.dataset.alfa === String(alfa)) return
      son = ornek
      canvas.dataset.alfa = String(alfa)

      // Tuval hücre ızgarası çözünürlüğünde; CSS onu videoya kadar esnetiyor.
      // Böylece hem ucuz hem de hücreler arası geçiş tarayıcı tarafından
      // yumuşatılıyor.
      if (canvas.width !== gw || canvas.height !== gh) {
        canvas.width = gw
        canvas.height = gh
      }

      ctx.clearRect(0, 0, gw, gh)
      if (alfa <= 0) return

      for (let y = 0; y < gh; y++) {
        for (let x = 0; x < gw; x++) {
          const v = (ornek.grid[y * gw + x] ?? 0) / olcek
          // Eşik: hareketli bir sahnede hücrelerin çoğu bir miktar değer
          // taşıyor ve hepsini boyamak kareyi tamamen kapatıyor. Isı
          // haritasının işi *nereye bakıldığını* göstermek, görüntünün yerine
          // geçmek değil.
          if (v < 0.25) continue
          ctx.fillStyle = renk(v, alfa)
          ctx.fillRect(x, y, 1, 1)
        }
      }
    }

    ciz(0)
    return subscribe((t) => ciz(t))
  }, [heatmap, subscribe, olcek])

  if (!heatmap) return null

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      className="pointer-events-none absolute inset-0 h-full w-full rounded-lg"
      // `screen` karışımı koyu bölgeleri olduğu gibi bırakıp yalnız hareketi
      // aydınlatıyor; normal alfa harmanı görüntünün tamamını soluklaştırıyordu.
      style={{ mixBlendMode: "screen" }}
    />
  )
}

/**
 * Düşük → yüksek hareket için renk.
 *
 * Mavi–camgöbeği–sarı sırası bilinçli: kırmızı iş güvenliği arayüzünde "risk"
 * demek, oysa burada ölçülen yalnızca hareket. Renk bir yargı taşımamalı.
 *
 * Saydamlık **karesel** artıyor. Doğrusal ölçekte orta şiddetteki hücreler bile
 * belirgin boyanıyordu ve `screen` harmanıyla birleşince kare tamamen
 * yıkanıyordu; kare alınca yalnız tepe noktaları öne çıkıyor.
 */
function renk(v: number, alfa: number) {
  const t = Math.min(1, Math.max(0, v))
  const r = Math.round(255 * Math.min(1, Math.max(0, t * 2 - 0.6)))
  const g = Math.round(255 * Math.min(1, t * 1.6))
  const b = Math.round(255 * Math.min(1, Math.max(0, 1 - t * 1.8)))
  return `rgba(${r},${g},${b},${t * t * alfa})`
}
