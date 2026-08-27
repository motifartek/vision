"use client"

import { useEffect, useMemo, useRef, useState } from "react"

export type AudioEvent = {
  class_index: number
  label: string
  label_tr: string
  mid: string
  start_sec: number
  end_sec: number
  peak_sec: number
  confidence: number
  mean_confidence: number
}

export type ClassSummary = {
  class_index: number
  label: string
  label_tr: string
  total_sec: number
  event_count: number
  peak_confidence: number
}

/** Sunucu yükü küçültmek için etiket adı yerine sınıf indeksi gönderir. */
export type Frame = { t: number; top: [number, number][] }


export type Severity = "critical" | "warning" | "info"

export type SafetyEvent = {
  label: string
  label_tr: string
  category: string
  severity: Severity
  start_sec: number
  end_sec: number
  confidence: number
}

/** Kural motorunun ürettiği bulgu — 'şu ana bak' demek, ihlal kararı değil. */
export type SafetyFinding = {
  rule: string
  severity: Severity
  start_sec: number
  end_sec: number
  title: string
  detail: string
  evidence: string[]
}

export type SafetyReport = { events: SafetyEvent[]; findings: SafetyFinding[] }

export type Analysis = {
  media: { duration_sec: number; sample_rate: number; truncated: boolean; decoder: string }
  model: {
    name: string
    weights: string
    providers: string[]
    classes: number
    profile: string
    window_sec: number
    hop_sec: number
    windows: number
    batch_size: number
  }
  events: AudioEvent[]
  /** `max_events` sınırına takıldıysa `true`; `summary` kırpmadan etkilenmez. */
  events_truncated: boolean
  summary: ClassSummary[]
  safety: SafetyReport
  frames?: Frame[]
  timing: {
    decode_ms: number
    mel_ms: number
    inference_ms: number
    segment_ms: number
    total_ms: number
    realtime_factor: number
  }
}

export type ClassLabel = {
  index: number
  mid: string
  display_name: string
  display_name_tr: string | null
  /** 527 sınıfın 57'sinde dolu; gerisi güvenlik sınıfı değil. */
  severity?: Severity
  category?: string
}

/** "live": servisten geldi, "error": analiz yapılamadı, sebebi `error` alanında. */
export type AnalysisSource = "live" | "error" | "loading"

/**
 * Gateway rotası kimlik doğrulaması istiyor ve dashboard'da henüz login akışı
 * yok; bu yüzden yerel geliştirmede doğrudan inference servisine gidiyoruz.
 * Login geldiğinde NEXT_PUBLIC_STREAM_API gateway'e çevrilir.
 */
export const API = process.env.NEXT_PUBLIC_STREAM_API ?? "/api/stream"

/** Servis kapalı/erişilemez durumunun tek metni. */
const SERVICE_DOWN = "analiz servisine ulaşılamıyor"

/** 527 etiket sayfa ömrü boyunca bir kez indirilir. */
let labelCache: Promise<ClassLabel[]> | null = null
function fetchLabels(): Promise<ClassLabel[]> {
  labelCache ??= fetch(`${API}/v1/labels`)
    .then((r) => {
      if (!r.ok) throw new Error(SERVICE_DOWN)
      return r.json() as Promise<ClassLabel[]>
    })
    .catch((cause) => {
      // Başarısız isteği önbellekte bırakmıyoruz: servis bir kez tökezlerse
      // reddedilmiş promise sonsuza dek kalıyor ve geri geldiğinde bile sayfa
      // yenilenene kadar hiçbir video analiz edilemiyordu.
      labelCache = null
      throw cause instanceof TypeError ? new Error(SERVICE_DOWN) : cause
    })
  return labelCache
}

/**
 * `mediaPath` uzantılı gerçek dosya adı olmalı (bkz. `useMediaFile`). Henüz
 * çözülmediyse `null` geçilir: istek atılmaz, durum "loading" kalır.
 *
 * `threshold` **sunucuya gider**. Eskiden istek sabit %35 ile atılıyordu:
 * kaydırıcı yalnız şeridin çizimini süzüyor, olaylar/özet/güvenlik bulguları
 * %35'te kalıyordu — aynı ekranda iki farklı eşik. Çağıran bu değeri geciktirerek
 * (debounce) versin, yoksa kaydırıcının her adımı yeni bir çözümleme başlatır.
 */
export function useAudioAnalysis(
  mediaPath: string | null,
  profile = "dengeli",
  threshold = 0.35,
) {
  // Başlangıçta örnek veriyle doldurmuyoruz: örnek başka bir videonun analizi
  // ve onu istenen videonun zaman çizelgesine çizmek olayları yanlış yerde
  // (genelde sola yığılmış) gösteriyordu. Analiz gelene kadar `null`.
  const [analysis, setAnalysis] = useState<Analysis | null>(null)
  const [labels, setLabels] = useState<ClassLabel[] | null>(null)
  const [source, setSource] = useState<AnalysisSource>("loading")
  const [error, setError] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  /** Hangi video/profil için veri elimizde: eşik değişiminde ekranı boşaltmamak için. */
  const shownFor = useRef<string | null>(null)

  useEffect(() => {
    let cancelled = false
    const key = mediaPath === null ? null : `${mediaPath}|${profile}`
    // Yalnız eşik değiştiyse eldeki analizi ekranda tutuyoruz: her kaydırma
    // adımında çizelgeyi iskelete döndürmek, tutarlılık için ödenecek bedelden
    // çok daha rahatsız edici.
    const keepVisible = key !== null && key === shownFor.current

    if (keepVisible) {
      setRefreshing(true)
    } else {
      setAnalysis(null)
      setSource("loading")
      setError(null)
    }
    // Dosya adı henüz çözülmedi ya da çözülemedi; sebebi çağıran gösteriyor.
    if (!mediaPath) return

    async function run() {
      try {
        const [data, labelList] = await Promise.all([
          fetch(`${API}/v1/audio/analyze`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              path: mediaPath,
              profile,
              threshold,
              // Canlı okuma paneli pencere başına ilk-K sınıfı kullanıyor.
              include_frames: true,
              top_k: 6,
            }),
          }).then(async (r) => {
            if (!r.ok) {
              // Servis 4xx'te `{"error": "..."}` döndürüyor ("Dosyada ses akışı
              // yok" gibi). Kullanıcıya HTTP kodu değil bu cümle gösterilmeli.
              const reason = await r
                .json()
                .then((b) => (b as { error?: string }).error)
                .catch(() => null)
              // 5xx'te gövde servisin değil, tünelin hata sayfası olur —
              // "HTTP 500" yerine sebebi insan diliyle söylemek gerekiyor.
              throw new Error(reason ?? (r.status >= 500 ? SERVICE_DOWN : `servis HTTP ${r.status} döndü`))
            }
            return r.json() as Promise<Analysis>
          }),
          fetchLabels(),
        ])
        if (cancelled) return
        setAnalysis(data)
        setLabels(labelList)
        setSource("live")
        setError(null)
        shownFor.current = key
      } catch (cause) {
        if (cancelled) return
        /**
         * Eskiden burada pakete gömülü örnek analize düşülüyordu. Sessiz bir
         * videoda ya da servis kapalıyken bu, **başka bir çekimin olaylarını**
         * bu videonun zaman çizelgesine çiziyordu; küçük "örnek veri" rozeti
         * uyarı olarak yetmiyordu. Yanlış veri göstermektense hiç göstermemek
         * doğru: analiz yoksa çizelge boş kalır ve sebebi yazılır.
         */
        setAnalysis(null)
        setSource("error")
        shownFor.current = null
        // Ağ seviyesinde düşen fetch `TypeError` atar; mesajı ("Failed to
        // fetch") kullanıcıya gösterilecek bir şey değil.
        setError(
          cause instanceof TypeError
            ? SERVICE_DOWN
            : cause instanceof Error
              ? cause.message
              : "bilinmeyen hata"
        )
      } finally {
        if (!cancelled) setRefreshing(false)
      }
    }

    run()
    return () => {
      cancelled = true
    }
  }, [mediaPath, profile, threshold])

  /** Sınıf indeksi → Türkçe ad. Etiketler gelmediyse olaylardan türetilir. */
  const nameOf = useMemo(() => {
    const fromLabels = new Map<number, string>()
    labels?.forEach((l) => fromLabels.set(l.index, l.display_name_tr ?? l.display_name))
    analysis?.events.forEach((e) => {
      if (!fromLabels.has(e.class_index)) fromLabels.set(e.class_index, e.label_tr)
    })
    analysis?.summary.forEach((s) => {
      if (!fromLabels.has(s.class_index)) fromLabels.set(s.class_index, s.label_tr)
    })
    return (index: number) => fromLabels.get(index) ?? `#${index}`
  }, [labels, analysis])

  /**
   * Sınıf indeksi → iş güvenliği önem derecesi (yoksa `null`).
   *
   * Zaman çizelgesi bloklarını buradan boyuyor. Kaynak `/v1/labels`; analizin
   * `safety.events` listesi değil. Aradaki fark eşikte ortaya çıkıyor: safety
   * olayları sunucudaki eşiğe göre süzülmüş, şerit ise kullanıcının kaydırdığı
   * eşiğe göre çiziliyor. Sabit listeyi kullanmak ikisinin ayrışmasını önlüyor.
   */
  const severityOf = useMemo(() => {
    const map = new Map<number, Severity>()
    labels?.forEach((l) => {
      if (l.severity) map.set(l.index, l.severity)
    })
    return (index: number) => map.get(index) ?? null
  }, [labels])

  return { analysis, source, error, refreshing, nameOf, severityOf }
}

