import Link from "next/link"
import { ArrowUpRight, CheckCircle2, Clock3, Film, Play, Sparkles } from "lucide-react"
import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"

const stats = [
  { label: "Toplam proje", value: "24", note: "Bu ay +6", icon: Film },
  { label: "İşlenen süre", value: "18,4 sa", note: "%12 artış", icon: Clock3 },
  { label: "Tamamlanan", value: "91%", note: "22 başarılı", icon: CheckCircle2 },
]
const projects = [
  { name: "Podcast Highlight 03", type: "Podcast", progress: 72, status: "İşleniyor" },
  { name: "Ürün Tanıtımı — Bahar", type: "Reklam", progress: 100, status: "Hazır" },
  { name: "Kurucu Röportajı", type: "Röportaj", progress: 44, status: "Analiz" },
]

export function DashboardView() {
  return <AppShell title="Kontrol Paneli" description="Video üretim akışınızın güncel görünümü">
    <div className="mx-auto flex max-w-7xl flex-col gap-6">
      <section className="flex flex-col justify-between gap-4 rounded-xl border bg-card p-6 md:flex-row md:items-center">
        <div className="flex flex-col gap-2"><Badge variant="secondary" className="w-fit"><Sparkles /> Akıllı iş akışı</Badge><h2 className="max-w-2xl text-balance text-2xl font-semibold md:text-3xl">İçeriğinizi daha hızlı, daha net ve yayına hazır hale getirin.</h2><p className="max-w-xl text-pretty text-sm leading-6 text-muted-foreground">Son projelerinize devam edin veya yapay zekâ destekli yeni bir video işleme akışı başlatın.</p></div>
        <Button render={<Link href="/videos" />} size="lg"><Play data-icon="inline-start" /> Yeni video seç</Button>
      </section>
      <section className="grid gap-4 md:grid-cols-3">{stats.map((stat) => <Card key={stat.label}><CardHeader><CardDescription>{stat.label}</CardDescription><CardAction><stat.icon className="size-4 text-muted-foreground" /></CardAction><CardTitle className="text-2xl">{stat.value}</CardTitle></CardHeader><CardContent><p className="text-xs text-muted-foreground">{stat.note}</p></CardContent></Card>)}</section>
      <section className="grid gap-6 lg:grid-cols-[1.5fr_1fr]">
        <Card><CardHeader><CardTitle>Son projeler</CardTitle><CardDescription>Kaldığınız yerden devam edin</CardDescription><CardAction><Button variant="ghost" size="sm" render={<Link href="/videos" />}>Tümünü gör <ArrowUpRight data-icon="inline-end" /></Button></CardAction></CardHeader><CardContent className="flex flex-col gap-3">{projects.map((project) => <Link href="/videos/podcast-highlight-03" key={project.name} className="flex flex-col gap-3 rounded-lg border bg-background p-4 transition-colors hover:bg-accent sm:flex-row sm:items-center"><div className="flex size-11 items-center justify-center rounded-lg bg-muted"><Film className="size-5 text-primary" /></div><div className="min-w-0 flex-1"><div className="flex items-center justify-between gap-3"><p className="truncate text-sm font-medium">{project.name}</p><Badge variant="outline">{project.status}</Badge></div><p className="mb-2 text-xs text-muted-foreground">{project.type}</p><Progress value={project.progress} /></div></Link>)}</CardContent></Card>
        <Card><CardHeader><CardTitle>Haftalık kapasite</CardTitle><CardDescription>İşleme kotası kullanımı</CardDescription></CardHeader><CardContent className="flex flex-col gap-6"><div className="flex items-end justify-between"><span className="text-3xl font-semibold">68%</span><span className="text-xs text-muted-foreground">34 / 50 saat</span></div><Progress value={68} /><div className="grid grid-cols-7 items-end gap-2">{[38,64,48,82,56,90,68].map((h,i)=><div key={i} className="flex flex-col items-center gap-2"><div className="w-full rounded-sm bg-primary/70" style={{height:`${h}px`}} /><span className="text-[10px] text-muted-foreground">{["Pzt","Sal","Çar","Per","Cum","Cmt","Paz"][i]}</span></div>)}</div></CardContent></Card>
      </section>
    </div>
  </AppShell>
}
