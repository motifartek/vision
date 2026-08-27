"use client"

import Link from "next/link"
import { useEffect, useRef, useState } from "react"
import { ArrowLeft, Eye, EyeOff, Pause, Play } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
import { Label } from "@/components/ui/label"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useAudioAnalysis } from "./audio-analysis"
import { useMediaFile } from "./media-file"
import { EditorTimeline } from "./editor-timeline"
import { MotionOverlay } from "./motion-overlay"
import { MotionStrip } from "./motion-strip"
import { NowPlayingPanel } from "./now-playing-panel"
import { SafetyPanel } from "./safety-panel"
import { usePlayback } from "./use-playback"
import { VisionPanel, PayloadSection, RawResponseSection } from "./vision-panel"
import { playbackSrc, useHeatmap, useStreamVideo, useVisionAnalysis } from "./vision-analysis"

function EditorNav({ videoId, playing, onToggle, autoApprove, setAutoApprove }: { videoId: string; playing: boolean; onToggle: () => void; autoApprove: boolean; setAutoApprove: (v: boolean) => void }) {
  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b bg-card px-3 md:px-5">
      <div className="flex min-w-0 items-center gap-3">
        <Button variant="ghost" size="icon" nativeButton={false} render={<Link href="/videos" />} aria-label="Videolara dön">
          <ArrowLeft />
        </Button>
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{videoId}</p>
          <p className="hidden text-[11px] text-muted-foreground sm:block">Görsel ve işitsel analiz</p>
        </div>
      </div>
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <Switch id="auto-approve" checked={autoApprove} onCheckedChange={setAutoApprove} />
          <Label htmlFor="auto-approve" className="text-xs font-medium">Otomatik Onayla</Label>
        </div>
        <Button variant="outline" onClick={onToggle}>
          {playing ? <Pause data-icon="inline-start" /> : <Play data-icon="inline-start" />}
          {playing ? "Duraklat" : "Oynat"}
        </Button>
      </div>
    </header>
  )
}

/**
 * Sunucudaki profillerle birebir aynı adlar (config.rs::PROFILES).
 *
 * Maliyet farkı büyük ve doğrusal değil: pencere kısaldıkça model çağrısı sayısı
 * artıyor ve her çağrının sabit maliyeti baskın hâle geliyor. 1 saat 40 dakikalık
 * bir kayıtta hassas ~24.000 pencere, geniş ~1.200 pencere demek (ölçüldü:
 * hassas 324 sn). İpucu metinleri bunu söylüyor ki kullanıcı sürprizle
 * karşılaşmasın.
 */
const PROFILES = [
  { id: "hassas", label: "Hassas", hint: "±0,25 sn — kısa ve ani sesler; en pahalı, uzun kayıtlarda dakikalar sürer" },
  { id: "dengeli", label: "Dengeli", hint: "±0,5 sn — çoğu video için doğru seçim" },
  { id: "isabetli", label: "Geniş", hint: "±2,5 sn — sürekli sesler; uzun kayıtlarda en hızlı" },
]

export function VideoDetailView({ videoId }: { videoId: string }) {
  // Uzantı kimlikten türetilemez — mkv/webm/mov da yüklenebiliyor ve `.mp4`
  // varsaymak o videoları hem oynatıcıda hem analizde kırıyordu. Gerçek dosya
  // adını servis söylüyor.
  const { filename, error: mediaError } = useMediaFile(videoId)

  // Görüntü tarafı ayrı bir serviste; iki depo orijinal dosya adı üzerinden
  // eşleşiyor. Ayrıntısı `vision-analysis.ts` içinde.
  const { video: streamVideo, error: streamError } = useStreamVideo(videoId, filename)
  const streamId = streamVideo?.id ?? null
  const { heatmap, loading: heatmapLoading, error: heatmapError } = useHeatmap(streamId)
  const vision = useVisionAnalysis(streamId, streamVideo?.info.duration_ms ?? 0)
  const [overlay, setOverlay] = useState(true)

  const videoRef = useRef<HTMLVideoElement>(null)
  const { currentTime, duration, playing, seek, toggle, subscribe } = usePlayback(videoRef)
  const [profile, setProfile] = useState("dengeli")

  // Mock Toolbox Alert state'i
  const [toolAlerts, setToolAlerts] = useState<{id: number; title: string; message: string}[]>([])
  const [autoApprove, setAutoApprove] = useState(false)

  useEffect(() => {
    // Gateway üzerinden ToolAlerts'leri (ve diğer olayları) SSE ile dinleyelim
    // streamId henüz null olabilir, videoId ile bağlanıyoruz (çünkü gateway doğrudan o string üzerinden yönlendiriyor)
    const sse = new EventSource(`http://localhost:8000/api/videos/${videoId}/events`)
    
    sse.addEventListener("report", (event) => {
      try {
        const data = JSON.parse(event.data)
        vision.setOutcome({ report: data, steps: [] })
        vision.setRunning(false)
      } catch (e) {
        console.error("Report parse hatası", e)
      }
    })

    sse.addEventListener("alert", (event) => {
      try {
        const data = JSON.parse(event.data)
        const newAlert = {
          id: Date.now() + Math.random(),
          title: data.title || "Dış Sistem Uyarıldı",
          message: data.message || "Bilinmeyen bir işlem gerçekleştirildi."
        }
        
        setToolAlerts(prev => [...prev, newAlert])
        
        // 5 saniye sonra alert'i ekrandan kaldır
        setTimeout(() => {
          setToolAlerts(prev => prev.filter(a => a.id !== newAlert.id))
        }, 5000)
      } catch(e) {
        console.error("SSE parse hatasi:", e)
      }
    })

    return () => {
      sse.close()
    }
  }, [videoId])

  /**
   * İki ayrı eşik var ve bu bilinçli:
   *
   * - `threshold` kaydırıcının anlık değeri; **yalnız çizimi süzer**, anında.
   * - `appliedThreshold` sunucuya gönderilmiş olan; olayları, özeti ve güvenlik
   *   bulgularını üreten değer.
   *
   * Kaydırıcıyı doğrudan sunucuya bağlamak denendi ve uzun kayıtlarda felaketti:
   * 1 saat 40 dakikalık bir videoda her dokunuş 324 saniyelik yeni bir analiz
   * başlatıyordu (ölçüldü) ve model tek oturumu kilitlediği için bu sırada başka
   * hiçbir video analiz edilemiyordu. Artık yeniden çözümleme yalnız kullanıcı
   * açıkça isteyince oluyor.
   */
  const [threshold, setThreshold] = useState(35)
  const [appliedThreshold, setAppliedThreshold] = useState(35)
  const { analysis, nameOf, severityOf, refreshing, ...analysisState } = useAudioAnalysis(
    filename,
    profile,
    appliedThreshold / 100,
  )
  const thresholdDirty = threshold !== appliedThreshold
  // Dosya bulunamadıysa çözümlemenin ne diyeceğini beklemenin anlamı yok:
  // hata zincirin ilk halkasında ve sebebi daha açık.
  const source = mediaError ? "error" : analysisState.source
  const error = mediaError ?? analysisState.error

  // Boşluk tuşu oynat/duraklat — editörlerin evrensel alışkanlığı
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // VIDEO da dışarıda: oynatıcı odaktayken boşluk/ok tuşlarını tarayıcının
      // kendi kontrolü zaten işliyor, ikisi birden çalışınca hareket iptal oluyordu.
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "BUTTON" || tag === "VIDEO") return
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
    <div className="flex h-screen flex-col overflow-hidden bg-background">
      {/* TOOL ALERTS TOAST CONTAINER */}
      <div className="fixed bottom-6 right-6 z-50 flex flex-col gap-3 pointer-events-none">
        {toolAlerts.map(alert => (
          <div 
            key={alert.id} 
            className="pointer-events-auto flex items-start gap-3 bg-red-950/90 border border-red-500/50 text-red-50 p-4 rounded-xl shadow-2xl backdrop-blur-sm animate-in slide-in-from-right fade-in duration-300"
          >
            <div className="mt-0.5 text-red-500">
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>
            </div>
            <div>
              <h4 className="font-semibold text-sm">{alert.title}</h4>
              <p className="text-xs text-red-200 mt-1">{alert.message}</p>
            </div>
          </div>
        ))}
      </div>
      <EditorNav videoId={videoId} playing={playing} onToggle={toggle} autoApprove={autoApprove} setAutoApprove={setAutoApprove} />

      <main className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_280px] gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_330px]">
        {/* sol: video üstte, zaman çizelgesi altta */}
        {/* Şeride rahat ama abartısız bir yükseklik: `auto` dibe yapıştırıyor,
            250px ise tek şeritle boş kalıyordu. */}
        <section className="grid min-h-0 min-w-0 grid-rows-[minmax(0,1fr)_auto_175px] gap-3">
          <div className="relative flex min-h-0 items-center justify-center rounded-xl border bg-card p-3">
            {/* Isı haritası videoyla aynı kutuya hizalanmalı; sarmalayıcı
                `<video>` ile birebir aynı boyutta olsun diye `inline-flex`. */}
            <div className="relative inline-flex max-h-full max-w-full">
              <video
                ref={videoRef}
                src={playbackSrc(filename, streamId)}
                className="max-h-full max-w-full rounded-lg bg-muted"
                onClick={toggle}
                controls
                playsInline
              />
              {overlay && <MotionOverlay heatmap={heatmap} subscribe={subscribe} />}
            </div>

            {heatmap && (
              <Button
                variant="outline"
                size="xs"
                className="absolute right-4 top-4 bg-card/90 backdrop-blur"
                onClick={() => setOverlay((o) => !o)}
                aria-pressed={overlay}
                title="Hareket yoğunluğunu video üzerinde gösterir"
              >
                {overlay ? <EyeOff data-icon="inline-start" /> : <Eye data-icon="inline-start" />}
                Isı haritası
              </Button>
            )}
          </div>

          <MotionStrip
            heatmap={heatmap}
            duration={duration}
            events={vision.outcome?.report.events ?? []}
            analysedRange={
              vision.outcome?.steps.at(-1)
                ? {
                    t0_ms: vision.outcome.steps.at(-1)!.t0_ms,
                    t1_ms: vision.outcome.steps.at(-1)!.t1_ms,
                  }
                : null
            }
            onSeek={seek}
            subscribe={subscribe}
            loading={heatmapLoading}
            error={heatmapError ?? streamError}
          />

          <EditorTimeline
            analysis={analysis}
            source={source}
            error={error}
            refreshing={refreshing}
            nameOf={nameOf}
            severityOf={severityOf}
            duration={duration}
            threshold={threshold / 100}
            onSeek={seek}
            subscribe={subscribe}
          />
        </section>

        {/* sağ: görsel analiz ve ses, sekmeli */}
        <Tabs defaultValue="gorsel" className="flex min-h-0 flex-col gap-3">
          <TabsList className="shrink-0">
            <TabsTrigger value="gorsel">Görsel</TabsTrigger>
            <TabsTrigger value="ses">Ses</TabsTrigger>
            <TabsTrigger value="veri">Veri</TabsTrigger>
          </TabsList>

          <TabsContent value="gorsel" keepMounted className="flex min-h-0 flex-1 flex-col data-[hidden]:hidden">
            <VisionPanel
              videoId={videoId}
              outcome={vision.outcome}
              payload={vision.payload}
              prompt={vision.prompt}
              running={vision.running}
              error={vision.error}
              onAnalyze={vision.analyze}
              onLoadPayload={() => vision.loadPayload()}
              onSeek={seek}
              ready={Boolean(streamId)}
              autoApprove={autoApprove}
            />
          </TabsContent>

          <TabsContent value="veri" keepMounted className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto data-[hidden]:hidden">
            <PayloadSection 
              payload={vision.payload} 
              prompt={vision.prompt} 
              onLoad={() => vision.loadPayload()} 
              ready={Boolean(streamId)} 
            />
            <RawResponseSection outcome={vision.outcome} />
          </TabsContent>

          <TabsContent value="ses" keepMounted className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto data-[hidden]:hidden">
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

            <div className="flex flex-col gap-2 border-t pt-3">
              <div className="flex items-center gap-3">
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

              {/* Kaydırıcı şeridi anında süzüyor; olaylar ve bulgular ise analiz
                  anındaki eşiğe ait. Fark varsa bunu saklamak yerine söylüyoruz
                  ve düzeltmeyi kullanıcının kararına bırakıyoruz — yeniden
                  çözümleme uzun kayıtlarda dakikalar sürebiliyor. */}
              {thresholdDirty && analysis && (
                <div className="flex flex-col gap-1.5">
                  <span className="text-[10px] leading-snug text-muted-foreground">
                    Şerit %{threshold} ile süzülüyor; olaylar ve güvenlik bulguları %
                    {appliedThreshold} ile üretildi.
                  </span>
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={() => setAppliedThreshold(threshold)}
                    disabled={refreshing}
                    title="Tüm panelleri bu eşiğe göre yeniden hesaplar; uzun kayıtlarda dakikalar sürebilir."
                  >
                    {refreshing ? "Çözümleniyor…" : `%${threshold} ile yeniden çözümle`}
                  </Button>
                </div>
              )}
            </div>
          </div>
          </TabsContent>
        </Tabs>
      </main>
    </div>
  )
}
