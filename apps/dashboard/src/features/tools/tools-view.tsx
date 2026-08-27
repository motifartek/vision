"use client"

import { useCallback, useEffect, useState } from "react"
import { Loader2, Plus, Trash, Pencil } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"

const API = process.env.NEXT_PUBLIC_API_URL ?? "/api"
// Rewrite config will redirect /api/tools to Gateway

type Tool = {
  id: number
  name: string
  title: string
  description: string
}

export function ToolsView() {
  const [tools, setTools] = useState<Tool[]>([])
  const [durum, setDurum] = useState<"bos" | "yukleniyor" | "kaydediliyor" | "hazir">("yukleniyor")
  const [hata, setHata] = useState<string | null>(null)

  const [duzenlenen, setDuzenlenen] = useState<Tool | null>(null)
  const [taslakName, setTaslakName] = useState("")
  const [taslakTitle, setTaslakTitle] = useState("")
  const [taslakDesc, setTaslakDesc] = useState("")
  const [modalAcik, setModalAcik] = useState(false)

  const yukle = useCallback(async () => {
    setDurum("yukleniyor")
    try {
      const r = await fetch(`${API}/tools`)
      if (!r.ok) throw new Error(`HTTP ${r.status}`)
      const d = (await r.json()) as { tools: Tool[] }
      setTools(d.tools)
      setHata(null)
    } catch {
      setHata("Toolbox API'ye ulaşılamıyor.")
      setTools([])
    } finally {
      setDurum("hazir")
    }
  }, [])

  useEffect(() => {
    void yukle()
  }, [yukle])

  const yeniEkle = () => {
    setDuzenlenen(null)
    setTaslakName("")
    setTaslakTitle("")
    setTaslakDesc("")
    setModalAcik(true)
  }

  const duzenle = (t: Tool) => {
    setDuzenlenen(t)
    setTaslakName(t.name)
    setTaslakTitle(t.title)
    setTaslakDesc(t.description)
    setModalAcik(true)
  }

  const kaydet = useCallback(async () => {
    if (!taslakName || !taslakTitle || !taslakDesc) {
        setHata("Tüm alanları doldurmalısınız.")
        return
    }
    setDurum("kaydediliyor")
    try {
      if (duzenlenen) {
        // Düzenleme
        const r = await fetch(`${API}/tools/${encodeURIComponent(duzenlenen.name)}`, {
          method: "PUT",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ name: taslakName, title: taslakTitle, description: taslakDesc }),
        })
        if (!r.ok) throw new Error(`HTTP ${r.status}`)
      } else {
        // Yeni
        const r = await fetch(`${API}/tools`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ id: 0, name: taslakName, title: taslakTitle, description: taslakDesc }),
        })
        if (!r.ok) throw new Error(`HTTP ${r.status}`)
      }
      
      setHata(null)
      setModalAcik(false)
      await yukle()
    } catch (e) {
      setHata((e as Error).message)
    } finally {
      setDurum("hazir")
    }
  }, [duzenlenen, taslakName, taslakTitle, taslakDesc, yukle])

  const sil = useCallback(async (name: string) => {
    if (!confirm(`'${name}' aracını silmek istediğinize emin misiniz?`)) return
    setDurum("kaydediliyor")
    try {
      const r = await fetch(`${API}/tools/${encodeURIComponent(name)}`, { method: "DELETE" })
      if (!r.ok) throw new Error(`HTTP ${r.status}`)
      setHata(null)
      await yukle()
    } catch (e) {
      setHata((e as Error).message)
    } finally {
      setDurum("hazir")
    }
  }, [yukle])

  return (
    <div className="flex w-full flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Dış Araçlar (External Tools)</h1>
          <p className="text-muted-foreground">
            Ajanların kararları sonucu tetiklenebilecek mock yetenekleri buradan yönetin.
          </p>
        </div>
        <Button onClick={yeniEkle} disabled={durum !== "hazir"}>
          <Plus className="mr-2 h-4 w-4" /> Yeni Araç
        </Button>
      </div>

      {hata && (
        <div className="rounded-md bg-destructive/15 p-3 text-sm text-destructive">
          {hata}
        </div>
      )}

      {durum === "yukleniyor" ? (
        <div className="flex h-32 items-center justify-center text-muted-foreground">
          <Loader2 className="h-6 w-6 animate-spin" />
        </div>
      ) : (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>İsim (Name)</TableHead>
                <TableHead>Başlık (Title)</TableHead>
                <TableHead>Açıklama</TableHead>
                <TableHead className="text-right">İşlemler</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {tools.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={4} className="h-24 text-center">
                    Kayıtlı araç bulunamadı.
                  </TableCell>
                </TableRow>
              ) : (
                tools.map((t) => (
                  <TableRow key={t.id}>
                    <TableCell className="font-mono font-medium">{t.name}</TableCell>
                    <TableCell>{t.title}</TableCell>
                    <TableCell className="text-muted-foreground">{t.description}</TableCell>
                    <TableCell className="text-right">
                      <Button variant="ghost" size="icon" onClick={() => duzenle(t)} disabled={durum !== "hazir"}>
                        <Pencil className="h-4 w-4" />
                      </Button>
                      <Button variant="ghost" size="icon" onClick={() => sil(t.name)} disabled={durum !== "hazir"}>
                        <Trash className="h-4 w-4 text-destructive" />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      )}

      <Dialog open={modalAcik} onOpenChange={setModalAcik}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{duzenlenen ? "Aracı Düzenle" : "Yeni Araç Ekle"}</DialogTitle>
            <DialogDescription>
              Ajan prompt'una ve arayüze yansıyacak dış araç tanımı.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="name">Sistem Adı (örn: call_ambulance)</Label>
              <Input
                id="name"
                value={taslakName}
                onChange={(e) => setTaslakName(e.target.value)}
                disabled={!!duzenlenen || durum === "kaydediliyor"}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="title">Görünür Başlık (örn: Ambulans Çağır)</Label>
              <Input
                id="title"
                value={taslakTitle}
                onChange={(e) => setTaslakTitle(e.target.value)}
                disabled={durum === "kaydediliyor"}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="desc">AI İçin Açıklama</Label>
              <Textarea
                id="desc"
                value={taslakDesc}
                onChange={(e) => setTaslakDesc(e.target.value)}
                disabled={durum === "kaydediliyor"}
                className="resize-none"
                rows={4}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setModalAcik(false)} disabled={durum === "kaydediliyor"}>
              İptal
            </Button>
            <Button onClick={kaydet} disabled={durum === "kaydediliyor"}>
              {durum === "kaydediliyor" && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Kaydet
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
