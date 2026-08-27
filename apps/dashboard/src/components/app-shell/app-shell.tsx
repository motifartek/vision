"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"
import { useEffect, useState } from "react"
import { Bell, CircleHelp, Gauge, House, Search, Settings2, Video, Wrench, LogOut, User, Sparkles, Shield, MessageSquareText } from "lucide-react"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command"
import { ory } from "@/lib/auth/ory"
import { cn } from "@/lib/utils"

const nav = [
  { href: "/", label: "Kontrol Paneli", description: "Projelerin genel durumunu görüntüle", icon: House },
  { href: "/videos", label: "Video Seçimi", description: "İşlenecek videoyu seç ve düzenle", icon: Video },
  { href: "/tools", label: "Araçlar", description: "İşleme araçlarını ve modülleri yönet", icon: Wrench },
  { href: "/users", label: "Kullanıcılar", description: "Sistemdeki tüm kullanıcıları yönetin", icon: User },
  { href: "/roles", label: "Roller ve Yetkiler", description: "Erişim hakları ve grupları belirleyin", icon: Shield },
  { href: "/prompts", label: "Prompt'lar", description: "Modele giden metni düzenleyin", icon: MessageSquareText },
]

function SidebarItem({ item, active }: { item: (typeof nav)[number]; active: boolean }) {
  return (
    <Tooltip>
      <TooltipTrigger render={
        <Link 
          href={item.href} 
          aria-label={item.label} 
          className={cn(
            "flex size-10 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-foreground", 
            active && "bg-accent text-primary"
          )}
        >
          <item.icon className="size-5" />
        </Link>
      } />
      <TooltipContent side="right" align="center" className="flex flex-col gap-1 font-normal text-foreground items-start">
        <span className="font-semibold">{item.label}</span>
        <span className="text-xs text-muted-foreground">{item.description}</span>
      </TooltipContent>
    </Tooltip>
  )
}

type AppShellProps = {
  children: React.ReactNode
  firstName?: string
  lastName?: string
  email?: string
}

export function AppShell({ children, firstName = "", lastName = "", email = "" }: AppShellProps) {
  const pathname = usePathname()
  const initials = [firstName[0], lastName[0]].filter(Boolean).join("").toUpperCase() || "?"
  const [openCommand, setOpenCommand] = useState(false)

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpenCommand((open) => !open)
      }
    }
    document.addEventListener("keydown", down)
    return () => document.removeEventListener("keydown", down)
  }, [])

  return (
    <div className="flex min-h-dvh bg-background">
      <CommandDialog open={openCommand} onOpenChange={setOpenCommand}>
        <CommandInput placeholder="Proje, araç veya ayar arayın..." />
        <CommandList>
          <CommandEmpty>Sonuç bulunamadı.</CommandEmpty>
          <CommandGroup heading="Kısayollar">
            <CommandItem onSelect={() => { setOpenCommand(false); window.location.href = "/videos" }}>
              <Video className="mr-2 h-4 w-4" />
              <span>Yeni Video İşle</span>
            </CommandItem>
            <CommandItem onSelect={() => { setOpenCommand(false); window.location.href = "/settings" }}>
              <Settings2 className="mr-2 h-4 w-4" />
              <span>Ayarlar</span>
            </CommandItem>
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup heading="Araçlar">
            <CommandItem onSelect={() => setOpenCommand(false)}>
              <Sparkles className="mr-2 h-4 w-4" />
              <span>AI Kırpma Aracı</span>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </CommandDialog>

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
        <Button variant="ghost" size="icon" nativeButton={false} render={<Link href="/settings" aria-label="Ayarlar" />}>
          <Settings2 />
        </Button>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col pl-16">
        <header className="sticky top-0 flex h-14 items-center justify-between border-b bg-background/95 px-4 backdrop-blur md:px-6 z-10">
          <div className="flex flex-1 items-center">
            <Button onClick={() => setOpenCommand(true)} variant="outline" className="relative h-8 w-full justify-start rounded-[0.5rem] bg-muted/50 text-sm font-normal text-muted-foreground shadow-none sm:pr-12 md:w-64">
              <Search className="mr-2 h-4 w-4" />
              <span className="hidden lg:inline-flex">Uygulama içinde ara...</span>
              <span className="inline-flex lg:hidden">Ara...</span>
              <kbd className="pointer-events-none absolute right-[0.3rem] top-[0.3rem] hidden h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium opacity-100 sm:flex">
                <span className="text-xs">⌘</span>K
              </kbd>
            </Button>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="icon" aria-label="Yardım"><CircleHelp /></Button>
            <Button variant="ghost" size="icon" aria-label="Bildirimler"><Bell /></Button>
            <DropdownMenu>
              <DropdownMenuTrigger nativeButton={false} render={
                <Avatar className="size-8 cursor-pointer ml-2 hover:opacity-80 transition-opacity">
                  <AvatarFallback title={email}>{initials}</AvatarFallback>
                </Avatar>
              } />
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuGroup>
                  <DropdownMenuLabel className="font-normal">
                    <div className="flex flex-col space-y-1">
                      <p className="text-sm font-medium leading-none">{firstName} {lastName}</p>
                      <p className="text-xs leading-none text-muted-foreground">{email}</p>
                    </div>
                  </DropdownMenuLabel>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuItem render={
                  <Link href="/settings" className="cursor-pointer flex items-center">
                    <User className="mr-2 h-4 w-4" />
                    <span>Profil ve Ayarlar</span>
                  </Link>
                } />
                <DropdownMenuSeparator />
                <DropdownMenuItem 
                  onClick={async () => {
                    try {
                      const { data } = await ory.createBrowserLogoutFlow()
                      window.location.href = data.logout_url
                    } catch (err) {
                      window.location.href = "/api/auth/self-service/logout/browser"
                    }
                  }}
                  className="text-destructive focus:bg-destructive focus:text-destructive-foreground cursor-pointer"
                >
                  <LogOut className="mr-2 h-4 w-4" />
                  <span>Çıkış Yap</span>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </header>
        <main className="flex-1 p-4 md:p-6 w-full">{children}</main>
      </div>
    </div>
  )
}
