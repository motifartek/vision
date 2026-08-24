"use client"

import { useEffect, useState } from "react"
import { API } from "./audio-analysis"

/**
 * Rota kimliğinden gerçek dosya adını çözer: `/videos/test3` → `test3.mkv`.
 *
 * Eskiden dosya adı `${videoId}.mp4` diye kurulurdu. Yükleme ve listeleme sekiz
 * uzantıyı kabul ettiği için mkv/webm/mov olarak yüklenen videolar listede
 * görünüyor, açılınca hem oynatıcı 404 alıyor hem analiz "Medya dosyası
 * bulunamadı" diyordu. Uzantının tek doğru kaynağı dosya sistemi; servis
 * `GET /v1/videos/:id` ile onu söylüyor.
 */
export type MediaFile = {
  /** Uzantılı dosya adı; çözülene kadar `null`. */
  filename: string | null
  /** Dosya bulunamadıysa/servise ulaşılamadıysa sebebi. */
  error: string | null
}

const NOT_FOUND = "bu kimlikle eşleşen video dosyası yok"
const SERVICE_DOWN = "analiz servisine ulaşılamıyor"

export function useMediaFile(videoId: string): MediaFile {
  const [state, setState] = useState<MediaFile>({ filename: null, error: null })

  useEffect(() => {
    let cancelled = false
    setState({ filename: null, error: null })

    fetch(`${API}/v1/videos/${encodeURIComponent(videoId)}`)
      .then(async (r) => {
        if (r.status === 404) throw new Error(NOT_FOUND)
        if (!r.ok) throw new Error(SERVICE_DOWN)
        return (await r.json()) as { filename: string }
      })
      .then((entry) => {
        if (!cancelled) setState({ filename: entry.filename, error: null })
      })
      .catch((cause) => {
        if (cancelled) return
        setState({
          filename: null,
          // Ağ seviyesinde düşen fetch `TypeError` atar; "Failed to fetch"
          // kullanıcıya gösterilecek bir cümle değil.
          error: cause instanceof TypeError ? SERVICE_DOWN : (cause as Error).message,
        })
      })

    return () => {
      cancelled = true
    }
  }, [videoId])

  return state
}
