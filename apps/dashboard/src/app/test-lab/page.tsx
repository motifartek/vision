"use client"

import { useState, useEffect } from "react"
import { Upload, Activity, ShieldCheck, TerminalSquare } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"

const GATEWAY_URL = "http://localhost:8000"

export default function TestLab() {
  const [file, setFile] = useState<File | null>(null)
  const [uploading, setUploading] = useState(false)
  const [videoId, setVideoId] = useState<string | null>(null)
  const [status, setStatus] = useState("Bekleniyor...")
  
  const [report, setReport] = useState<any>(null)
  const [traces, setTraces] = useState<string[]>([])
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  const handleUpload = async () => {
    if (!file) return
    setUploading(true)
    setStatus("Yükleniyor...")
    setTraces([])
    setReport(null)
    
    const formData = new FormData()
    formData.append("file", file)

    try {
      const res = await fetch(`${GATEWAY_URL}/api/stream/v1/videos`, {
        method: "POST",
        body: formData,
      })
      
      if (!res.ok) throw new Error("Yükleme başarısız")
      
      const data = await res.json()
      const id = data.id || data.video_id
      setVideoId(id)
      setStatus(`Yüklendi! NATS tetiklendi. Video ID: ${id}. SSE Bekleniyor...`)
    } catch (err) {
      setStatus(`Hata: ${err}`)
    } finally {
      setUploading(false)
    }
  }

  useEffect(() => {
    if (!videoId) return

    setStatus(`Orchestrator bekleniyor... SSE bağlantısı açıldı: ${videoId}`)
    
    const sse = new EventSource(`${GATEWAY_URL}/api/videos/${videoId}/events`)
    
    // Anlık mikroservis logları
    sse.addEventListener("trace", (event) => {
      try {
        const parsed = JSON.parse(event.data)
        setTraces((prev) => [...prev, parsed.message])
      } catch (e) {
        console.error("Trace parse hatası", e)
      }
    })

    // Nihai VLM sonucu
    sse.addEventListener("report", (event) => {
      try {
        const parsed = JSON.parse(event.data)
        setReport(parsed)
        setStatus("Analiz Tamamlandı! Rapor alındı.")
      } catch (e) {
        console.error("Report parse hatası", e)
      }
    })

    // Fallback için (eski standart mesajlar)
    sse.onmessage = (event) => {
       // do nothing for unnamed events
    }

    return () => {
      sse.close()
    }
  }, [videoId])

  if (!mounted) return null

  return (
    <div className="container mx-auto p-8 max-w-4xl space-y-6">
      <div>
        <h1 className="text-3xl font-bold mb-2">🧪 Orchestrator Test Laboratuvarı</h1>
        <p className="text-muted-foreground">Bu sayfa, yeni kurulan Macro Loop akışını test etmek için tasarlanmıştır.</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>1. Video Yükle & Tetikle</CardTitle>
          <CardDescription>Bir video yüklediğinizde Stream servisi otomatik olarak NATS üzerinden olayı yayınlar.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center gap-4">
            <input 
              type="file" 
              accept="video/mp4,video/webm" 
              onChange={(e) => setFile(e.target.files?.[0] || null)}
            />
            <Button onClick={handleUpload} disabled={!file || uploading}>
              {uploading ? <Activity className="mr-2 h-4 w-4 animate-spin" /> : <Upload className="mr-2 h-4 w-4" />}
              {uploading ? "Yükleniyor..." : "Yükle ve Ajanları Uyandır"}
            </Button>
          </div>
          
          <div className="bg-secondary/50 p-4 rounded-md flex items-center justify-between">
            <span className="font-mono text-sm">{status}</span>
            {uploading && <Badge variant="secondary" className="animate-pulse">İşlemde</Badge>}
          </div>
        </CardContent>
      </Card>

      {traces.length > 0 && (
        <Card className="border-blue-500 shadow-sm shadow-blue-500/20">
          <CardHeader>
            <CardTitle className="text-blue-500 flex items-center">
              <TerminalSquare className="mr-2" /> 2. Canlı Mikroservis İzleri (NATS & Orchestrator)
            </CardTitle>
            <CardDescription>Ajanların birbirine pas atarken arka planda oluşturduğu PostgreSQL Trace Logları anlık akıyor.</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="bg-black text-green-400 p-4 rounded-md font-mono text-sm space-y-2 h-64 overflow-y-auto">
              {traces.map((t, i) => (
                <div key={i} className="animate-in fade-in slide-in-from-bottom-2">
                  <span className="opacity-50 select-none mr-2">{">"}</span> {t}
                </div>
              ))}
              {!report && (
                <div className="animate-pulse">
                  <span className="opacity-50 select-none mr-2">{">"}</span> _
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {report && (
        <Card className="border-green-500 shadow-sm shadow-green-500/20">
          <CardHeader>
            <CardTitle className="text-green-500 flex items-center">
              <ShieldCheck className="mr-2" /> 3. Nihai VLM Analiz Sonucu
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <pre className="bg-muted p-4 rounded-md overflow-auto text-xs">
              {JSON.stringify(report, null, 2)}
            </pre>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
