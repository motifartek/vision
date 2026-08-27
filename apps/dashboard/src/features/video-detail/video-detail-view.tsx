"use client"

import Link from "next/link"
<<<<<<< HEAD
import React, { useEffect, useRef, useState } from "react"
import { ArrowLeft, Captions, Check, ChevronDown, ChevronLeft, ChevronRight, CirclePlay, Download, Ellipsis, Film, ImageIcon, Music2, Pause, Play, Redo2, Scissors, Send, Sparkles, Undo2, Upload, Volume2, WandSparkles, ShieldAlert, X } from "lucide-react"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
=======
import { useEffect, useRef, useState } from "react"
import { ArrowLeft, Eye, EyeOff, Pause, Play } from "lucide-react"
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs"
import { Slider } from "@/components/ui/slider"
<<<<<<< HEAD

import {
  VideoPlayer,
  VideoPlayerContent,
  VideoPlayerControlBar,
  VideoPlayerMuteButton,
  VideoPlayerPlayButton,
  VideoPlayerTimeDisplay,
  VideoPlayerTimeRange,
  VideoPlayerVolumeRange,
} from "@/features/video-detail/video-player"
import { MorphSurface } from "@/components/ui/morph-input"

import { useAudioAnalysis } from "./audio-analysis"
import { useMediaFile } from "./media-file"
import { EditorTimeline } from "./editor-timeline"
=======
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useAudioAnalysis } from "./audio-analysis"
import { useMediaFile } from "./media-file"
import { EditorTimeline } from "./editor-timeline"
import { MotionOverlay } from "./motion-overlay"
import { MotionStrip } from "./motion-strip"
import { NowPlayingPanel } from "./now-playing-panel"
import { SafetyPanel } from "./safety-panel"
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
import { usePlayback } from "./use-playback"
import { VisionPanel } from "./vision-panel"
import { playbackSrc, useHeatmap, useStreamVideo, useVisionAnalysis } from "./vision-analysis"

<<<<<<< HEAD
=======
function EditorNav({ videoId, playing, onToggle }: { videoId: string; playing: boolean; onToggle: () => void }) {
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
      {/* "Geri al / İleri al / Dışa aktar" düğmeleri buradaydı ve hiçbiri bir
          şey yapmıyordu: ortada bir düzenleme modeli yok, dolayısıyla geri
          alınacak bir işlem de yok. Çalışmayan düğme, olmayandan kötü. */}
      <div className="flex items-center gap-2">
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
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
const PROFILES = [
  { id: "hizli", label: "Hızlı", hint: "±0,1 sn — tekil alarmlar ve patlamalar" },
  { id: "dengeli", label: "Dengeli", hint: "±0,5 sn — çoğu video için doğru seçim" },
  { id: "isabetli", label: "Geniş", hint: "±2,5 sn — sürekli sesler; uzun kayıtlarda en hızlı" },
]

function EditorNav({ videoId }: { videoId: string }) {
  return (
    <div className="absolute top-0 inset-x-0 z-50 group pointer-events-none">
      <div className="h-4 w-full absolute top-0 pointer-events-auto"></div>
      
      <header className="flex h-14 items-center justify-between border-b bg-card px-3 md:px-5 transform -translate-y-full group-hover:translate-y-0 transition-transform duration-300 shadow-md pointer-events-auto">
        <div className="flex min-w-0 items-center gap-3">
          <Button variant="ghost" size="icon" render={<Link href="/videos" />} nativeButton={false} aria-label="Akışlara dön">
            <ArrowLeft/>
          </Button>
          <div className="min-w-0">
            <p className="truncate text-sm font-medium">Kamera-04: Depo Yükleme Alanı ({videoId})</p>
            <p className="hidden text-[11px] text-muted-foreground sm:block">Durum: Aktif İzleme | Model: OHS-Vision-v2</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" className="hidden sm:flex">İzlemeyi Durdur</Button>
          <Button size="sm"><Download data-icon="inline-start" className="h-4 w-4 mr-1"/> Raporla</Button>
        </div>
      </header>
    </div>
  )
}

function EnhancePanel() {
  const [currentHitlIndex, setCurrentHitlIndex] = useState(0);

  const pendingHitls = [
    {
      id: 1,
      title: "Baret İhlali & Çarpışma",
      tool: "AmbulanceAPI",
      description: "Yapay zeka, yükleme alanında baretsiz bir personel ve hızla yaklaşan bir forklift tespit etti. Sistem 1.2 saniye içinde çarpışma olacağını hesaplıyor. Alanın acil durum moduna alınıp sirenlerin tetiklenmesi ve en yakın Ambulans biriminin aranması öneriliyor.",
    },
    {
      id: 2,
      title: "Yetkisiz Giriş Tespiti",
      tool: "SecurityAPI",
      description: "Sistem, sunucu odasına mesai saatleri dışında yetkisiz bir personelin girdiğini tespit etti. Kapıların kilitlenmesi ve güvenlik birimlerinin uyarılması öneriliyor.",
    },
    {
      id: 3,
      title: "Yangın Tehlikesi",
      tool: "FireAlarmAPI",
      description: "Depo bölümünde termal kamera normalin üzerinde ısı algıladı. Otomatik yangın söndürme sisteminin devreye sokulması ve personelin tahliyesi öneriliyor.",
    }
  ];

  const handleNextHitl = () => setCurrentHitlIndex(prev => (prev + 1) % pendingHitls.length);
  const handlePrevHitl = () => setCurrentHitlIndex(prev => (prev - 1 + pendingHitls.length) % pendingHitls.length);
  
  const currentHitl = pendingHitls[currentHitlIndex];

  return (
    <Card className="min-h-0 flex flex-col">
      <Tabs defaultValue="agent" className="flex flex-col h-full overflow-hidden">
        <div className="p-2 border-b shrink-0">
          <TabsList className="w-full grid grid-cols-3 h-8">
            <TabsTrigger value="agent" className="text-[11px] h-6">Ajan Logu</TabsTrigger>
            <TabsTrigger value="context" className="text-[11px] h-6">Bağlam</TabsTrigger>
            <TabsTrigger value="events" className="text-[11px] h-6">Olaylar</TabsTrigger>
          </TabsList>
        </div>

        <CardContent className="p-0 flex-1 overflow-hidden min-h-0 relative">
          <TabsContent value="agent" className="m-0 h-full overflow-auto p-3">
            <div className="flex gap-2">
              <Avatar className="h-6 w-6 mt-0.5 shrink-0 border">
                <AvatarFallback className="text-[10px] bg-primary text-primary-foreground font-semibold"><Sparkles className="size-3" /></AvatarFallback>
              </Avatar>
              <div className="flex-1 space-y-1.5 text-[11px] font-mono leading-relaxed pb-4">
                <p className="text-muted-foreground">{`> Analiz ediliyor: frame_0192.jpg...`}</p>
                <p className="text-red-500">{`> Tespit: Personel_Baret_Yok (Güven: %94)`}</p>
                <p>{`> Analiz: Personel yaya yolunun dışında, forklift ile mesafe 1.2 metre.`}</p>
                <p>{`> Hesaplama: Hız = 12km/s. Çarpışma rotası kesin.`}</p>
                <p>{`> Risk Skoru = %89 (Eşik %85 aşıldı).`}</p>
                <p className="text-yellow-600 dark:text-yellow-400 font-semibold mt-1">{`> AKSİYON GEREKLİ: AlarmSistemi.Tetikle(), YüklemeAlanı.Kapat()`}</p>
                <p className="text-yellow-600 dark:text-yellow-400 font-semibold">{`> AKSİYON GEREKLİ: AmbulansAPI.Ara()`}</p>
                <p className="text-blue-500">{`> HITL Kuralı: İnsan onayı bekleniyor... (Timeout: 10s)`}</p>
              </div>
            </div>
          </TabsContent>

          <TabsContent value="context" className="m-0 h-full p-3 text-xs text-muted-foreground">
            <p className="font-semibold text-foreground mb-2">Sistem Bağlamı (Context)</p>
            <ul className="list-disc pl-4 flex flex-col gap-1">
              <li><span className="font-medium">Kamera:</span> Depo-04 (Yükleme Rampası)</li>
              <li><span className="font-medium">Aktif Kurallar:</span> Baret zorunlu, Hız sınırı 10km/s, Yaya yolu ihlali yasak</li>
              <li><span className="font-medium">Model:</span> OHS-Vision-v2</li>
              <li><span className="font-medium">Latency:</span> 1.2s</li>
              <li><span className="font-medium">Vardiya:</span> Sabah (08:00 - 16:00)</li>
            </ul>
          </TabsContent>

          <TabsContent value="events" className="m-0 h-full p-3">
            <ul className="flex flex-col gap-2 text-xs">
              <li className="flex flex-col bg-muted/30 p-2 rounded border border-muted">
                <div className="flex justify-between items-center mb-1">
                  <span className="flex items-center gap-1.5 font-medium text-red-500"><ShieldAlert className="size-3"/> Kritik Risk</span>
                  <span className="text-muted-foreground text-[10px]">08:15:23</span>
                </div>
                <p className="text-muted-foreground">Baret ihlali ve çarpışma riski tespit edildi. Onay bekleniyor.</p>
              </li>
              <li className="flex flex-col bg-muted/30 p-2 rounded border border-muted">
                <div className="flex justify-between items-center mb-1">
                  <span className="flex items-center gap-1.5 font-medium text-yellow-600"><Sparkles className="size-3"/> Ajan Testi</span>
                  <span className="text-muted-foreground text-[10px]">08:05:00</span>
                </div>
                <p className="text-muted-foreground">Siren sistemi bağlantı testi başarılı.</p>
              </li>
              <li className="flex flex-col bg-muted/30 p-2 rounded border border-muted">
                <div className="flex justify-between items-center mb-1">
                  <span className="flex items-center gap-1.5 font-medium text-green-600"><Check className="size-3"/> Başlangıç</span>
                  <span className="text-muted-foreground text-[10px]">08:00:00</span>
                </div>
                <p className="text-muted-foreground">Sabah vardiyası kaydı başladı.</p>
              </li>
            </ul>
          </TabsContent>
        </CardContent>
      </Tabs>

      <div className="p-3 shrink-0 bg-muted/10 relative z-20">
        <div className="flex justify-between items-center mb-2">
          <span className="text-[10px] font-semibold text-muted-foreground uppercase flex items-center gap-1">
            <ShieldAlert className="size-3 text-red-500" /> ONAY BEKLEYENLER (HITL)
          </span>
          <div className="flex items-center gap-1">
            <Button size="icon" variant="ghost" className="h-5 w-5" onClick={handlePrevHitl}><ChevronLeft className="size-3"/></Button>
            <span className="text-[10px] text-muted-foreground w-6 text-center">{currentHitlIndex + 1}/{pendingHitls.length}</span>
            <Button size="icon" variant="ghost" className="h-5 w-5" onClick={handleNextHitl}><ChevronRight className="size-3"/></Button>
          </div>
        </div>
        
        <MorphSurface
          key={currentHitl.id}
          collapsedWidth="100%"
          expandedWidth="100%"
          expandedHeight={220}
          collapsedBorderRadius={8}
          triggerLabel={currentHitl.tool}
          triggerClassName="font-semibold text-foreground text-xs"
          renderIndicator={() => (
            <div className="flex gap-1.5 shrink-0">
              <Button size="icon" variant="outline" className="h-6 w-6 text-green-600 border-green-600/30 hover:bg-green-600/10 rounded-full" aria-label="Onayla" onClick={(e) => { e.stopPropagation(); }}>
                <Check className="size-3"/>
              </Button>
              <Button size="icon" variant="outline" className="h-6 w-6 text-red-600 border-red-600/30 hover:bg-red-600/10 rounded-full" aria-label="Reddet" onClick={(e) => { e.stopPropagation(); }}>
                <X className="size-3"/>
              </Button>
            </div>
          )}
          renderContent={({ onClose }) => (
            <div className="flex flex-col h-full w-full">
              <div className="flex justify-between items-center px-4 py-2 ">
                <h4 className="text-sm font-bold flex items-center gap-1.5 ps-3 pb-1">{currentHitl.title}</h4>
                <Button size="icon" variant="ghost" onClick={onClose} className="h-6 w-6 text-muted-foreground pb-1"><X className="size-3"/></Button>
              </div>
              <p className="text-xs text-muted-foreground flex-1 overflow-auto leading-relaxed px-4 pb-2">
                {currentHitl.description}
              </p>
            </div>
          )}
        />
      </div>
    </Card>
  )
}

export function VideoDetailView({ videoId }: { videoId: string }) {
  const { filename, error: mediaError } = useMediaFile(videoId)

  // Görüntü tarafı ayrı bir serviste; iki depo orijinal dosya adı üzerinden
  // eşleşiyor. Ayrıntısı `vision-analysis.ts` içinde.
  const { video: streamVideo, error: streamError } = useStreamVideo(videoId, filename)
  const streamId = streamVideo?.id ?? null
  const { heatmap, loading: heatmapLoading, error: heatmapError } = useHeatmap(streamId)
  const vision = useVisionAnalysis(streamId)
  const [overlay, setOverlay] = useState(true)

  const videoRef = useRef<HTMLVideoElement>(null)
  const { currentTime, duration, playing, seek, toggle, subscribe } = usePlayback(videoRef)
  const [profile, setProfile] = useState("dengeli")
  
  const [threshold, setThreshold] = useState(35)
  const [appliedThreshold, setAppliedThreshold] = useState(35)
  const { analysis, nameOf, severityOf, refreshing, ...analysisState } = useAudioAnalysis(
    filename,
    profile,
    appliedThreshold / 100,
  )
  const thresholdDirty = threshold !== appliedThreshold
  const source = mediaError ? "error" : analysisState.source
  const error = mediaError ?? analysisState.error

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
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
<<<<<<< HEAD
    <div className="relative flex h-dvh min-w-[680px] flex-col overflow-hidden bg-background">
      <EditorNav videoId={videoId} />
      <main className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_260px] gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_320px]">
        <section className="grid min-h-0 grid-rows-[minmax(300px,1.25fr)_minmax(230px,.75fr)] gap-3">
          {/* OYNAYICI (PLAYER) KARTI */}
          <Card className="panel-grid min-h-0 relative">
            <CardContent className="flex min-h-0 flex-1 items-center justify-center p-4">
              <div className="relative h-full w-full max-h-[600px] overflow-hidden rounded-xl border bg-muted group">
                <VideoPlayer className="w-full h-full">
                  <VideoPlayerContent
                    ref={videoRef}
                    crossOrigin=""
                    muted
                    preload="auto"
                    slot="media"
                    src={filename ? `/media/${encodeURIComponent(filename)}` : undefined}
                    className="size-full object-contain"
                    tabIndex={-1}
                    suppressHydrationWarning
                    onClick={toggle}
                  />
                  <VideoPlayerControlBar className="bg-background/85 backdrop-blur border-t border-white/10 m-3 rounded-lg overflow-hidden absolute inset-x-0 bottom-0 opacity-0 transition-opacity duration-300 group-hover:opacity-100">
                    <VideoPlayerPlayButton />
                    <VideoPlayerTimeRange />
                    <VideoPlayerTimeDisplay showDuration />
                    <VideoPlayerMuteButton />
                    <VideoPlayerVolumeRange />
                  </VideoPlayerControlBar>
                </VideoPlayer>
              </div>
            </CardContent>
          </Card>
          
          {/* Timeline - Cizelge (Kullanicinin istedigi eski ControlPanel yerine) */}
=======
    <div className="flex h-dvh min-w-[720px] flex-col overflow-hidden bg-background">
      <EditorNav videoId={videoId} playing={playing} onToggle={toggle} />

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

>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
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
<<<<<<< HEAD
        
        <EnhancePanel />
=======

        {/* sağ: görsel analiz ve ses, sekmeli */}
        <Tabs defaultValue="gorsel" className="flex min-h-0 flex-col gap-3">
          <TabsList className="shrink-0">
            <TabsTrigger value="gorsel">Görsel</TabsTrigger>
            <TabsTrigger value="ses">Ses</TabsTrigger>
          </TabsList>

          <TabsContent value="gorsel" className="flex min-h-0 flex-1 flex-col">
            <VisionPanel
              outcome={vision.outcome}
              payload={vision.payload}
              running={vision.running}
              error={vision.error}
              onAnalyze={vision.analyze}
              onLoadPayload={() => vision.loadPayload()}
              onSeek={seek}
              ready={Boolean(streamId)}
            />
          </TabsContent>

          <TabsContent value="ses" className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
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
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
      </main>
    </div>
  )
}