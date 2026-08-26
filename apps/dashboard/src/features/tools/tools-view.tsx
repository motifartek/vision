"use client"

import { BrainCircuit, Captions, CloudUpload, Mic2, Scissors, Sparkles, WandSparkles } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

const tools = [
  { name: "Akıllı Kesim", desc: "Sessiz bölümleri ve tekrarları otomatik temizler.", icon: Scissors, active: true, label: "Kurgu" },
  { name: "Konuşma İyileştirme", desc: "Gürültüyü azaltır, ses seviyesini dengeler.", icon: Mic2, active: true, label: "Ses" },
  { name: "Otomatik Altyazı", desc: "Türkçe konuşmayı zaman kodlu metne dönüştürür.", icon: Captions, active: true, label: "Metin" },
  { name: "Sahne Analizi", desc: "Önemli anları ve konu geçişlerini algılar.", icon: BrainCircuit, active: true, label: "Yapay zekâ" },
  { name: "Görsel İyileştirme", desc: "Renk, kontrast ve netliği sahne bazında ayarlar.", icon: WandSparkles, active: false, label: "Görüntü" },
  { name: "Otomatik Yükleme", desc: "Hazır çıktıları bağlı depolama alanına gönderir.", icon: CloudUpload, active: false, label: "Dağıtım" },
]

export function ToolsView() {
  return (
    <div className="flex flex-col gap-4"><div className="mb-2"><h1 className="text-2xl font-bold tracking-tight"></h1><p className="text-muted-foreground"></p></div>
      <div className="mx-auto flex max-w-7xl flex-col gap-6">
        <section className="flex flex-col justify-between gap-4 rounded-xl border bg-card p-6 md:flex-row md:items-center">
          <div>
            <Badge variant="secondary"><Sparkles /> 4 araç etkin</Badge>
            <h2 className="mt-3 text-balance text-2xl font-semibold">İşleme araçları</h2>
            <p className="mt-1 max-w-xl text-sm text-muted-foreground">
              Her projede otomatik çalışacak modülleri seçin ve çalışma sırasını yönetin.
            </p>
          </div>
          <Button>Değişiklikleri uygula</Button>
        </section>
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {tools.map((tool) => (
            <Card key={tool.name} className={tool.active ? "" : "opacity-70"}>
              <CardHeader>
                <div className="flex size-10 items-center justify-center rounded-lg bg-muted">
                  <tool.icon className="size-5 text-primary" />
                </div>
                <CardAction>
                  <Switch defaultChecked={tool.active} aria-label={`${tool.name} aracını etkinleştir`} />
                </CardAction>
                <CardTitle>{tool.name}</CardTitle>
                <CardDescription>{tool.desc}</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="flex items-center justify-between rounded-lg border bg-background p-3">
                  <span className="text-xs text-muted-foreground">Çalışma modu</span>
                  <Select defaultValue="otomatik">
                    <SelectTrigger className="w-28" size="sm"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="otomatik">Otomatik</SelectItem>
                        <SelectItem value="dengeli">Dengeli</SelectItem>
                        <SelectItem value="manuel">Manuel</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </div>
              </CardContent>
              <CardFooter className="justify-between">
                <Badge variant="outline">{tool.label}</Badge>
                <Button variant="ghost" size="sm">Yapılandır</Button>
              </CardFooter>
            </Card>
          ))}
        </div>
      </div>
    </div>
  )
}
