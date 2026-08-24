"use client"

import Link from "next/link"
import { useState } from "react"
import { ArrowLeft, Captions, Check, ChevronDown, ChevronLeft, ChevronRight, CirclePlay, Download, Ellipsis, Film, ImageIcon, Music2, Pause, Play, Redo2, Scissors, Send, Sparkles, Undo2, Upload, Volume2, WandSparkles, ShieldAlert, X } from "lucide-react"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs"
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

function EditorNav() {
  return (
    <div className="absolute top-0 inset-x-0 z-50 group pointer-events-none">
      {/* Invisible hover trigger area */}
      <div className="h-4 w-full absolute top-0 pointer-events-auto"></div>
      
      <header className="flex h-14 items-center justify-between border-b bg-card px-3 md:px-5 transform -translate-y-full group-hover:translate-y-0 transition-transform duration-300 shadow-md pointer-events-auto">
        <div className="flex min-w-0 items-center gap-3">
          <Button variant="ghost" size="icon" render={<Link href="/videos" />} nativeButton={false} aria-label="Akışlara dön">
            <ArrowLeft/>
          </Button>
          <div className="min-w-0">
            <p className="truncate text-sm font-medium">Kamera-04: Depo Yükleme Alanı</p>
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

function ControlPanel() {
  return (
    <Card className="min-h-0 flex py-0! flex-col">
      <CardHeader className="p-0! shrink-0">
        <div className="flex justify-between items-center">
          <CardTitle className="text-sm font-semibold sr-only">Akış Analizi</CardTitle>
        </div>
      </CardHeader>

      <CardContent className="flex-1 overflow-hidden p-0 relative">
        <div className="h-full w-full overflow-auto p-3 flex flex-col">
          {/* Timeline Inner Content width forces horizontal scroll */}
          <div className="min-w-[800px] flex flex-col flex-1 relative">
            
            <div className="flex items-center justify-between text-[10px] text-muted-foreground border-b pb-1 mb-2 px-8 relative select-none">
              <span>08:15:00</span>
              <span>08:15:15</span>
              <span>08:15:30</span>
              <span>08:15:45</span>
              <span>08:16:00</span>
              <span>Canlı</span>
            </div>
            
            <div className="relative flex-1 bg-muted/30 rounded-md p-2 flex flex-col gap-2">
              {/* Playhead */}
              <div className="absolute top-0 bottom-0 w-[1px] bg-red-500 left-[65%] z-10 pointer-events-none">
                <div className="w-2.5 h-2.5 bg-red-500 absolute -top-1 -left-[4px] rounded-sm"></div>
              </div>

              {/* Görüntü Kanalı (Stream) */}
              <div className="flex items-center gap-2 group">
                <div className="w-6 shrink-0 text-[10px] font-bold text-muted-foreground text-center group-hover:text-foreground">STR</div>
                <div className="flex-1 relative h-10 bg-background/50 rounded overflow-hidden border flex items-center">
                  <div className="absolute left-0 w-[25%] h-full bg-slate-500/20 border-r border-slate-500/40 flex items-center px-2 text-[10px] truncate">Seg_01</div>
                  <div className="absolute left-[25%] w-[25%] h-full bg-slate-500/20 border-r border-slate-500/40 flex items-center px-2 text-[10px] truncate">Seg_02</div>
                  <div className="absolute left-[50%] w-[25%] h-full bg-slate-500/20 border-r border-slate-500/40 flex items-center px-2 text-[10px] truncate">Seg_03</div>
                </div>
              </div>

              {/* AI Analiz Kanalı (AI) */}
              <div className="flex items-center gap-2 group">
                <div className="w-6 shrink-0 text-[10px] font-bold text-muted-foreground text-center group-hover:text-foreground">AI</div>
                <div className="flex-1 relative h-10 bg-background/50 rounded overflow-hidden border flex items-center">
                  <div className="absolute left-0 w-[50%] h-full bg-green-500/20 border-r border-green-500/40 flex items-center px-2 text-[10px] text-green-600 font-medium">Güvenli</div>
                  <div className="absolute left-[50%] w-[15%] h-full bg-red-500/30 border-r border-red-500/50 flex items-center px-2 text-[10px] text-red-600 font-bold animate-pulse">İhlal!</div>
                  <div className="absolute left-[65%] w-[10%] h-full bg-yellow-500/20 flex items-center px-2 text-[10px] text-yellow-600">Analiz...</div>
                </div>
              </div>

              {/* Aksiyon Kanalı (ACT) */}
              <div className="flex items-center gap-2 group">
                <div className="w-6 shrink-0 text-[10px] font-bold text-muted-foreground text-center group-hover:text-foreground">ACT</div>
                <div className="flex-1 relative h-8 rounded overflow-hidden flex items-center">
                  <div className="absolute left-[58%] text-[10px] bg-red-500 text-white px-1.5 py-0.5 rounded shadow">Agent: Siren Tetiklendi</div>
                </div>
              </div>
              
              {/* Extensible space at bottom if more tracks needed */}
              <div className="h-4"></div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
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
        {/* Sekmeler Header */}
        <div className="p-2 border-b shrink-0">
          <TabsList className="w-full grid grid-cols-3 h-8">
            <TabsTrigger value="agent" className="text-xs">Ajan</TabsTrigger>
            <TabsTrigger value="context" className="text-xs">Bağlam</TabsTrigger>
            <TabsTrigger value="events" className="text-xs">Olaylar</TabsTrigger>
          </TabsList>
        </div>
        
        {/* Sekme İçerikleri */}
        <CardContent className="flex-1 overflow-auto p-0 border-b">
          {/* AJAN (DÜŞÜNCE) SEKME İÇERİĞİ */}
          <TabsContent value="agent" className="m-0 h-full p-3 flex flex-col">
            <div className="text-[11px] font-mono text-muted-foreground flex flex-col gap-1">
              <div className="text-foreground flex items-center gap-2 mb-1 font-sans font-medium text-xs">
                <Sparkles className="size-3 text-primary"/> Agent Düşünce Akışı
              </div>
              <div className="pl-3 border-l border-muted py-1 flex flex-col gap-1.5">
                <p className="text-green-600 dark:text-green-400">{`> [08:15:21] Yeni Segment alındı.`}</p>
                <p>{`> OHS-Vision YOLOv8 ile kareler tarandı.`}</p>
                <p>{`> Tespit: Forklift (Güven: %98)`}</p>
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

          {/* BAĞLAM SEKME İÇERİĞİ */}
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

          {/* OLAYLAR SEKME İÇERİĞİ */}
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

      {/* HITL Carousel Alanı - En alta taşındı */}
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
              <Button size="icon" variant="outline" className="h-6 w-6 text-green-600 border-green-600/30 hover:bg-green-600/10 rounded-full" aria-label="Onayla" onClick={(e) => { e.stopPropagation(); /* handle approve */ }}>
                <Check className="size-3"/>
              </Button>
              <Button size="icon" variant="outline" className="h-6 w-6 text-red-600 border-red-600/30 hover:bg-red-600/10 rounded-full" aria-label="Reddet" onClick={(e) => { e.stopPropagation(); /* handle reject */ }}>
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

export function VideoDetailView() {
  return (
    <div className="relative flex h-dvh min-w-[680px] flex-col overflow-hidden bg-background">
      <EditorNav />
      <main className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_260px] gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_320px]">
        <section className="grid min-h-0 grid-rows-[minmax(300px,1.25fr)_minmax(230px,.75fr)] gap-3">
          {/* OYNAYICI (PLAYER) KARTI */}
          <Card className="panel-grid min-h-0 relative">
            <CardContent className="flex min-h-0 flex-1 items-center justify-center p-4">
              <div className="relative h-full w-full max-h-[600px] overflow-hidden rounded-xl border bg-muted group">
                
                <VideoPlayer className="w-full h-full">
                  <VideoPlayerContent
                    crossOrigin=""
                    muted
                    preload="auto"
                    slot="media"
                    src="https://stream.mux.com/DS00Spx1CV902MCtPj5WknGlR102V5HFkDe/high.mp4"
                    className="size-full object-cover"
                    tabIndex={-1}
                    suppressHydrationWarning
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
          
          <ControlPanel />
        </section>
        
        <EnhancePanel />
      </main>
    </div>
  )
}
