"use client"

import Link from "next/link"
import { useEffect, useState } from "react"
import { Clock3, Film, Grid2X2, HardDrive, List, Search, Trash2, X } from "lucide-react"
import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { formatTime } from "@/features/video-detail/time"
import { VideoUploadDialog } from "./video-upload-dialog"

const API = process.env.NEXT_PUBLIC_AUDIO_API ?? "/api/inference"
const STREAM = process.env.NEXT_PUBLIC_STREAM_API ?? "/api/stream"

type VideoEntry = {
  id: string
  filename: string
  size: number
  /** Kapsayıcı başlığından okundu; okunamadıysa null. */
  duration_sec: number | null
}

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/** Dosya adından güzel bir başlık üret: tire/alt çizgi → boşluk, baş harf büyük. */
function prettifyName(id: string) {
  return id
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toLocaleUpperCase("tr"))
}

const CARD_COLORS = [
  "bg-primary/20",
  "bg-success/15",
  "bg-media/15",
  "bg-muted",
  "bg-primary/10",
  "bg-success/10",
]

/**
 * İki adımlı silme: ilk tıklama sorar, ikincisi siler.
 *
 * Modal kurmaya değmez ama tek tıklamayla kalıcı silme de olmaz — dosya diskten
 * gidiyor, geri alınamıyor.
 */
function DeleteControl({
  video,
  armed,
  busy,
  onArm,
  onCancel,
  onConfirm,
}: {
  video: VideoEntry
  armed: boolean
  busy: boolean
  onArm: () => void
  onCancel: () => void
  onConfirm: () => void
}) {
  if (busy) return <span className="text-[11px] text-muted-foreground">siliniyor…</span>

  if (armed) {
    return (
      <span className="flex items-center gap-1">
        <Button size="xs" variant="destructive" onClick={onConfirm}>
          Sil
        </Button>
        <Button size="icon-xs" variant="ghost" onClick={onCancel} aria-label="Vazgeç">
          <X />
        </Button>
      </span>
    )
  }

  return (
    <Button
      size="icon-xs"
      variant="ghost"
      onClick={onArm}
      aria-label={`${video.filename} dosyasını sil`}
      title="Videoyu diskten sil"
    >
      <Trash2 />
    </Button>
  )
}

export function VideoSelectionView() {
  const [query, setQuery] = useState("")
  const [videos, setVideos] = useState<VideoEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [uploadOpen, setUploadOpen] = useState(false)
  const [view, setView] = useState<"grid" | "list">("grid")
  /** Silme iki adımlı: ilk tıklama sorar, ikincisi siler. Modal kurmaya değmez. */
  const [confirmId, setConfirmId] = useState<string | null>(null)
  const [busyId, setBusyId] = useState<string | null>(null)

  /**
   * Listeyi görüntü servisinden kurar.
   *
   * Kaynak `stream`, çünkü görsel analiz ürünün kendisi ve videoların
   * yüklendiği yer orası. Ses servisi ayrıca sorgulanıyor ama **isteğe bağlı**:
   * ayakta değilken liste boş kalıyordu ve panelden video eklemek de,
   * eklenmiş videoyu görmek de imkânsız hâle geliyordu.
   */
  const loadVideos = () => {
    setLoading(true)

    const stream = fetch(`${STREAM}/v1/videos`)
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`)
        return r.json() as Promise<{
          videos: { id: string; original_name: string; info: { size_bytes: number; duration_ms: number } }[]
        }>
      })
      .then(({ videos }) =>
        videos.map<VideoEntry>((v) => ({
          id: v.id,
          filename: v.original_name,
          size: v.info.size_bytes,
          duration_sec: v.info.duration_ms ? v.info.duration_ms / 1000 : null,
        })),
      )

    stream
      .then((data) => {
        setVideos(data)
        setError(null)
      })
      .catch((err: Error) => {
        setError(err.message)
        setVideos([])
      })
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    loadVideos()
  }, [])

  const removeVideo = async (id: string) => {
    setBusyId(id)
    try {
      const r = await fetch(`${STREAM}/v1/videos/${encodeURIComponent(id)}`, { method: "DELETE" })
      if (!r.ok && r.status !== 404) throw new Error(`HTTP ${r.status}`)

      // Ses tarafındaki kopya kendi kimliğiyle duruyor ve ad üzerinden
      // eşleşiyor; servis kapalıysa silme başarısız sayılmıyor.
      const ad = videos.find((v) => v.id === id)?.filename
      if (ad) {
        await fetch(`${API}/v1/videos/${encodeURIComponent(ad)}`, { method: "DELETE" }).catch(
          () => undefined,
        )
      }

      setVideos((list) => list.filter((v) => v.id !== id))
    } catch (err) {
      setError(err instanceof Error ? err.message : "silinemedi")
    } finally {
      setBusyId(null)
      setConfirmId(null)
    }
  }

  // Upload dialog kapandığında listeyi yenile
  const handleUploadClose = () => {
    setUploadOpen(false)
    loadVideos()
  }

  const filtered = videos.filter((v) =>
    prettifyName(v.id).toLocaleLowerCase("tr").includes(query.toLocaleLowerCase("tr")) ||
    v.filename.toLocaleLowerCase("tr").includes(query.toLocaleLowerCase("tr"))
  )

  return (
    <AppShell>
      <div className="mx-auto flex max-w-7xl flex-col gap-6">
        <div className="flex flex-col justify-between gap-4 md:flex-row md:items-end">
          <div>
            <h2 className="text-balance text-2xl font-semibold">Video kütüphanesi</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {loading
                ? "Yükleniyor…"
                : error
                  ? "Inference servisi bağlantısı kurulamadı"
                  : `${videos.length} video`}
            </p>
          </div>
          <Button onClick={() => setUploadOpen(true)}>
            <Film data-icon="inline-start" /> Video yükle
          </Button>
        </div>

        <div className="flex flex-col justify-between gap-3 rounded-xl border bg-card p-3 sm:flex-row">
          <div className="relative flex-1 sm:max-w-sm">
            <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="pl-9"
              placeholder="Video ara..."
            />
          </div>
          {/* "Filtrele" düğmesi buradaydı ve hiçbir şey yapmıyordu; süzme işini
              zaten soldaki arama kutusu görüyor. Görünüm düğmeleri artık gerçekten
              görünümü değiştiriyor. */}
          <div className="flex gap-2">
            <Button
              variant={view === "grid" ? "secondary" : "ghost"}
              size="icon"
              aria-label="Izgara görünümü"
              aria-pressed={view === "grid"}
              onClick={() => setView("grid")}
            >
              <Grid2X2 />
            </Button>
            <Button
              variant={view === "list" ? "secondary" : "ghost"}
              size="icon"
              aria-label="Liste görünümü"
              aria-pressed={view === "list"}
              onClick={() => setView("list")}
            >
              <List />
            </Button>
          </div>
        </div>

        {loading ? (
          /* İskelet */
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <Card key={i} className="animate-pulse">
                <CardContent className="pt-0">
                  <div className="flex aspect-video items-center justify-center rounded-lg border bg-muted" />
                </CardContent>
                <CardHeader>
                  <div className="h-5 w-3/4 rounded bg-muted" />
                </CardHeader>
                <CardFooter>
                  <div className="h-3 w-1/2 rounded bg-muted" />
                </CardFooter>
              </Card>
            ))}
          </div>
        ) : error ? (
          <div className="flex flex-col items-center gap-3 rounded-xl border bg-card p-12 text-center">
            <HardDrive className="size-10 text-muted-foreground" />
            <p className="text-sm font-medium">Inference servisi bağlantısı kurulamadı</p>
            <p className="text-xs text-muted-foreground">
              Servisin çalıştığından emin olun. Hata: {error}
            </p>
            <Button variant="outline" size="sm" onClick={loadVideos}>
              Tekrar dene
            </Button>
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex flex-col items-center gap-3 rounded-xl border bg-card p-12 text-center">
            <Film className="size-10 text-muted-foreground" />
            <p className="text-sm font-medium">
              {videos.length === 0 ? "Henüz video yüklenmemiş" : "Aramanızla eşleşen video yok"}
            </p>
            <p className="text-xs text-muted-foreground">
              {videos.length === 0
                ? "Başlamak için yukarıdaki \"Video yükle\" butonunu kullanın."
                : "Farklı bir arama terimi deneyin."}
            </p>
            {videos.length === 0 && (
              <Button size="sm" onClick={() => setUploadOpen(true)}>
                <Film data-icon="inline-start" /> Video yükle
              </Button>
            )}
          </div>
        ) : (
          <div className={view === "grid" ? "grid gap-4 sm:grid-cols-2 xl:grid-cols-3" : "flex flex-col gap-2"}>
            {filtered.map((video, i) =>
              view === "grid" ? (
                <Card key={video.id} className="group transition-colors hover:bg-accent/30">
                  <CardContent className="pt-0">
                    <Link
                      href={`/videos/${encodeURIComponent(video.id)}`}
                      className={`panel-grid flex aspect-video items-center justify-center rounded-lg border ${CARD_COLORS[i % CARD_COLORS.length]}`}
                    >
                      <div className="flex size-14 items-center justify-center rounded-full border bg-background/80 transition-transform group-hover:scale-105">
                        <Film className="size-6" />
                      </div>
                    </Link>
                  </CardContent>
                  <CardHeader>
                    <div className="flex items-center justify-between gap-3">
                      <CardTitle className="truncate">{prettifyName(video.id)}</CardTitle>
                      <Badge variant="outline">{formatSize(video.size)}</Badge>
                    </div>
                  </CardHeader>
                  {/* Saat ikonunun yanında eskiden yine boyut yazıyordu; süre
                      artık gerçekten süre (servis kapsayıcı başlığından okuyor). */}
                  <CardFooter className="justify-between gap-2 text-xs text-muted-foreground">
                    <span className="truncate">{video.filename}</span>
                    <span className="flex shrink-0 items-center gap-2">
                      <span className="flex items-center gap-1 tabular-nums">
                        <Clock3 className="size-3" />
                        {video.duration_sec === null ? "—" : formatTime(video.duration_sec)}
                      </span>
                      <DeleteControl
                        video={video}
                        armed={confirmId === video.id}
                        busy={busyId === video.id}
                        onArm={() => setConfirmId(video.id)}
                        onCancel={() => setConfirmId(null)}
                        onConfirm={() => removeVideo(video.id)}
                      />
                    </span>
                  </CardFooter>
                </Card>
              ) : (
                <div
                  key={video.id}
                  className="flex items-center gap-3 rounded-lg border bg-card px-3 py-2 transition-colors hover:bg-accent/30"
                >
                  <Link href={`/videos/${encodeURIComponent(video.id)}`} className="flex min-w-0 flex-1 items-center gap-3">
                    <span className={`flex size-9 shrink-0 items-center justify-center rounded-md border ${CARD_COLORS[i % CARD_COLORS.length]}`}>
                      <Film className="size-4" />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium">{prettifyName(video.id)}</span>
                      <span className="block truncate text-[11px] text-muted-foreground">{video.filename}</span>
                    </span>
                  </Link>
                  <span className="flex shrink-0 items-center gap-3 text-xs text-muted-foreground">
                    <span className="flex items-center gap-1 tabular-nums">
                      <Clock3 className="size-3" />
                      {video.duration_sec === null ? "—" : formatTime(video.duration_sec)}
                    </span>
                    <span className="tabular-nums">{formatSize(video.size)}</span>
                    <DeleteControl
                      video={video}
                      armed={confirmId === video.id}
                      busy={busyId === video.id}
                      onArm={() => setConfirmId(video.id)}
                      onCancel={() => setConfirmId(null)}
                      onConfirm={() => removeVideo(video.id)}
                    />
                  </span>
                </div>
              ),
            )}
          </div>
        )}
      </div>

      <VideoUploadDialog open={uploadOpen} onClose={handleUploadClose} />
    </AppShell>
  )
}
