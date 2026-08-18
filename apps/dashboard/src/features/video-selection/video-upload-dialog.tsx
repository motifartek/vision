"use client"

import { useCallback, useRef, useState } from "react"
import { useRouter } from "next/navigation"
import { Upload, X, FileVideo, CheckCircle2, AlertCircle } from "lucide-react"
import { Button } from "@/components/ui/button"

const API = process.env.NEXT_PUBLIC_AUDIO_API ?? "http://127.0.0.1:8081"

type UploadState = "idle" | "dragging" | "uploading" | "done" | "error"

type Props = {
  open: boolean
  onClose: () => void
}

export function VideoUploadDialog({ open, onClose }: Props) {
  const router = useRouter()
  const inputRef = useRef<HTMLInputElement>(null)
  const [state, setState] = useState<UploadState>("idle")
  const [progress, setProgress] = useState(0)
  const [fileName, setFileName] = useState("")
  const [errorMsg, setErrorMsg] = useState("")
  const [uploadedId, setUploadedId] = useState("")

  const reset = useCallback(() => {
    setState("idle")
    setProgress(0)
    setFileName("")
    setErrorMsg("")
    setUploadedId("")
  }, [])

  const handleClose = useCallback(() => {
    if (state === "uploading") return // yükleme sırasında kapatma
    reset()
    onClose()
  }, [state, reset, onClose])

  const upload = useCallback(
    async (file: File) => {
      setState("uploading")
      setFileName(file.name)
      setProgress(0)
      setErrorMsg("")

      const formData = new FormData()
      formData.append("file", file)

      try {
        // XMLHttpRequest ile ilerleme takibi
        const result = await new Promise<{ id: string; filename: string }>((resolve, reject) => {
          const xhr = new XMLHttpRequest()

          xhr.upload.addEventListener("progress", (e) => {
            if (e.lengthComputable) {
              setProgress(Math.round((e.loaded / e.total) * 100))
            }
          })

          xhr.addEventListener("load", () => {
            if (xhr.status === 201) {
              resolve(JSON.parse(xhr.responseText))
            } else {
              let msg = "Yukleme basarisiz"
              try {
                const body = JSON.parse(xhr.responseText)
                msg = body.error || msg
              } catch {
                /* ignore */
              }
              reject(new Error(msg))
            }
          })

          xhr.addEventListener("error", () => reject(new Error("Ag hatasi")))
          xhr.addEventListener("abort", () => reject(new Error("Iptal edildi")))

          xhr.open("POST", `${API}/v1/upload`)
          xhr.send(formData)
        })

        setUploadedId(result.id)
        setState("done")

        // 1.5 sn sonra video sayfasına yönlendir
        setTimeout(() => {
          handleClose()
          router.push(`/videos/${result.id}`)
        }, 1500)
      } catch (err) {
        setErrorMsg(err instanceof Error ? err.message : "Bilinmeyen hata")
        setState("error")
      }
    },
    [router, handleClose],
  )

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setState("idle")
      const file = e.dataTransfer.files[0]
      if (file) upload(file)
    },
    [upload],
  )

  const onFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      if (file) upload(file)
    },
    [upload],
  )

  if (!open) return null

  return (
    // Backdrop
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm"
      onClick={handleClose}
    >
      {/* Dialog */}
      <div
        className="relative mx-4 flex w-full max-w-lg flex-col gap-4 rounded-2xl border bg-card p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Başlık */}
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">Video yükle</h2>
          <Button variant="ghost" size="icon" onClick={handleClose} aria-label="Kapat">
            <X />
          </Button>
        </div>

        {/* İçerik */}
        {state === "done" ? (
          <div className="flex flex-col items-center gap-3 py-8">
            <CheckCircle2 className="size-12 text-success" />
            <p className="text-sm font-medium">Yükleme tamamlandı!</p>
            <p className="text-xs text-muted-foreground">{fileName} → analiz sayfasına yönlendiriliyorsunuz…</p>
          </div>
        ) : state === "error" ? (
          <div className="flex flex-col items-center gap-3 py-8">
            <AlertCircle className="size-12 text-destructive" />
            <p className="text-sm font-medium">Yükleme başarısız</p>
            <p className="text-xs text-muted-foreground">{errorMsg}</p>
            <Button variant="outline" size="sm" onClick={reset}>
              Tekrar dene
            </Button>
          </div>
        ) : state === "uploading" ? (
          <div className="flex flex-col items-center gap-4 py-8">
            <FileVideo className="size-10 text-primary" />
            <p className="text-sm font-medium">{fileName}</p>
            {/* İlerleme çubuğu */}
            <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary transition-[width] duration-200"
                style={{ width: `${progress}%` }}
              />
            </div>
            <p className="font-mono text-xs tabular-nums text-muted-foreground">%{progress}</p>
          </div>
        ) : (
          /* Sürükle-bırak alanı */
          <div
            className={`flex cursor-pointer flex-col items-center gap-3 rounded-xl border-2 border-dashed px-4 py-12 transition-colors ${
              state === "dragging"
                ? "border-primary bg-primary/5"
                : "border-border hover:border-primary/50 hover:bg-accent/30"
            }`}
            onDragOver={(e) => {
              e.preventDefault()
              setState("dragging")
            }}
            onDragLeave={() => setState("idle")}
            onDrop={onDrop}
            onClick={() => inputRef.current?.click()}
          >
            <Upload className={`size-10 ${state === "dragging" ? "text-primary" : "text-muted-foreground"}`} />
            <div className="text-center">
              <p className="text-sm font-medium">
                {state === "dragging" ? "Bırakın" : "Videoyu sürükleyin veya tıklayın"}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">MP4, MKV, WebM, MOV, AVI</p>
            </div>
            <input
              ref={inputRef}
              type="file"
              accept="video/*"
              className="hidden"
              onChange={onFileSelect}
            />
          </div>
        )}
      </div>
    </div>
  )
}
