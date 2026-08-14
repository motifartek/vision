"use client"

import Link from "next/link"
import { useState } from "react"
import { Clock3, Film, Grid2X2, List, Search, SlidersHorizontal } from "lucide-react"
import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

const videos = [
 { id:"podcast-highlight-03", title:"Podcast Highlight 03", category:"Podcast", duration:"18:42", status:"Taslak", color:"bg-primary/20" },
 { id:"founder-interview", title:"Kurucu Röportajı", category:"Röportaj", duration:"42:18", status:"Analiz edildi", color:"bg-success/15" },
 { id:"product-spring", title:"Bahar Ürün Tanıtımı", category:"Reklam", duration:"01:24", status:"Hazır", color:"bg-media/15" },
 { id:"weekly-recap", title:"Haftalık Ekip Özeti", category:"Kurumsal", duration:"08:16", status:"Taslak", color:"bg-muted" },
 { id:"customer-story", title:"Müşteri Hikâyesi", category:"Röportaj", duration:"12:05", status:"Hazır", color:"bg-success/15" },
 { id:"launch-teaser", title:"Lansman Fragmanı", category:"Sosyal", duration:"00:46", status:"İşleniyor", color:"bg-primary/20" },
]

export function VideoSelectionView(){
 const [query,setQuery]=useState("")
 const filtered=videos.filter(v=>v.title.toLocaleLowerCase("tr").includes(query.toLocaleLowerCase("tr")))
 return <AppShell title="Video Seçimi" description="Düzenlemek istediğiniz içeriği seçin">
  <div className="mx-auto flex max-w-7xl flex-col gap-6">
   <div className="flex flex-col justify-between gap-4 md:flex-row md:items-end"><div><h2 className="text-balance text-2xl font-semibold">Video kütüphanesi</h2><p className="mt-1 text-sm text-muted-foreground">{videos.length} içerik arasından projenizi seçin.</p></div><Button><Film data-icon="inline-start" /> Video yükle</Button></div>
   <div className="flex flex-col justify-between gap-3 rounded-xl border bg-card p-3 sm:flex-row"><div className="relative flex-1 sm:max-w-sm"><Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"/><Input value={query} onChange={e=>setQuery(e.target.value)} className="pl-9" placeholder="Video ara..." /></div><div className="flex gap-2"><Button variant="outline"><SlidersHorizontal data-icon="inline-start"/> Filtrele</Button><Button variant="secondary" size="icon" aria-label="Izgara görünümü"><Grid2X2/></Button><Button variant="ghost" size="icon" aria-label="Liste görünümü"><List/></Button></div></div>
   <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{filtered.map(video=><Card key={video.id} className="group transition-colors hover:bg-accent/30"><CardContent className="pt-0"><Link href={`/videos/${video.id}`} className={`panel-grid flex aspect-video items-center justify-center rounded-lg border ${video.color}`}><div className="flex size-14 items-center justify-center rounded-full border bg-background/80 transition-transform group-hover:scale-105"><Film className="size-6"/></div></Link></CardContent><CardHeader><div className="flex items-center justify-between gap-3"><CardTitle className="truncate">{video.title}</CardTitle><Badge variant="outline">{video.status}</Badge></div></CardHeader><CardFooter className="justify-between text-xs text-muted-foreground"><span>{video.category}</span><span className="flex items-center gap-1"><Clock3 className="size-3"/>{video.duration}</span></CardFooter></Card>)}</div>
  </div>
 </AppShell>
}
