"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"
import { useState } from "react"
import { Bell, CircleHelp, Gauge, House, Search, Settings2, SlidersHorizontal, Video, Wrench } from "lucide-react"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, PopoverTrigger } from "@/components/ui/popover"
import { cn } from "@/lib/utils"

const nav = [
  { href: "/", label: "Kontrol Paneli", description: "Projelerin genel durumunu görüntüle", icon: House },
  { href: "/videos", label: "Video Seçimi", description: "İşlenecek videoyu seç ve düzenle", icon: Video },
  { href: "/tools", label: "Araçlar", description: "İşleme araçlarını ve modülleri yönet", icon: Wrench },
]

function SidebarItem({ item, active }: { item: (typeof nav)[number]; active: boolean }) {
  const [open, setOpen] = useState(false)
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <span onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)} onFocus={() => setOpen(true)} onBlur={() => setOpen(false)}>
        <PopoverTrigger render={<Link href={item.href} aria-label={item.label} className={cn("flex size-10 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-foreground", active && "bg-accent text-primary")} />}>
          <item.icon className="size-5" />
        </PopoverTrigger>
      </span>
      <PopoverContent side="right" align="center" className="w-56">
        <PopoverHeader><PopoverTitle>{item.label}</PopoverTitle><PopoverDescription>{item.description}</PopoverDescription></PopoverHeader>
      </PopoverContent>
    </Popover>
  )
}

type AppShellProps = {
  children: React.ReactNode
  title?: string
  description?: string
  firstName?: string
  lastName?: string
  email?: string
}

export function AppShell({ children, title, description, firstName = "", lastName = "", email = "" }: AppShellProps) {
  const pathname = usePathname()
  const initials = [firstName[0], lastName[0]].filter(Boolean).join("").toUpperCase() || "?"

  return (
    <div className="flex min-h-dvh bg-background">
      <aside className="fixed inset-y-0 left-0 flex w-16 flex-col items-center border-r bg-card py-4">
        <Link href="/" className="mb-6 flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground" aria-label="MotifAI ana sayfa">
          <Gauge className="size-5" />
        </Link>
        <nav className="flex flex-1 flex-col items-center gap-2" aria-label="Ana navigasyon">
          {nav.map((item) => {
            const active = item.href === "/" ? pathname === "/" : pathname.startsWith(item.href)
            return <SidebarItem key={item.href} item={item} active={active} />
          })}
        </nav>
        <Link href="/settings" aria-label="Ayarlar">
          <Button variant="ghost" size="icon"><Settings2 /></Button>
        </Link>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col pl-16">
        <header className="sticky top-0 flex h-16 items-center justify-between border-b bg-background/95 px-4 backdrop-blur md:px-6">
          <div className="min-w-0">
            {title && <h1 className="truncate text-base font-semibold">{title}</h1>}
            {description && <p className="hidden text-xs text-muted-foreground sm:block">{description}</p>}
          </div>
          <div className="flex items-center gap-2">
            <div className="relative hidden md:block">
              <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
              <Input className="w-56 pl-9" placeholder="Proje veya video ara" aria-label="Ara" />
            </div>
            <Button variant="ghost" size="icon" aria-label="Yardım"><CircleHelp /></Button>
            <Button variant="ghost" size="icon" aria-label="Bildirimler"><Bell /></Button>
            <Link href="/settings" aria-label={`${firstName} ${lastName} — Ayarlar`}>
              <Avatar className="size-8 cursor-pointer">
                <AvatarFallback title={email}>{initials}</AvatarFallback>
              </Avatar>
            </Link>
          </div>
        </header>
        <main className="flex-1 p-4 md:p-6">{children}</main>
      </div>
    </div>
  )
}
