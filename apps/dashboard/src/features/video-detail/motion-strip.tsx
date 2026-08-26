"use client"

import { useCallback, useEffect, useRef } from "react"
import type { DetectedEvent, Heatmap } from "./vision-analysis"

/**
 * Hareket profilinin şeridi.
 *
 * Ses şeridiyle aynı zaman eksenini paylaşıyor ama ayrı duruyor: biri neyin
 * *duyulduğunu*, diğeri neyin *görüldüğünü* gösteriyor. Üst üste bindirmek
 * ikisini de okunamaz yapıyordu.
 *
 * Üç eğri çizilir — toplam hareket, en hareketli bölge, sahne kesitleri — ve
 * ajanın raporladığı olaylar işaretlenir. Tıklamak o ana atlar.
 */
export function MotionStrip({
  heatmap,
  duration,
  events,
  analysedRange,
  onSeek,
  subscribe,
  loading,
  error,
}: {
  heatmap: Heatmap | null
  /** Saniye. Oynatıcıdan gelir; profil yokken de eksen doğru olsun diye ayrı. */
  duration: number
  events: DetectedEvent[]
  /** Modele gönderilen pencere; kaydın tamamı değilse vurgulanır. */
  analysedRange: { t0_ms: number; t1_ms: number } | null
  onSeek: (seconds: number) => void
  subscribe: (fn: (t: number) => void) => () => void
  loading: boolean
  error: string | null
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const headRef = useRef<HTMLDivElement>(null)
  const boxRef = useRef<HTMLDivElement>(null)

  const totalMs = (duration || 0) * 1000 || heatmap?.frames.at(-1)?.t_ms || 1

  // Oynatma kafası React'e uğramadan sürülüyor.
  useEffect(() => {
    return subscribe((t) => {
      const head = headRef.current
      if (!head) return
      head.style.left = `${Math.min(100, Math.max(0, ((t * 1000) / totalMs) * 100))}%`
    })
  }, [subscribe, totalMs])

  useEffect(() => {
    const canvas = canvasRef.current
    const box = boxRef.current
    if (!canvas || !box) return

    const ciz = () => {
      const w = box.clientWidth
      const h = box.clientHeight
      if (w === 0 || h === 0) return

      const dpr = window.devicePixelRatio || 1
      canvas.width = w * dpr
      canvas.height = h * dpr
      canvas.style.width = `${w}px`
      canvas.style.height = `${h}px`

      const ctx = canvas.getContext("2d")
      if (!ctx) return
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      ctx.clearRect(0, 0, w, h)

      if (!heatmap || heatmap.frames.length === 0) return

      const X = (tMs: number) => (tMs / totalMs) * w
      const Y = (v: number) => h - 4 - Math.min(1, Math.max(0, v)) * (h - 10)

      // Analiz edilen pencere: dışarıda kalan kısım karartılıyor ki modelin
      // neyi görmediği belli olsun.
      if (analysedRange && (analysedRange.t0_ms > 0 || analysedRange.t1_ms < totalMs - 500)) {
        ctx.fillStyle = "rgba(0,0,0,0.28)"
        ctx.fillRect(0, 0, X(analysedRange.t0_ms), h)
        ctx.fillRect(X(analysedRange.t1_ms), 0, w - X(analysedRange.t1_ms), h)
      }

      // Sahne kesitleri en altta, dikey çizgi olarak.
      ctx.strokeStyle = "rgba(160,140,255,0.55)"
      ctx.lineWidth = 1
      for (const f of heatmap.frames) {
        if (!f.is_scene_cut) continue
        ctx.beginPath()
        ctx.moveTo(X(f.t_ms), 0)
        ctx.lineTo(X(f.t_ms), h)
        ctx.stroke()
      }

      // En hareketli bölge: küçük ama şiddetli hareketi yakalar, toplamda
      // kaybolur. İnce ve soluk çizilir.
      ctx.strokeStyle = "rgba(120,200,255,0.6)"
      ctx.lineWidth = 1
      ctx.beginPath()
      heatmap.frames.forEach((f, i) =>
        i ? ctx.lineTo(X(f.t_ms), Y(f.cell_peak)) : ctx.moveTo(X(f.t_ms), Y(f.cell_peak)),
      )
      ctx.stroke()

      // Toplam hareket: dolgulu alan.
      ctx.beginPath()
      ctx.moveTo(0, h)
      heatmap.frames.forEach((f) => ctx.lineTo(X(f.t_ms), Y(f.score)))
      ctx.lineTo(w, h)
      ctx.closePath()
      ctx.fillStyle = "rgba(90,170,255,0.22)"
      ctx.fill()

      ctx.strokeStyle = "rgba(90,170,255,0.9)"
      ctx.lineWidth = 1.5
      ctx.beginPath()
      heatmap.frames.forEach((f, i) =>
        i ? ctx.lineTo(X(f.t_ms), Y(f.score)) : ctx.moveTo(X(f.t_ms), Y(f.score)),
      )
      ctx.stroke()
    }

    ciz()
    const ro = new ResizeObserver(ciz)
    ro.observe(box)
    return () => ro.disconnect()
  }, [heatmap, totalMs, analysedRange])

  const tikla = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const box = boxRef.current
      if (!box) return
      const oran = (e.clientX - box.getBoundingClientRect().left) / box.clientWidth
      onSeek((oran * totalMs) / 1000)
    },
    [onSeek, totalMs],
  )

  return (
    <div className="flex shrink-0 flex-col gap-1.5 rounded-xl border bg-card px-3 py-2.5">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs font-medium">Hareket profili</span>
        <div className="flex items-center gap-3 text-[10px] text-muted-foreground">
          <Legend color="rgb(90,170,255)" label="toplam" />
          <Legend color="rgb(120,200,255)" label="en hareketli bölge" />
          <Legend color="rgb(160,140,255)" label="sahne kesiti" />
          {events.length > 0 && <Legend color="rgb(240,160,60)" label="olay" />}
        </div>
      </div>

      <div
        ref={boxRef}
        onClick={tikla}
        role="presentation"
        className="relative h-[62px] cursor-pointer overflow-hidden rounded-md bg-muted/40"
        title="Tıklayın: o ana gidin"
      >
        <canvas ref={canvasRef} className="absolute inset-0" />

        {/* Ajanın raporladığı olaylar — şeridin üstünde işaret olarak. */}
        {events.map((ev, i) => (
          <button
            key={`${ev.t_ms}-${i}`}
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              onSeek(ev.t_ms / 1000)
            }}
            className="absolute top-0 h-full w-[3px] -translate-x-1/2 bg-[rgb(240,160,60)] outline-none focus-visible:ring-2 focus-visible:ring-ring"
            style={{ left: `${(ev.t_ms / totalMs) * 100}%` }}
            title={`${ev.time} — ${ev.event}`}
            aria-label={`${ev.time} ${ev.event}`}
          />
        ))}

        <div
          ref={headRef}
          className="pointer-events-none absolute top-0 h-full w-px bg-foreground"
          style={{ left: 0 }}
        />

        {(loading || error || !heatmap) && (
          <div className="absolute inset-0 flex items-center justify-center bg-card/70 text-[11px] text-muted-foreground">
            {loading
              ? "Hareket profili çıkarılıyor…"
              : error
                ? error
                : "Hareket profili yok"}
          </div>
        )}
      </div>
    </div>
  )
}

function Legend({ color, label }: { color: string; label: string }) {
  return (
    <span className="flex items-center gap-1">
      <span className="inline-block h-[3px] w-3 rounded-full" style={{ background: color }} />
      {label}
    </span>
  )
}
