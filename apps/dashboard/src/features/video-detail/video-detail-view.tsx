"use client"

import Link from "next/link"
import { useEffect, useRef, useState } from "react"
import { ArrowLeft, Download, Pause, Play, Redo2, Undo2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Slider } from "@/components/ui/slider"
import { useAudioAnalysis } from "./audio-analysis"
import { EditorTimeline } from "./editor-timeline"
import { NowPlayingPanel } from "./now-playing-panel"
import { SafetyPanel } from "./safety-panel"
import { usePlayback } from "./use-playback"

function EditorNav({ videoId, playing, onToggle }: { videoId: string; playing: boolean; onToggle: () => void }) {
  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b bg-card px-3 md:px-5">
      <div className="flex min-w-0 items-center gap-3">
        <Button variant="ghost" size="icon" nativeButton={false} render={<Link href="/videos" />} aria-label="Videolara dön">
          <ArrowLeft />
        </Button>
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{videoId}</p>
          <p className="hidden text-[11px] text-muted-foreground sm:block">Ses olay analizi</p>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" aria-label="Geri al"><Undo2 /></Button>
        <Button variant="ghost" size="icon" aria-label="İleri al"><Redo2 /></Button>
        <Button variant="outline" onClick={onToggle}>
          {playing ? <Pause data-icon="inline-start" /> : <Play data-icon="inline-start" />}
          {playing ? "Duraklat" : "Oynat"}
        </Button>
        <Button><Download data-icon="inline-start" /> Dışa aktar</Button>
      </div>
    </header>
  )
}

/** Sunucudaki profillerle birebir aynı adlar (config.rs::PROFILES). */
const PROFILES = [
  { id: "hassas", label: "Hassas", hint: "±0,25 sn — kısa ve ani sesler, en yavaş" },
  { id: "dengeli", label: "Dengeli", hint: "±0,5 sn — çoğu video için doğru seçim" },
  { id: "isabetli", label: "Geniş", hint: "±2,5 sn — sürekli sesler, en hızlı" },
]

export function VideoDetailView({ videoId }: { videoId: string }) {
  // Rota kimliği dosya adını belirler; inference servisi aynı adı
  // INFERENCE_MEDIA_ROOT altında arar.
  const mediaFile = `${videoId}.mp4`

  const videoRef = useRef<HTMLVideoElement>(null)
  const { currentTime, duration, playing, seek, toggle, subscribe } = usePlayback(videoRef)
  const [profile, setProfile] = useState("dengeli")
  const { analysis, source, error, nameOf, severityOf } = useAudioAnalysis(mediaFile, profile)
  const [threshold, setThreshold] = useState(35)

  // Boşluk tuşu oynat/duraklat — editörlerin evrensel alışkanlığı
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "BUTTON") return
      if (e.code === "Space") {
        e.preventDefault()
        toggle()
      } else if (e.key === "ArrowLeft") {
        e.preventDefault()
        seek(currentTime - (e.shiftKey ? 5 : 1))
      } else if (e.key === "ArrowRight") {
        e.preventDefault()
        seek(currentTime + (e.shiftKey ? 5 : 1))
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [toggle, seek, currentTime])

  return (
    <div className="flex h-dvh min-w-[720px] flex-col overflow-hidden bg-background">
      <EditorNav videoId={videoId} playing={playing} onToggle={toggle} />

      <main className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_280px] gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_330px]">
        {/* sol: video üstte, zaman çizelgesi altta */}
        {/* Şeride rahat ama abartısız bir yükseklik: `auto` dibe yapıştırıyor,
            250px ise tek şeritle boş kalıyordu. */}
        <section className="grid min-h-0 min-w-0 grid-rows-[minmax(0,1fr)_175px] gap-3">
          <div className="flex min-h-0 items-center justify-center rounded-xl border bg-card p-3">
            <video
              ref={videoRef}
              src={`/media/${mediaFile}`}
              className="max-h-full max-w-full rounded-lg bg-muted"
              onClick={toggle}
              controls
              playsInline
            />
          </div>

          <EditorTimeline
            analysis={analysis}
            source={source}
            error={error}
            nameOf={nameOf}
            severityOf={severityOf}
            duration={duration}
            threshold={threshold / 100}
            onSeek={seek}
            subscribe={subscribe}
          />
        </section>

        {/* sağ: o anda duyulan sesler */}
        <div className="flex min-h-0 flex-col gap-3">
          <NowPlayingPanel
            analysis={analysis}
            source={source}
            currentTime={currentTime}
            threshold={threshold / 100}
            nameOf={nameOf}
          />
          <SafetyPanel analysis={analysis} onSeek={seek} />

          <div className="flex shrink-0 flex-col gap-3 rounded-xl border bg-card px-4 py-3">
            {/* Profil zaman çözünürlüğünü belirler: küçük adım = daha kesin
                zamanlama ama daha çok pencere, dolayısıyla daha uzun analiz.
                Değişince analiz yeniden çalışır. */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-baseline justify-between">
                <span className="text-xs text-muted-foreground">Zaman çözünürlüğü</span>
                <span className="font-mono text-[10px] text-muted-foreground">
                  {analysis ? `${analysis.model.window_sec}s / ${analysis.model.hop_sec}s` : "—"}
                </span>
              </div>
              <div className="flex gap-1">
                {PROFILES.map((p) => (
                  <Button
                    key={p.id}
                    variant={profile === p.id ? "secondary" : "ghost"}
                    size="xs"
                    className="flex-1"
                    onClick={() => setProfile(p.id)}
                    title={p.hint}
                    aria-pressed={profile === p.id}
                  >
                    {p.label}
                  </Button>
                ))}
              </div>
              <span className="text-[10px] text-muted-foreground">
                {PROFILES.find((p) => p.id === profile)?.hint}
              </span>
            </div>

            <div className="flex items-center gap-3 border-t pt-3">
              <span className="shrink-0 text-xs text-muted-foreground">Eşik</span>
              <Slider
                value={[threshold]}
                onValueChange={(v) => setThreshold(Array.isArray(v) ? v[0] : v)}
                min={5}
                max={95}
                aria-label="En düşük güven eşiği"
              />
              <span className="w-9 shrink-0 text-right font-mono text-xs tabular-nums">%{threshold}</span>
            </div>
          </div>
        </div>
      </main>
    </div>
  )
}
