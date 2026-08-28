"use client"

import { useCallback, useEffect, useState, useMemo } from "react"
import Link from "next/link"
import { Search, Grid2X2, List, Film, Clock3, HardDrive, Trash2, ArrowUpDown } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import {
  MinimalCard,
  MinimalCardImage,
  MinimalCardTitle,
  MinimalCardDescription,
} from "@/components/ui/mini-card-cult"
import { VideoUploadDialog } from "./video-upload-dialog"

import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table"

export type VideoRecord = {
  id: string
  filename: string
  size: number
  duration_sec: number | null
}

const SONIC = process.env.NEXT_PUBLIC_SONIC_API ?? "/api/sonic"
const STREAM = process.env.NEXT_PUBLIC_STREAM_API ?? "/api/stream"

const CARD_COLORS = [
  "bg-blue-500/10 text-blue-500 border-blue-500/20",
  "bg-purple-500/10 text-purple-500 border-purple-500/20",
  "bg-orange-500/10 text-orange-500 border-orange-500/20",
  "bg-green-500/10 text-green-500 border-green-500/20",
  "bg-pink-500/10 text-pink-500 border-pink-500/20",
  "bg-yellow-500/10 text-yellow-500 border-yellow-500/20",
]

function formatSize(bytes: number) {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function formatTime(seconds: number) {
  const m = Math.floor(seconds / 60)
  const s = Math.floor(seconds % 60)
  return `${m}:${s.toString().padStart(2, "0")}`
}

function prettifyName(id: string) {
  const parts = id.split("-")
  if (parts.length > 2) return `${parts[0]}-${parts[1]}`
  return id
}

function DeleteControl({
  video,
  armed,
  busy,
  onArm,
  onCancel,
  onConfirm,
}: {
  video: VideoRecord
  armed: boolean
  busy: boolean
  onArm: () => void
  onCancel: () => void
  onConfirm: () => void
}) {
  if (busy) {
    return (
      <Button variant="ghost" size="icon" disabled className="size-7">
        <div className="size-3 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </Button>
    )
  }

  if (armed) {
    return (
      <div className="flex items-center gap-1">
        <Button variant="destructive" size="sm" className="h-7 px-2 text-[10px]" onClick={onConfirm}>
          Emin misin?
        </Button>
        <Button variant="ghost" size="sm" className="h-7 px-2 text-[10px]" onClick={onCancel}>
          İptal
        </Button>
      </div>
    )
  }

  return (
    <Button
      variant="ghost"
      size="icon"
      className="size-7 text-muted-foreground hover:text-destructive"
      onClick={onArm}
      aria-label="Sil"
    >
      <Trash2 className="size-3" />
    </Button>
  )
}

export function VideoSelectionView() {
  const [videos, setVideos] = useState<VideoRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState("")
  const [view, setView] = useState<"grid" | "list">("grid")
  const [uploadOpen, setUploadOpen] = useState(false)
  const [confirmId, setConfirmId] = useState<string | null>(null)
  const [busyId, setBusyId] = useState<string | null>(null)

  const loadVideos = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const r = await fetch(`${STREAM}/v1/videos`)
      if (!r.ok) throw new Error(`${r.status}`)
      const data = await r.json()
      // Sort newest first normally
      setVideos((data.videos || []).reverse())
    } catch (err) {
      setError(err instanceof Error ? err.message : "Bilinmeyen hata")
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    loadVideos()
  }, [loadVideos])

  const filtered = useMemo(() => {
    if (!query.trim()) return videos
    const lower = query.toLowerCase()
    return videos.filter(
      (v) => v.id.toLowerCase().includes(lower) || v.filename.toLowerCase().includes(lower),
    )
  }, [videos, query])

  const removeVideo = async (id: string) => {
    setBusyId(id)
    try {
      const r = await fetch(`${STREAM}/v1/videos/${id}`, { method: "DELETE" })
      if (!r.ok) throw new Error("Silinemedi")
      setVideos((prev) => prev.filter((v) => v.id !== id))
      setConfirmId(null)
    } catch (err) {
      alert("Hata: " + (err instanceof Error ? err.message : String(err)))
    } finally {
      setBusyId(null)
    }
  }

  const handleUploadClose = (success?: boolean) => {
    setUploadOpen(false)
    if (success) loadVideos()
  }

  const columns = useMemo(
    () => [
      {
        accessorKey: "filename",
        header: "Dosya Adı",
        cell: ({ row }: any) => {
          const video = row.original
          return (
            <Link href={`/videos/${encodeURIComponent(video.id)}`} className="flex items-center gap-3">
              <span className="flex size-8 shrink-0 items-center justify-center rounded-md border bg-muted">
                <Film className="size-4" />
              </span>
              <div className="flex flex-col">
                <span className="truncate text-sm font-medium">{prettifyName(video.id)}</span>
                <span className="truncate text-xs text-muted-foreground">{video.filename}</span>
              </div>
            </Link>
          )
        },
      },
      {
        accessorKey: "duration_sec",
        header: "Süre",
        cell: ({ row }: any) => {
          const s = row.original.duration_sec
          return (
            <div className="flex items-center gap-1 text-sm text-muted-foreground tabular-nums">
              <Clock3 className="size-3" />
              {s === null ? "—" : formatTime(s)}
            </div>
          )
        },
      },
      {
        accessorKey: "size",
        header: "Boyut",
        cell: ({ row }: any) => (
          <div className="text-sm text-muted-foreground tabular-nums">{formatSize(row.original.size)}</div>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }: any) => {
          const video = row.original
          return (
            <div className="flex justify-end">
              <DeleteControl
                video={video}
                armed={confirmId === video.id}
                busy={busyId === video.id}
                onArm={() => setConfirmId(video.id)}
                onCancel={() => setConfirmId(null)}
                onConfirm={() => removeVideo(video.id)}
              />
            </div>
          )
        },
      },
    ],
    [confirmId, busyId, removeVideo]
  )

  const table = useReactTable({
    data: filtered,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  })

  return (
    <div className="flex flex-col h-full overflow-hidden p-6 gap-6">
      <div className="flex flex-col sm:flex-row items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Klipler</h1>
          <p className="text-muted-foreground mt-1">Yüklenen tüm video klipleri buradan yönetin.</p>
        </div>
        <Button onClick={() => setUploadOpen(true)}>
          <Film className="mr-2 size-4" /> Video yükle
        </Button>
      </div>

      <div className="flex flex-col sm:flex-row justify-between items-center gap-3 rounded-xl border bg-card p-3">
        <div className="relative w-full sm:w-64 transition-all duration-300 focus-within:sm:w-96">
          <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="pl-9 w-full bg-background"
            placeholder="Video ara..."
          />
        </div>
        
        <div className="flex gap-2">
          <Button
            variant={view === "grid" ? "secondary" : "ghost"}
            size="icon"
            onClick={() => setView("grid")}
          >
            <Grid2X2 className="size-4" />
          </Button>
          <Button
            variant={view === "list" ? "secondary" : "ghost"}
            size="icon"
            onClick={() => setView("list")}
          >
            <List className="size-4" />
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <MinimalCard key={i} className="animate-pulse h-48">
                <div className="h-full w-full bg-muted rounded-xl" />
              </MinimalCard>
            ))}
          </div>
        ) : error ? (
          <div className="flex flex-col items-center gap-3 rounded-xl border bg-card p-12 text-center">
            <HardDrive className="size-10 text-muted-foreground" />
            <p className="text-sm font-medium">Bağlantı kurulamadı</p>
            <p className="text-xs text-muted-foreground">Hata: {error}</p>
            <Button variant="outline" size="sm" onClick={loadVideos}>
              Tekrar dene
            </Button>
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex flex-col items-center gap-3 rounded-xl border bg-card p-12 text-center">
            <Film className="size-10 text-muted-foreground" />
            <p className="text-sm font-medium">Bulunamadı</p>
            <p className="text-xs text-muted-foreground">Eşleşen video yok veya henüz yüklenmedi.</p>
          </div>
        ) : view === "grid" ? (
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {filtered.map((video, i) => (
              <Link href={`/videos/${encodeURIComponent(video.id)}`} key={video.id}>
                <MinimalCard className="h-full flex flex-col justify-between p-4 group cursor-pointer hover:shadow-md transition-all">
                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center gap-3 overflow-hidden">
                      <div className={`flex size-10 shrink-0 items-center justify-center rounded-xl border ${CARD_COLORS[i % CARD_COLORS.length]}`}>
                        <Film className="size-5" />
                      </div>
                      <div className="flex flex-col overflow-hidden">
                        <MinimalCardTitle className="truncate text-sm font-bold">
                          {prettifyName(video.id)}
                        </MinimalCardTitle>
                        <MinimalCardDescription className="truncate text-xs">
                          {video.filename}
                        </MinimalCardDescription>
                      </div>
                    </div>
                    <Badge variant="secondary" className="text-[10px] tabular-nums shrink-0">
                      {formatSize(video.size)}
                    </Badge>
                  </div>
                  <div className="flex items-center justify-between mt-auto pt-4 border-t border-border/50">
                    <span className="flex items-center gap-1 text-xs text-muted-foreground tabular-nums">
                      <Clock3 className="size-3" />
                      {video.duration_sec === null ? "—" : formatTime(video.duration_sec)}
                    </span>
                    <div onClick={(e) => e.preventDefault()}>
                      <DeleteControl
                        video={video}
                        armed={confirmId === video.id}
                        busy={busyId === video.id}
                        onArm={() => setConfirmId(video.id)}
                        onCancel={() => setConfirmId(null)}
                        onConfirm={() => removeVideo(video.id)}
                      />
                    </div>
                  </div>
                </MinimalCard>
              </Link>
            ))}
          </div>
        ) : (
          <div className="rounded-md border bg-card">
            <Table>
              <TableHeader>
                {table.getHeaderGroups().map((headerGroup) => (
                  <TableRow key={headerGroup.id}>
                    {headerGroup.headers.map((header) => (
                      <TableHead key={header.id}>
                        {header.isPlaceholder
                          ? null
                          : flexRender(
                              header.column.columnDef.header,
                              header.getContext()
                            )}
                      </TableHead>
                    ))}
                  </TableRow>
                ))}
              </TableHeader>
              <TableBody>
                {table.getRowModel().rows?.length ? (
                  table.getRowModel().rows.map((row) => (
                    <TableRow
                      key={row.id}
                      data-state={row.getIsSelected() && "selected"}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <TableCell key={cell.id}>
                          {flexRender(
                            cell.column.columnDef.cell,
                            cell.getContext()
                          )}
                        </TableCell>
                      ))}
                    </TableRow>
                  ))
                ) : (
                  <TableRow>
                    <TableCell
                      colSpan={columns.length}
                      className="h-24 text-center"
                    >
                      Sonuç yok.
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      <VideoUploadDialog open={uploadOpen} onClose={handleUploadClose} />
    </div>
  )
}
