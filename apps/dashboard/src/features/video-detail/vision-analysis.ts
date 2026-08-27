"use client"

import { useCallback, useEffect, useState } from "react"

/**
 * Görsel analiz veri katmanı: `stream` (hareket, klip) ve `vision` (rapor).
 *
 * Panel videoları ses servisine yüklüyor; `stream` ise kendi deposunu UUID ile
 * tutuyor. İki taraf **orijinal dosya adı** üzerinden eşleşiyor — yükleme
 * sırasında dosya her iki servise de gönderildiği için ad ortak.
 */

const STREAM = process.env.NEXT_PUBLIC_STREAM_API ?? "/api/stream"
const VISION = process.env.NEXT_PUBLIC_VISION_API ?? "/api/vision"

/** Hareket profilinin tek örneği. */
export type MotionSample = {
  t_ms: number
  /** Kare farkının toplamı, 0–1 arası normalize. */
  score: number
  /** En hareketli hücrenin değeri; küçük ama şiddetli hareketi yakalar. */
  cell_peak: number
  is_scene_cut: boolean
  /** 12×8 bölgesel hareket ızgarası, satır sırasına göre düz dizi. */
  grid: number[]
}

export type Heatmap = {
  grid_w: number
  grid_h: number
  frames: MotionSample[]
}

export type DetectedEvent = {
  t_ms: number
  time: string
  event: string
  severity: "Düşük" | "Orta" | "Yüksek"
}

export type Report = {
  summary: string
  events: DetectedEvent[]
  risk: "Düşük" | "Orta" | "Yüksek"
  actions: string[]
  processing_ms?: number
}

/** Ajanın attığı tek adım — hangi pencereye, hangi hızda baktığı. */
export type AgentStep = {
  step: number
  action: string
  t0_ms: number
  t1_ms: number
  time_scale: number
  service_frames: number
  elapsed_ms: number
}

export type Outcome = { report: Report; steps: AgentStep[] }

export type StreamVideo = {
  id: string
  original_name: string
  info: { duration_ms: number; fps: number; width: number; height: number; codec: string }
}

const SERVIS_KAPALI = "görüntü servisine ulaşılamıyor"

async function iste<T>(url: string, init?: RequestInit): Promise<T> {
  let r: Response
  try {
    r = await fetch(url, init)
  } catch {
    // Ağ seviyesinde düşen fetch `TypeError` atıyor; "Failed to fetch"
    // kullanıcıya gösterilecek bir cümle değil.
    throw new Error(SERVIS_KAPALI)
  }
  if (!r.ok) {
    const govde = await r.text().catch(() => "")
    let mesaj = `${r.status}`
    try {
      const j = JSON.parse(govde)
      if (typeof j.error === "string") mesaj = j.error
    } catch {
      /* gövde JSON değilse durum kodu yeterli */
    }
    throw new Error(mesaj)
  }

  // 204 No Content veya 202 Accepted durumlarında genelde body boştur
  if (r.status === 204 || r.status === 202) {
    return null as T
  }

  const text = await r.text()
  if (!text) return null as T
  
  return JSON.parse(text) as T
}

/**
 * `stream` kaydını bulur.
 *
 * Önce rota kimliğine, sonra dosya adına bakıyor. İki servis ayrı kimlik uzayı
 * kullanıyor ve ortak olan tek şey yüklenen dosyanın adı — ama kayıt doğrudan
 * `stream` kimliğiyle de açılabilmeli.
 *
 * Ada göre eşleşme ses servisinden gelen `filename`'e bağlı; **kimliğe göre
 * eşleşme değil.** Görsel analizin ses servisi ayakta olmadan da çalışabilmesi
 * için bu ayrım şart: iki taraf birbirinden bağımsız çalışıyor.
 */
export function useStreamVideo(videoId: string | null, filename: string | null) {
  const [video, setVideo] = useState<StreamVideo | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let iptal = false
    setVideo(null)
    setError(null)

    iste<{ videos: StreamVideo[] }>(`${STREAM}/v1/videos`)
      .then(({ videos }) => {
        if (iptal) return
        const eslesen =
          videos.find((v) => v.id === videoId) ??
          (filename ? videos.find((v) => v.original_name === filename) : undefined)
        if (eslesen) setVideo(eslesen)
        else setError("bu video görüntü servisine yüklenmemiş")
      })
      .catch((e: Error) => !iptal && setError(e.message))

    return () => {
      iptal = true
    }
  }, [videoId, filename])

  return { video, error }
}

/**
 * Hareket profilini çeker.
 *
 * İlk istek videoyu baştan sona çözüp analiz ettiği için yavaş; sonrakiler
 * önbellekten geliyor.
 */
export function useHeatmap(videoId: string | null) {
  const [heatmap, setHeatmap] = useState<Heatmap | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!videoId) {
      setHeatmap(null)
      return
    }
    let iptal = false
    setLoading(true)
    setError(null)

    iste<Heatmap>(`${STREAM}/v1/videos/${videoId}/heatmap?fps=10`)
      .then((h) => !iptal && setHeatmap(h))
      .catch((e: Error) => !iptal && setError(e.message))
      .finally(() => !iptal && setLoading(false))

    return () => {
      iptal = true
    }
  }, [videoId])

  return { heatmap, loading, error }
}

/** Modele giden yükün önizlemesi — klip, istem, token tahmini. */
export type Payload = {
  pass: string
  clip: {
    t0_ms: number
    t1_ms: number
    source_span_ms: number
    duration_ms: number
    time_scale: number
    service_frames: number
    effective_fps: number
    size_bytes: number
    object_key: string
    url: string
  }
  tokens: { frame_width: number; frame_height: number; per_frame: number; total: number }
  reduction: { source_frames: number; sent_frames: number; ratio: number }
  evidence_frames: { ord: number; t_ms: number; time: string; url: string; motion_score: number; is_scene_cut: boolean }[]
}

/**
 * Analiz ve yük önizlemesi.
 *
 * İkisi de **açıkça istenince** çalışıyor, sayfa açılınca değil: her ikisi de
 * çıkarım servisine gerçek istek atıyor ve servis bütün takımlarca paylaşılıyor.
 */
/**
 * Modele gidecek istem.
 *
 * `stream` bunu artık üretmiyor: bir zamanlar üretiyordu ve `vision`'ın
 * gönderdiğiyle ayrışmıştı — panel gönderilmeyen bir metni "tam olarak bu
 * gidiyor" diye gösteriyordu. Metin artık ajanın kendi render'ından geliyor.
 */
export type PromptPreview = {
  kind: "ilk_bakis" | "yakinlastirma"
  prefix: string
  suffix: string
  joined: string
  version: { agent: string; number: number; hash: string }
  text_tokens: number
}

export function useVisionAnalysis(videoId: string | null, durationMs: number) {
  const [outcome, setOutcome] = useState<Outcome | null>(null)
  const [payload, setPayload] = useState<Payload | null>(null)
  const [prompt, setPrompt] = useState<PromptPreview | null>(null)
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Video değiştiğinde eldeki sonuç artık başka bir videoya ait.
  useEffect(() => {
    setOutcome(null)
    setPayload(null)
    setPrompt(null)
    setError(null)
  }, [videoId])

  const analyze = useCallback(async () => {
    if (!videoId) return
    setRunning(true)
    setError(null)
    try {
      await iste(`${STREAM}/v1/videos/${videoId}/analyze`, {
        method: "POST"
      })
      // Sonuç SSE üzerinden 'report' eventi ile gelecek, o yüzden setRunning(false) yapmıyoruz.
    } catch (e) {
      setError((e as Error).message)
      setRunning(false)
    }
  }, [videoId])

  const loadPayload = useCallback(
    async (range?: { t0_ms: number; t1_ms: number }) => {
      if (!videoId) return
      setError(null)
      try {
        const p = await iste<Payload>(`${STREAM}/v1/videos/${videoId}/payload`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(range ?? {}),
        })
        setPayload(p)

        const yakinlastirma = p.clip.t0_ms > 0 || p.clip.time_scale > 1.01
        
        let toolsText = null
        try {
          const tRes = await fetch("/api/tools")
          if (tRes.ok) {
            const toolsList = await tRes.json() as {name: string, title: string, description: string}[]
            if (toolsList && toolsList.length > 0) {
              toolsText = toolsList.map(t => `- ${t.name} (${t.title}): ${t.description}`).join("\n")
            }
          }
        } catch {
          // Ignore tools fetch error
        }

        const pr = await iste<PromptPreview>(`${VISION}/v1/prompts/preview`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            duration_ms: durationMs,
            tools: toolsText,
            clip: yakinlastirma
              ? {
                  t0_ms: p.clip.t0_ms,
                  t1_ms: p.clip.t1_ms,
                  object_key: p.clip.object_key,
                  duration_ms: p.clip.duration_ms,
                  time_scale: p.clip.time_scale,
                  service_frames: p.clip.service_frames,
                  effective_fps: p.clip.effective_fps,
                }
              : null,
          }),
        })
        setPrompt(pr)
      } catch (e) {
        setError((e as Error).message)
      }
    },
    [videoId, durationMs],
  )

  return { outcome, payload, prompt, running, error, analyze, loadPayload, setOutcome, setRunning }
}

/** Blob adresini panelin proxy'si üzerinden çözer. */
export function streamUrl(path: string) {
  return `${STREAM}${path}`
}

/**
 * Videonun oynatılacağı adres.
 *
 * Ses servisi `/media/...` sunuyor ama görsel analiz ona bağımlı olmamalı;
 * ses tarafı düştüğünde oynatıcı `stream`'in ham videosuna düşüyor.
 */
export function playbackSrc(filename: string | null, streamId: string | null) {
  if (filename) return `/media/${encodeURIComponent(filename)}`
  if (streamId) return `${STREAM}/v1/videos/${streamId}/raw`
  return undefined
}
