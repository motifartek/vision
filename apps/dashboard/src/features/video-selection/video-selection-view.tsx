"use client"

import Link from "next/link"
import { useState, useMemo } from "react"
import { Clock3, Film, Grid2X2, List, Search, SlidersHorizontal } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"
import { MinimalCard, MinimalCardImage, MinimalCardTitle, MinimalCardDescription, MinimalCardContent } from "@/components/ui/mini-card-cult"
import { flexRender, getCoreRowModel, getSortedRowModel, useReactTable, ColumnDef } from "@tanstack/react-table"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { VideoUploadDialog } from "./video-upload-dialog"

<<<<<<< HEAD
const videos = [
  { id: "podcast-highlight-03", title: "Podcast Highlight 03", status: "İşleniyor", category: "Podcast", duration: "12:45", color: "bg-blue-500/10 text-blue-500", image: "https://images.unsplash.com/photo-1529699211952-734e80c4d42b?auto=format&fit=crop&q=80&w=400&h=300" },
  { id: "urun-tanitimi-bahar", title: "Ürün Tanıtımı - Bahar", status: "Hazır", category: "Reklam", duration: "01:30", color: "bg-emerald-500/10 text-emerald-500", image: "https://images.unsplash.com/photo-1542751371-adc38448a05e?auto=format&fit=crop&q=80&w=400&h=300" },
  { id: "kurucu-roportaji", title: "Kurucu Röportajı", status: "Analiz", category: "Röportaj", duration: "45:00", color: "bg-orange-500/10 text-orange-500", image: "https://images.unsplash.com/photo-1516321318423-f06f85e504b3?auto=format&fit=crop&q=80&w=400&h=300" },
  { id: "webinar-kaydi", title: "Q3 Webinar Kaydı", status: "Bekliyor", category: "Eğitim", duration: "1:15:00", color: "bg-purple-500/10 text-purple-500", image: "https://images.unsplash.com/photo-1515162816999-a0c47dc192f7?auto=format&fit=crop&q=80&w=400&h=300" },
=======
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
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
]

export const columns: ColumnDef<typeof videos[0]>[] = [
  {
    accessorKey: "title",
    header: "Video Adı",
    cell: ({ row }) => {
      const video = row.original
      return (
        <Link href={`/videos/${video.id}`} className="font-medium hover:underline flex items-center gap-2">
          <Film className="size-4 text-muted-foreground" />
          {video.title}
        </Link>
      )
    },
  },
  {
    accessorKey: "category",
    header: "Kategori",
  },
  {
    accessorKey: "duration",
    header: "Süre",
    cell: ({ row }) => (
      <span className="flex items-center gap-1">
        <Clock3 className="size-3 text-muted-foreground"/> {row.getValue("duration")}
      </span>
    ),
  },
  {
    accessorKey: "status",
    header: "Durum",
    cell: ({ row }) => <Badge variant="outline">{row.getValue("status")}</Badge>,
  },
]

export function VideoSelectionView(){
  const [query, setQuery] = useState("")
  const [searchOpen, setSearchOpen] = useState(false)
  const [uploadOpen, setUploadOpen] = useState(false)
    const [viewMode, setViewMode] = useState<"grid" | "list">("grid")
  
  const filtered = useMemo(() => {
    return videos.filter(v => v.title.toLocaleLowerCase("tr").includes(query.toLocaleLowerCase("tr")))
  }, [query])

<<<<<<< HEAD
  const table = useReactTable({
    data: filtered,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    autoResetPageIndex: false,
  })

  return (
    <>
      <div className="flex flex-col gap-6 w-full">
      <div className="flex flex-col justify-between gap-4 md:flex-row md:items-end">
        <div>
          <h2 className="text-balance text-2xl font-semibold">Video kütüphanesi</h2>
          <p className="mt-1 text-sm text-muted-foreground">{videos.length} içerik arasından projenizi seçin.</p>
=======
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
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
        </div>
        <Button onClick={() => setUploadOpen(true)}><Film data-icon="inline-start" /> Video yükle</Button>
      </div>
      
      <div className="flex items-center justify-between gap-3 rounded-xl border bg-card p-2 sm:p-3">
        {/* Expanding Search Bar */}
        <div className={cn("relative transition-all duration-300 ease-in-out overflow-hidden flex items-center", searchOpen ? "w-full max-w-sm opacity-100" : "w-10 opacity-70 hover:opacity-100")}>
          {!searchOpen ? (
            <Button variant="ghost" size="icon" onClick={() => setSearchOpen(true)} className="shrink-0 text-muted-foreground">
              <Search className="size-5" />
            </Button>
          ) : (
            <>
              <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
              <Input 
                value={query} 
                onChange={e => setQuery(e.target.value)} 
                className="pl-9 w-full" 
                placeholder="Video ara..." 
                autoFocus
                onBlur={(e) => { if (!e.target.value) setSearchOpen(false) }}
              />
            </>
          )}
        </div>
        
        <div className="flex items-center gap-1 sm:gap-2 shrink-0">
          <Button variant="outline" className="hidden sm:flex"><SlidersHorizontal data-icon="inline-start"/> Filtrele</Button>
          <Button variant="outline" size="icon" className="sm:hidden"><SlidersHorizontal /></Button>
          <div className="h-4 w-px bg-border mx-1" />
          <Button variant={viewMode === "grid" ? "secondary" : "ghost"} size="icon" onClick={() => setViewMode("grid")} aria-label="Izgara görünümü"><Grid2X2/></Button>
          <Button variant={viewMode === "list" ? "secondary" : "ghost"} size="icon" onClick={() => setViewMode("list")} aria-label="Liste görünümü"><List/></Button>
        </div>
      </div>
      
      {viewMode === "grid" ? (
        <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {filtered.map(video => (
            <Link key={video.id} href={`/videos/${video.id}`} className="block w-full">
              <MinimalCard className="h-full flex flex-col cursor-pointer">
                <MinimalCardImage src={video.image} alt={video.title} />
                <MinimalCardTitle className="truncate">{video.title}</MinimalCardTitle>
                <MinimalCardDescription className="flex items-center gap-1 mt-1">
                  <Clock3 className="size-3" /> {video.duration} &bull; {video.category}
                </MinimalCardDescription>
              </MinimalCard>
            </Link>
          ))}
          {filtered.length === 0 && (
            <div className="col-span-full py-12 text-center text-muted-foreground">Sonuç bulunamadı.</div>
          )}
        </div>
      ) : (
        <div className="rounded-md border bg-card w-full overflow-hidden">
          <Table>
            <TableHeader className="bg-muted/50">
              {table.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <TableHead key={header.id}>
                      {header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {table.getRowModel().rows?.length ? (
                table.getRowModel().rows.map((row) => (
                  <TableRow key={row.id} data-state={row.getIsSelected() && "selected"}>
                    {row.getVisibleCells().map((cell) => (
                      <TableCell key={cell.id} className="py-3">
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </TableCell>
                    ))}
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell colSpan={columns.length} className="h-24 text-center text-muted-foreground">
                    Sonuç bulunamadı.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
      <VideoUploadDialog open={uploadOpen} onClose={() => setUploadOpen(false)} />
    </>
  )
}
