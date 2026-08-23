"use client"

import { useCallback, useEffect, useRef, useState } from "react"

/**
 * Oynatma durumunu tek kaynaktan yönetir.
 *
 * `timeupdate` olayı saniyede yalnız ~4 kez tetiklenir; oynatma kafası bununla
 * sürülürse tıknaz görünür. Bu yüzden zaman `requestAnimationFrame` ile okunur.
 * React durumu ise **yalnızca 100 ms'lik dilim değiştiğinde** güncellenir —
 * böylece ağaç saniyede 60 kez değil, en fazla 10 kez yeniden çizilir.
 *
 * Kafa gibi her karede hareket etmesi gereken şeyler `subscribe` ile doğrudan
 * DOM'a yazmalı, React'e uğramadan.
 */
export function usePlayback(videoRef: React.RefObject<HTMLVideoElement | null>) {
  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [playing, setPlaying] = useState(false)

  const listeners = useRef(new Set<(t: number) => void>())
  const raf = useRef<number | null>(null)
  const lastPublished = useRef(-1)

  /** Her karede ham zamanı alan aboneler (kafa, cetvel) React'i atlar. */
  const subscribe = useCallback((fn: (t: number) => void) => {
    listeners.current.add(fn)
    return () => {
      listeners.current.delete(fn)
    }
  }, [])

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const tick = () => {
      const t = video.currentTime
      listeners.current.forEach((fn) => fn(t))

      // 100 ms'lik dilim değişmediyse React'i rahatsız etme
      const slice = Math.floor(t * 10)
      if (slice !== lastPublished.current) {
        lastPublished.current = slice
        setCurrentTime(t)
      }
      raf.current = requestAnimationFrame(tick)
    }

    const onMeta = () => setDuration(video.duration || 0)
    const onPlay = () => setPlaying(true)
    const onPause = () => setPlaying(false)
    // Duraklatılmış hâlde de sürükleme/atlama zamanı güncellemeli
    const onSeek = () => {
      listeners.current.forEach((fn) => fn(video.currentTime))
      setCurrentTime(video.currentTime)
    }

    video.addEventListener("loadedmetadata", onMeta)
    video.addEventListener("play", onPlay)
    video.addEventListener("pause", onPause)
    video.addEventListener("seeked", onSeek)
    if (video.readyState >= 1) onMeta()

    raf.current = requestAnimationFrame(tick)
    return () => {
      if (raf.current) cancelAnimationFrame(raf.current)
      video.removeEventListener("loadedmetadata", onMeta)
      video.removeEventListener("play", onPlay)
      video.removeEventListener("pause", onPause)
      video.removeEventListener("seeked", onSeek)
    }
  }, [videoRef])

  const seek = useCallback(
    (seconds: number, autoplay = false) => {
      const video = videoRef.current
      if (!video) return
      video.currentTime = Math.max(0, Math.min(seconds, video.duration || seconds))
      if (autoplay && video.paused) void video.play()
    },
    [videoRef],
  )

  const toggle = useCallback(() => {
    const video = videoRef.current
    if (!video) return
    if (video.paused) void video.play()
    else video.pause()
  }, [videoRef])

  return { currentTime, duration, playing, seek, toggle, subscribe }
}
