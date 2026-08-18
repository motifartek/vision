"use client"

import Link from "next/link"
import { useEffect, useState } from "react"
import { Clock3, Film, Grid2X2, HardDrive, List, Search, SlidersHorizontal } from "lucide-react"
import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { VideoUploadDialog } from "./video-upload-dialog"

const API = process.env.NEXT_PUBLIC_AUDIO_API ?? "http://127.0.0.1:8081"

type VideoEntry = {
  id: string
  filename: string
  size: number
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

export function VideoSelectionView() {
  const [query, setQuery] = useState("")
  const [videos, setVideos] = useState<VideoEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [uploadOpen, setUploadOpen] = useState(false)

  const loadVideos = () => {
    setLoading(true)
    fetch(`${API}/v1/videos`)
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`)
        return r.json() as Promise<VideoEntry[]>
      })
      .then((data) => {
        setVideos(data)
        setError(null)
      })
      .catch((err) => {
        setError(err.message)
        setVideos([])
      })
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    loadVideos()
  }, [])

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
    <AppShell title="Video Seçimi" description="Düzenlemek istediğiniz içeriği seçin">
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
          <div className="flex gap-2">
            <Button variant="outline">
              <SlidersHorizontal data-icon="inline-start" /> Filtrele
            </Button>
            <Button variant="secondary" size="icon" aria-label="Izgara görünümü">
              <Grid2X2 />
            </Button>
            <Button variant="ghost" size="icon" aria-label="Liste görünümü">
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
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {filtered.map((video, i) => (
              <Card key={video.id} className="group transition-colors hover:bg-accent/30">
                <CardContent className="pt-0">
                  <Link
                    href={`/videos/${video.id}`}
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
                <CardFooter className="justify-between text-xs text-muted-foreground">
                  <span>{video.filename}</span>
                  <span className="flex items-center gap-1">
                    <Clock3 className="size-3" />
                    {formatSize(video.size)}
                  </span>
                </CardFooter>
              </Card>
            ))}
          </div>
        )}
      </div>

      <VideoUploadDialog open={uploadOpen} onClose={handleUploadClose} />
    </AppShell>
  )
}
