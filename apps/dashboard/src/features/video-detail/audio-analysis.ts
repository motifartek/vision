"use client"

import { useEffect, useMemo, useState } from "react"
import sampleAnalysis from "./sample-analysis.json"

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

/** "live": servisten geldi, "sample": servis yok, pakete gömülü örnek veri. */
export type AnalysisSource = "live" | "sample" | "loading"

/**
 * Gateway rotası kimlik doğrulaması istiyor ve dashboard'da henüz login akışı
 * yok; bu yüzden yerel geliştirmede doğrudan inference servisine gidiyoruz.
 * Login geldiğinde NEXT_PUBLIC_AUDIO_API gateway'e çevrilir.
 */
const API = process.env.NEXT_PUBLIC_AUDIO_API ?? "/api/inference"

/** 527 etiket sayfa ömrü boyunca bir kez indirilir. */
let labelCache: Promise<ClassLabel[]> | null = null
function fetchLabels(): Promise<ClassLabel[]> {
  labelCache ??= fetch(`${API}/v1/labels`).then((r) => {
    if (!r.ok) throw new Error(`etiketler alınamadı: HTTP ${r.status}`)
    return r.json() as Promise<ClassLabel[]>
  })
  return labelCache
}

export function useAudioAnalysis(mediaPath: string, profile = "dengeli") {
  // Başlangıçta örnek veriyle doldurmuyoruz: örnek başka bir videonun analizi
  // ve onu istenen videonun zaman çizelgesine çizmek olayları yanlış yerde
  // (genelde sola yığılmış) gösteriyordu. Analiz gelene kadar `null`.
  const [analysis, setAnalysis] = useState<Analysis | null>(null)
  const [labels, setLabels] = useState<ClassLabel[] | null>(null)
  const [source, setSource] = useState<AnalysisSource>("loading")
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setAnalysis(null)
    setSource("loading")

    async function run() {
      try {
        const [data, labelList] = await Promise.all([
          fetch(`${API}/v1/audio/analyze`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              path: mediaPath,
              profile,
              threshold: 0.35,
              // Canlı okuma paneli pencere başına ilk-K sınıfı kullanıyor.
              include_frames: true,
              top_k: 6,
            }),
          }).then((r) => {
            if (!r.ok) throw new Error(`HTTP ${r.status}`)
            return r.json() as Promise<Analysis>
          }),
          fetchLabels(),
        ])
        if (cancelled) return
        setAnalysis(data)
        setLabels(labelList)
        setSource("live")
        setError(null)
      } catch (cause) {
        if (cancelled) return
        // Servis kapalıysa arayüz tamamen boş kalmasın: gömülü örnek veriye düş,
        // ama bunu kullanıcıdan gizleme (rozet "örnek veri" gösterir).
        setAnalysis(sampleAnalysis as unknown as Analysis)
        setSource("sample")
        setError(cause instanceof Error ? cause.message : "bilinmeyen hata")
      }
    }

    run()
    return () => {
      cancelled = true
    }
  }, [mediaPath, profile])

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

  return { analysis, source, error, nameOf, severityOf }
}
