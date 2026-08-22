"use client"

import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Separator } from "@/components/ui/separator"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ShieldCheck, User } from "lucide-react"
import { useState } from "react"

export function SettingsView() {
  const [tab, setTab] = useState<"profile" | "security">("profile")

  return (
    <AppShell title="Ayarlar" description="Hesap bilgilerinizi ve tercihlerinizi yönetin">
      <div className="mx-auto flex max-w-3xl flex-col gap-6">
        <Tabs value={tab} onValueChange={(v) => setTab(v as typeof tab)}>
          <TabsList>
            <TabsTrigger value="profile"><User className="size-4" /> Profil</TabsTrigger>
            <TabsTrigger value="security"><ShieldCheck className="size-4" /> Güvenlik</TabsTrigger>
          </TabsList>
        </Tabs>

        {tab === "profile" && (
          <Card>
            <CardHeader>
              <CardTitle>Profil Bilgileri</CardTitle>
              <CardDescription>İsim ve e-posta bilgilerinizi güncelleyin</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <div className="grid grid-cols-2 gap-4">
                <div className="flex flex-col gap-1.5">
                  <label className="text-sm font-medium">İsim</label>
                  <Input defaultValue="Deniz" />
                </div>
                <div className="flex flex-col gap-1.5">
                  <label className="text-sm font-medium">Soyisim</label>
                  <Input defaultValue="Karaman" />
                </div>
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">E-posta</label>
                <div className="flex items-center gap-2">
                  <Input defaultValue="deniz@motif.ai" type="email" />
                  <Badge variant="secondary">Doğrulandı</Badge>
                </div>
              </div>
              <Separator />
              <div className="flex justify-end">
                <Button>Kaydet</Button>
              </div>
            </CardContent>
          </Card>
        )}

        {tab === "security" && (
          <Card>
            <CardHeader>
              <CardTitle>Şifre Değiştir</CardTitle>
              <CardDescription>Hesabınızın güvenliği için güçlü bir şifre belirleyin</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">Mevcut şifre</label>
                <Input type="password" />
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">Yeni şifre</label>
                <Input type="password" />
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">Yeni şifre (tekrar)</label>
                <Input type="password" />
              </div>
              <Separator />
              <div className="flex justify-end">
                <Button>Şifreyi Güncelle</Button>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </AppShell>
  )
}
