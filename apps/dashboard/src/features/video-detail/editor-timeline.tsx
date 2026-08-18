"use client"

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { Crosshair, Minus, Plus, Scan } from "lucide-react"
import { Button } from "@/components/ui/button"
import type { Analysis, Severity } from "./audio-analysis"
import { formatTime } from "./time"

type Props = {
  analysis: Analysis | null
  duration: number
  threshold: number
  nameOf: (index: number) => string
  /** Sınıf indeksi → iş güvenliği önem derecesi; blok rengini bu belirler. */
  severityOf: (index: number) => Severity | null
  onSeek: (seconds: number, autoplay?: boolean) => void
  /** Her karede ham zamanı veren abonelik — kafa React'e uğramadan sürülür. */
  subscribe: (fn: (t: number) => void) => () => void
}

const TRACK_H = 44
/** Bu genişliğin altındaki bloklar görünmez ve tıklanamaz hâle geliyor. */
const MIN_BLOCK_W = 5
/** Kritik/uyarı blokları için daha geniş taban: kaza sesleri kısa olur
 *  (cam kırılması ~0,7 sn) ve uzaklaşınca 1 piksele inip kayboluyordu. */
const MIN_ALERT_BLOCK_W = 7
/** Blok bu kadar genişse adı içine yazılabilir. */
const LABEL_IN_BLOCK_W = 34

/**
 * Önem derecesi → renk.
 *
 * Aynı kırmızı ve kehribar sağdaki güvenlik panelinde de kullanılıyor: bir
 * olay iki yerde görünüyorsa iki yerde de aynı renkte görünmeli, yoksa
 * kullanıcı bunların aynı şey olduğunu anlamıyor. `info` (makine sesi,
 * konuşma) bilerek dışarıda: o bir uyarı değil, bağlam.
 */
const ALERT_PAINT = {
  critical: { bg: "var(--destructive)", fg: "#FFFFFF", label: "Kritik" },
  warning: { bg: "#F5A524", fg: "#231803", label: "Uyarı" },
} as const

type AlertLevel = keyof typeof ALERT_PAINT

/** `info` ve güvenlik dışı sınıflar tek gruba iner: nötr mavi. */
function alertLevel(severity: Severity | null): AlertLevel | null {
  return severity === "critical" || severity === "warning" ? severity : null
}

/** Cetvel aralığı: yakınlaştırmaya göre okunur bir adım seç. */
function tickStep(pxPerSec: number) {
  const targetPx = 68
  const candidates = [0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600]
  return candidates.find((c) => c * pxPerSec >= targetPx) ?? 900
}

type Segment = {
  classIndex: number
  start: number
  end: number
  peak: number
  alert: AlertLevel | null
}

export function EditorTimeline({ analysis, duration, threshold, nameOf, severityOf, onSeek, subscribe }: Props) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const headRef = useRef<HTMLDivElement>(null)
  const timeRef = useRef<HTMLSpanElement>(null)
  const [viewW, setViewW] = useState(900)
  const [zoom, setZoom] = useState(1) // 1 = tümü sığar
  const [follow, setFollow] = useState(true)
  const dragging = useRef(false)
  const autoZoomed = useRef(false)

  const total = duration || analysis?.media.duration_sec || 1
  const trackW = Math.max(viewW - 24, 240)
  const basePxPerSec = trackW / total
  const pxPerSec = basePxPerSec * zoom
  const contentW = total * pxPerSec

  /**
   * Her an için **yalnız baskın sesi** göster.
   *
   * Sınıf başına ayrı şerit denendi ve okunmuyordu: 12 seyrek satır, çoğu boş,
   * bloklar birkaç piksel. Tek satırda ardışık etiketli bloklar hem video
   * editörü mantığına uyuyor hem de "şu an ne oluyor" sorusunu doğrudan
   * cevaplıyor. Ayrıntı sağdaki panelde zaten var.
   */
  const segments = useMemo<Segment[]>(() => {
    const frames = analysis?.frames
    if (!frames?.length) return []
    const hop = analysis?.model.hop_sec ?? 0.5

    // 1) Kare başına baskın sınıf
    const raw: (number | null)[] = frames.map((f) => {
      const best = f.top[0]
      return best && best[1] >= threshold ? best[0] : null
    })

    // 2) Titreme süzgeci (3'lü medyan). Benzer sınıflar (Uğultu ↔ Şebeke
    //    uğultusu) yarım saniyede bir yer değiştirip onlarca okunmaz parça
    //    üretiyordu; komşuları hemfikirse tek kareyi onlara uydur.
    const smooth = raw.slice()
    for (let i = 1; i < raw.length - 1; i++) {
      if (raw[i - 1] !== null && raw[i - 1] === raw[i + 1] && raw[i] !== raw[i - 1]) {
        smooth[i] = raw[i - 1]
      }
    }

    // 3) Ardışık aynı sınıfları birleştir.
    //    `frame.t` pencerenin başlangıcı, ama pencere `[t, t+window]` aralığını
    //    anlatıyor — blok, tarif ettiği sesin üstünde dursun diye yarım pencere
    //    kaydırılıyor. Kaydırmazsak bloklar videodan window/2 kadar önde görünür.
    const center = (analysis?.model.window_sec ?? 0) / 2
    const out: Segment[] = []
    let cur: Segment | null = null
    for (let i = 0; i < smooth.length; i++) {
      const cls = smooth[i]
      const score = frames[i].top[0]?.[1] ?? 0
      const at = frames[i].t + center
      if (cls !== null && cur && cur.classIndex === cls) {
        cur.end = at + hop
        cur.peak = Math.max(cur.peak, score)
      } else {
        if (cur) out.push(cur)
        cur =
          cls === null
            ? null
            : { classIndex: cls, start: at, end: at + hop, peak: score, alert: alertLevel(severityOf(cls)) }
      }
    }
    if (cur) out.push(cur)
    return out
  }, [analysis, threshold, severityOf])

  /** Başlıktaki sayaç: "hiç kırmızı yok" ile "arayüz bozuk" ayrımı için. */
  const alertCounts = useMemo(() => {
    let critical = 0
    let warning = 0
    for (const s of segments) {
      if (s.alert === "critical") critical++
      else if (s.alert === "warning") warning++
    }
    return { critical, warning }
  }, [segments])

  useLayoutEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const ro = new ResizeObserver(() => setViewW(el.clientWidth))
    ro.observe(el)
    setViewW(el.clientWidth)
    return () => ro.disconnect()
  }, [])

  /**
   * Açılışta ~75 saniyelik pencere göster, tüm videoyu değil.
   *
   * "Tümünü sığdır" varsayılanı 9 dakikalık videoda blokları 14 piksele
   * indiriyordu: isim sığmıyor, okunacak bir şey kalmıyordu. Takip modu zaten
   * şeridi oynatmayla birlikte kaydırdığı için okunabilir yakınlık daha
   * kullanışlı. Genel görünüm "Tümünü sığdır" düğmesinde duruyor.
   */
  useEffect(() => {
    if (autoZoomed.current || total <= 90) return
    autoZoomed.current = true
    setZoom(Math.min(60, Math.max(1, total / 75)))
  }, [total])

  // Kafa: React durumu değil, doğrudan DOM. Her karede transform ile kayar.
  useEffect(() => {
    return subscribe((t) => {
      const x = t * pxPerSec
      if (headRef.current) headRef.current.style.transform = `translateX(${x}px)`
      if (timeRef.current) timeRef.current.textContent = formatTime(t, true)

      if (!follow || dragging.current) return
      const el = scrollRef.current
      if (!el) return
      const left = el.scrollLeft
      // Kafa görünür alanın %80'ini geçince ileri kaydır
      if (x < left || x > left + el.clientWidth * 0.8) {
        el.scrollLeft = Math.max(0, x - el.clientWidth * 0.3)
      }
    })
  }, [subscribe, pxPerSec, follow])

  /**
   * Kullanıcı elle kaydırmaya başlarsa takibi kapat.
   *
   * Takip her karede scrollLeft'i kafaya çekiyor; bu açıkken kaydırma çubuğunu
   * sürüklemek imkânsızdı — el bırakmadan konum geri alınıyordu. Artık tekerlek
   * ya da işaretçiyle müdahale takibi durduruyor, "Kafaya dön" düğmesi geri
   * açıyor. Log görüntüleyicilerdeki "tail" davranışının aynısı.
   */
  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const stopFollow = (e: Event) => {
      // Ctrl+tekerlek yakınlaştırma; o takibi bozmamalı
      if (e.type === "wheel" && ((e as WheelEvent).ctrlKey || (e as WheelEvent).metaKey)) return
      setFollow((f) => (f ? false : f))
    }
    el.addEventListener("wheel", stopFollow, { passive: true })
    el.addEventListener("pointerdown", stopFollow)
    return () => {
      el.removeEventListener("wheel", stopFollow)
      el.removeEventListener("pointerdown", stopFollow)
    }
  }, [])

  /** Kafayı görünür alanın ortasına getirir ve takibi yeniden açar. */
  const backToPlayhead = useCallback(() => {
    const el = scrollRef.current
    if (el && headRef.current) {
      const x = parseFloat(headRef.current.style.transform.replace(/[^0-9.-]/g, "")) || 0
      el.scrollLeft = Math.max(0, x - el.clientWidth * 0.3)
    }
    setFollow(true)
  }, [])

  const seekFromPointer = useCallback(
    (clientX: number, autoplay = false) => {
      const el = scrollRef.current
      if (!el) return
      const rect = el.getBoundingClientRect()
      const x = clientX - rect.left + el.scrollLeft
      onSeek(Math.max(0, Math.min(x / pxPerSec, total)), autoplay)
    },
    [onSeek, pxPerSec, total],
  )

  useEffect(() => {
    const move = (e: PointerEvent) => {
      if (!dragging.current) return
      e.preventDefault()
      seekFromPointer(e.clientX)
    }
    const up = () => {
      dragging.current = false
    }
    window.addEventListener("pointermove", move)
    window.addEventListener("pointerup", up)
    return () => {
      window.removeEventListener("pointermove", move)
      window.removeEventListener("pointerup", up)
    }
  }, [seekFromPointer])

  const zoomBy = useCallback(
    (factor: number) => setZoom((z) => Math.min(60, Math.max(1, z * factor))),
    [],
  )

  // Ctrl/Cmd + tekerlek: imlecin altındaki an sabit kalarak yakınlaş.
  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return
      e.preventDefault()
      const rect = el.getBoundingClientRect()
      const pointerX = e.clientX - rect.left + el.scrollLeft
      const timeAtPointer = pointerX / pxPerSec
      const next = Math.min(60, Math.max(1, zoom * (e.deltaY < 0 ? 1.25 : 1 / 1.25)))
      setZoom(next)
      requestAnimationFrame(() => {
        el.scrollLeft = timeAtPointer * (basePxPerSec * next) - (e.clientX - rect.left)
      })
    }
    el.addEventListener("wheel", onWheel, { passive: false })
    return () => el.removeEventListener("wheel", onWheel)
  }, [pxPerSec, zoom, basePxPerSec])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === "INPUT" || tag === "TEXTAREA") return
      if (e.key === "+" || e.key === "=") { e.preventDefault(); zoomBy(1.5) }
      else if (e.key === "-" || e.key === "_") { e.preventDefault(); zoomBy(1 / 1.5) }
      else if (e.key === "0") { e.preventDefault(); setZoom(1) }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [zoomBy])

  const step = tickStep(pxPerSec)
  const ticks: number[] = []
  for (let t = 0; t <= total; t += step) ticks.push(t)

  return (
    // `min-w-0` şart: grid/flex içinde bir öğe, içeriği genişse kutusunu büyütür.
    // O durumda kaydırma kutusu kırpmak yerine yayılıyor ve genişlik ölçümü
    // (viewW → pxPerSec → içerik genişliği) kendi kendini besleyen döngüye giriyor.
    <div className="flex min-h-0 w-full min-w-0 flex-col gap-2 rounded-xl border bg-card p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-baseline gap-2">
          <h2 className="text-sm font-semibold">Zaman çizelgesi</h2>
          <span ref={timeRef} className="font-mono text-xs tabular-nums text-primary">
            {formatTime(0, true)}
          </span>
          <span className="font-mono text-[11px] tabular-nums text-muted-foreground">
            / {formatTime(total)}
          </span>
          {!analysis ? (
            <span className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
              <span className="size-1.5 animate-pulse rounded-full bg-primary" />
              Ses analiz ediliyor
            </span>
          ) : (
            /* Renk anahtarı + sayaç. Sayaç önemli: sıfır görmek "bu kayıtta
               kritik ses yok" demek; anahtarın hiç görünmemesi ise arayüzün
               bozuk olduğu izlenimi veriyordu. */
            <span className="flex items-center gap-2.5 text-[10px] text-muted-foreground">
              {(["critical", "warning"] as const).map((lvl) => (
                <span key={lvl} className="flex items-center gap-1 tabular-nums">
                  <span className="size-2 shrink-0 rounded-[2px]" style={{ background: ALERT_PAINT[lvl].bg }} />
                  {ALERT_PAINT[lvl].label} {lvl === "critical" ? alertCounts.critical : alertCounts.warning}
                </span>
              ))}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant={follow ? "secondary" : "outline"}
            size="xs"
            onClick={follow ? () => setFollow(false) : backToPlayhead}
            title={follow ? "Takip açık — kaydırınca kendiliğinden kapanır" : "Oynatma kafasına dön ve takibi aç"}
          >
            {follow ? "Takip açık" : <><Crosshair data-icon="inline-start" /> Kafaya dön</>}
          </Button>
          <Button variant="ghost" size="icon-xs" onClick={() => zoomBy(1 / 1.5)} aria-label="Uzaklaştır"><Minus /></Button>
          <span className="w-9 text-center font-mono text-[11px] tabular-nums text-muted-foreground">{zoom.toFixed(1)}×</span>
          <Button variant="ghost" size="icon-xs" onClick={() => zoomBy(1.5)} aria-label="Yakınlaştır"><Plus /></Button>
          <Button variant="ghost" size="icon-xs" onClick={() => setZoom(1)} aria-label="Tümünü sığdır"><Scan /></Button>
        </div>
      </div>

      {/* Kalan alanı kaplar ama şerit dikeyde ortalanır: hem dibe yapışmaz
          hem de altta kocaman boşluk kalmaz. */}
      <div ref={scrollRef} className="relative flex min-h-0 w-full min-w-0 flex-1 flex-col justify-center overflow-x-auto overflow-y-hidden">
        <div className="relative" style={{ width: contentW }}>
          {/* cetvel — üzerine tıkla/sürükle ile tarama */}
          <div
            className="sticky top-0 z-20 h-6 cursor-ew-resize bg-card"
            onPointerDown={(e) => {
              dragging.current = true
              seekFromPointer(e.clientX)
            }}
          >
            {ticks.map((t) => (
              <div key={t} className="absolute top-0 h-full border-l border-border" style={{ left: t * pxPerSec }}>
                <span className="ml-1 font-mono text-[10px] tabular-nums text-muted-foreground">{formatTime(t)}</span>
              </div>
            ))}
          </div>

          {/* oynatma kafası — DOM'dan sürülüyor */}
          <div
            ref={headRef}
            className="pointer-events-none absolute top-0 left-0 z-30 w-px bg-primary"
            style={{ height: 24 + TRACK_H + 8 }}
            aria-hidden
          >
            <span className="absolute -left-[3px] top-0 size-[7px] rounded-full bg-primary" />
          </div>

          {/* Güvenlik bulguları — kritik kırmızı, uyarı kehribar.
              Şeridin üstünde ayrı bir sıra: göz önce buraya düşsün.

              Aralık dolu bir çubukla çizilmiyor: "alarm sonrası faaliyet"
              bulgusu 81 saniyelik bir aralığı kapsıyor ama faaliyet o sürede
              aralıklı; dolu kırmızı bir duvar kesintisiz sürdüğünü ima
              ediyordu. Kural metni bunu zaten "5 ayrı aralıkta" diye
              söylüyor, çizim de aynı şeyi söylemeli. Bu yüzden aralık soluk
              bir bant, başlangıç anı ise dolu bir kapak. */}
          {(analysis?.safety?.findings.length ?? 0) > 0 && (
            <div className="relative mt-2 h-3">
              {analysis!.safety.findings.map((f, i) => {
                const w = Math.max(4, (f.end_sec - f.start_sec) * pxPerSec)
                const col = f.severity === "critical" ? "var(--destructive)" : "#F5A524"
                return (
                  <button
                    key={i}
                    type="button"
                    onClick={() => onSeek(f.start_sec, true)}
                    className="absolute inset-y-0 rounded-r-[2px] transition-[filter] hover:brightness-125 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
                    style={{
                      left: f.start_sec * pxPerSec,
                      width: w,
                      background: `color-mix(in oklab, ${col} 22%, transparent)`,
                      borderLeft: `3px solid ${col}`,
                    }}
                    title={`${f.title} · ${formatTime(f.start_sec)} — ${f.detail}`}
                    aria-label={`${f.title}, ${formatTime(f.start_sec)} konumuna git`}
                  />
                )
              })}
            </div>
          )}

          {/* baskın ses şeridi */}
          <div className="relative mt-2 mb-1 overflow-hidden rounded-md bg-background" style={{ height: TRACK_H }}>
            {!analysis ? (
              /* Boş kutu yerine şeridin dolacağı yeri gösteren iskelet: kullanıcı
                 neyin geleceğini görüyor, arayüz "bozuk" hissi vermiyor. */
              <div className="absolute inset-1 flex items-center gap-2" aria-hidden>
                {[26, 12, 40, 18, 30, 14, 52, 22, 36, 16, 44, 20].map((w, i) => (
                  <span
                    key={i}
                    className="h-full shrink-0 animate-pulse rounded-[4px] bg-muted"
                    style={{ width: `${w * 4}px`, animationDelay: `${i * 90}ms` }}
                  />
                ))}
              </div>
            ) : segments.length === 0 ? (
              <p className="flex h-full items-center justify-center text-xs text-muted-foreground">
                Bu eşiğin üzerinde ses yok — eşiği düşürmeyi deneyin.
              </p>
            ) : (
              segments.map((s, i) => {
                const paint = s.alert ? ALERT_PAINT[s.alert] : null
                const w = Math.max(paint ? MIN_ALERT_BLOCK_W : MIN_BLOCK_W, (s.end - s.start) * pxPerSec)
                const name = nameOf(s.classIndex)
                return (
                  <button
                    key={i}
                    type="button"
                    onClick={() => onSeek(s.start, true)}
                    className="absolute inset-y-1 flex items-center overflow-hidden rounded-[4px] border px-1.5 transition-[filter] hover:brightness-125 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
                    style={{
                      left: s.start * pxPerSec,
                      width: w,
                      background: paint ? paint.bg : "var(--primary)",
                      borderColor: paint ? paint.bg : "color-mix(in oklab, var(--primary) 40%, transparent)",
                      color: paint ? paint.fg : "var(--primary-foreground)",
                      // Güven değeri opaklığa yansır; zayıf tespitler soluk.
                      // Uyarılarda taban yüksek tutuluyor: %36'lık gerçek bir
                      // kaza sesi soluklaşıp gözden kaçmamalı.
                      opacity: paint ? 0.78 + s.peak * 0.22 : 0.5 + s.peak * 0.5,
                      // Geniş taban komşu bloğun üstüne taşabilir; uyarı üstte kalsın.
                      zIndex: paint ? 2 : 1,
                    }}
                    title={`${name} · ${formatTime(s.start)}–${formatTime(s.end)} · %${Math.round(s.peak * 100)}${paint ? ` · ${paint.label}` : ""}`}
                    aria-label={`${paint ? `${paint.label}: ` : ""}${name}, ${formatTime(s.start)} konumuna git`}
                  >
                    {w >= LABEL_IN_BLOCK_W && (
                      <span className="truncate text-[11px] font-medium leading-none">{name}</span>
                    )}
                  </button>
                )
              })
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
