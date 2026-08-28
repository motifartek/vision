"use client"

import { useCallback, useRef, useState } from "react"
import { useRouter } from "next/navigation"
import { Upload, X, FileVideo, CheckCircle2, AlertCircle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"

const API = process.env.NEXT_PUBLIC_SONIC_API ?? "/api/sonic"
const STREAM = process.env.NEXT_PUBLIC_STREAM_API ?? "/api/stream"

/** Sunucudaki `VIDEO_EXTENSIONS` ile birebir aynı (upload.rs). */
const VIDEO_EXTENSIONS = ["mp4", "mkv", "webm", "mov", "avi", "flv", "wmv", "m4v"]

function isVideoFile(name: string) {
  const ext = name.split(".").pop()?.toLowerCase()
  return Boolean(ext && VIDEO_EXTENSIONS.includes(ext))
}

type UploadState = "idle" | "dragging" | "uploading" | "done" | "error"

type Props = {
  open: boolean
  onClose: () => void
}

export function VideoUploadDialog({ open, onClose }: Props) {
  const router = useRouter()
  const inputRef = useRef<HTMLInputElement>(null)
  const [state, setState] = useState<UploadState>("idle")
  /** Ses servisine gönderim; görüntü yüklemesinden bağımsız başarısız olabilir. */
  const [audioState, setAudioState] = useState<"idle" | "uploading" | "done" | "failed">("idle")
  const [progress, setProgress] = useState(0)
  const [fileName, setFileName] = useState("")
  const [errorMsg, setErrorMsg] = useState("")
  /**
   * Sürükleme sayacı: `dragleave` alt öğelerin üzerinden geçerken de tetikleniyor
   * ve tek bir bayrakla çerçeve titriyordu. Giren/çıkan olayları sayınca yalnız
   * gerçekten alandan çıkıldığında sıfırlanıyor.
   */
  const dragDepth = useRef(0)

  const reset = useCallback(() => {
    setState("idle")
    setProgress(0)
    setFileName("")
    setErrorMsg("")
    dragDepth.current = 0
  }, [])

  const handleClose = useCallback(() => {
    if (state === "uploading") return // yükleme sırasında kapatma
    reset()
    onClose()
  }, [state, reset, onClose])

  const upload = useCallback(
    async (file: File) => {
      setAudioState("idle")
      // Sunucu da reddediyor (415), ama kullanıcıyı 2 GB göndermeden uyarmak gerek.
      if (!isVideoFile(file.name)) {
        setFileName(file.name)
        setErrorMsg(`Yalnız video dosyaları yüklenebilir (${VIDEO_EXTENSIONS.join(", ")}).`)
        setState("error")
        return
      }

      setState("uploading")
      setFileName(file.name)
      setProgress(0)
      setErrorMsg("")

      try {
        // --- birincil: görüntü servisi ---
        //
        // Görsel analiz ürünün kendisi, dolayısıyla yönlendirmede kullanılan
        // kimlik `stream`'den geliyor. Önceden yükleme ses servisine yapılıyor
        // ve kimlik oradan alınıyordu; ses servisi ayakta değilken video
        // eklemek tümüyle imkânsız hâle geliyordu, oysa görsel taraf onsuz da
        // çalışıyor.
        const result = await new Promise<{ id: string }>((resolve, reject) => {
          const xhr = new XMLHttpRequest()

          xhr.upload.addEventListener("progress", (e) => {
            if (e.lengthComputable) {
              setProgress(Math.round((e.loaded / e.total) * 100))
            }
          })

          xhr.addEventListener("load", () => {
            // stream 201 döndürüyor; başka 2xx de kabul.
            if (xhr.status >= 200 && xhr.status < 300) {
              resolve(JSON.parse(xhr.responseText))
            } else {
              let msg = "Yükleme başarısız"
              try {
                const body = JSON.parse(xhr.responseText)
                msg = body.error || msg
              } catch {
                /* gövde JSON değilse mesajı olduğu gibi bırak */
              }
              reject(new Error(msg))
            }
          })

          xhr.addEventListener("error", () => reject(new Error("Ağ hatası")))
          xhr.addEventListener("abort", () => reject(new Error("İptal edildi")))

          const sf = new FormData()
          sf.append("file", file)
          xhr.open("POST", `${STREAM}/v1/videos`)
          xhr.send(sf)
        })

        // --- ikincil: ses servisi ---
        //
        // İki servis ayrı depo tutuyor ve ortak olan tek şey dosya adı;
        // detay sayfası ikisini o ad üzerinden eşleştiriyor. Tarayıcı baytları
        // zaten elinde tuttuğu için ikinci gönderim ek indirme gerektirmiyor.
        //
        // Başarısız olursa yükleme başarısız sayılmıyor: görsel analiz
        // çalışmaya devam ediyor, yalnız ses analizi o video için kapalı
        // kalıyor.
        setAudioState("uploading")
        try {
          const af = new FormData()
          af.append("file", file)
          const r = await fetch(`${API}/v1/upload`, { method: "POST", body: af })
          setAudioState(r.ok ? "done" : "failed")
        } catch {
          setAudioState("failed")
        }

        setState("done")

        // 1.5 sn sonra video sayfasına yönlendir
        setTimeout(() => {
          handleClose()
          router.push(`/videos/${encodeURIComponent(result.id)}`)
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
      dragDepth.current = 0
      setState("idle")
      const file = e.dataTransfer.files[0]
      if (file) upload(file)
    },
    [upload],
  )

  const onFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      // Aynı dosyayı ikinci kez seçebilmek için girdiyi boşalt.
      e.target.value = ""
      if (file) upload(file)
    },
    [upload],
  )

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) handleClose()
      }}
    >
      <DialogContent>
        <div className="flex items-center justify-between">
          <DialogTitle>Video yükle</DialogTitle>
          <DialogClose
            render={
              <Button variant="ghost" size="icon" aria-label="Kapat" disabled={state === "uploading"}>
                <X />
              </Button>
            }
          />
        </div>

        {state === "done" ? (
          <div className="flex flex-col items-center gap-3 py-8">
            <CheckCircle2 className="size-12 text-success" />
            <p className="text-sm font-medium">Yükleme tamamlandı!</p>
            <DialogDescription>{fileName} → analiz sayfasına yönlendiriliyorsunuz…</DialogDescription>
            {audioState === "failed" && (
              <p className="max-w-xs text-center text-[11px] leading-snug text-muted-foreground">
                Ses servisine ulaşılamadı; görsel analiz çalışır, ses analizi bu video için
                kapalı kalır.
              </p>
            )}
          </div>
        ) : state === "error" ? (
          <div className="flex flex-col items-center gap-3 py-8">
            <AlertCircle className="size-12 text-destructive" />
            <p className="text-sm font-medium">Yükleme başarısız</p>
            <DialogDescription>{errorMsg}</DialogDescription>
            <Button variant="outline" size="sm" onClick={reset}>
              Tekrar dene
            </Button>
          </div>
        ) : state === "uploading" ? (
          <div className="flex flex-col items-center gap-4 py-8">
            <FileVideo className="size-10 text-primary" />
            <p className="text-sm font-medium">{fileName}</p>
            <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary transition-[width] duration-200"
                style={{ width: `${progress}%` }}
                role="progressbar"
                aria-valuenow={progress}
                aria-valuemin={0}
                aria-valuemax={100}
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
            onDragEnter={(e) => {
              e.preventDefault()
              dragDepth.current += 1
              setState("dragging")
            }}
            onDragOver={(e) => e.preventDefault()}
            onDragLeave={() => {
              dragDepth.current = Math.max(0, dragDepth.current - 1)
              if (dragDepth.current === 0) setState("idle")
            }}
            onDrop={onDrop}
            onClick={() => inputRef.current?.click()}
          >
            <Upload className={`size-10 ${state === "dragging" ? "text-primary" : "text-muted-foreground"}`} />
            <div className="text-center">
              <p className="text-sm font-medium">
                {state === "dragging" ? "Bırakın" : "Videoyu sürükleyin veya tıklayın"}
              </p>
              <DialogDescription className="mt-1">MP4, MKV, WebM, MOV, AVI</DialogDescription>
            </div>
            <input
              ref={inputRef}
              type="file"
              accept={VIDEO_EXTENSIONS.map((e) => `.${e}`).join(",")}
              className="hidden"
              onChange={onFileSelect}
            />
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
