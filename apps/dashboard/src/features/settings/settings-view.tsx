"use client"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

export function SettingsView() {
  return (
    <div className="flex flex-col gap-8 w-full">
      <div className="mb-2">
        <h1 className="text-2xl font-bold tracking-tight">Ayarlar</h1>
        <p className="text-muted-foreground">Hesap bilgilerinizi ve tercihlerinizi yönetin</p>
      </div>

      <Tabs defaultValue="profile" className="w-full">
        <TabsList className="mb-6">
          <TabsTrigger value="profile">Profil</TabsTrigger>
          <TabsTrigger value="security">Güvenlik</TabsTrigger>
          <TabsTrigger value="notifications">Bildirimler</TabsTrigger>
        </TabsList>
        
        <TabsContent value="profile" className="flex flex-col gap-6 w-full max-w-4xl">
          <section className="space-y-4">
            <h3 className="text-lg font-medium">Kişisel Bilgiler</h3>
            <p className="text-sm text-muted-foreground mb-4">Ad, soyad ve iletişim bilgilerinizi güncelleyin.</p>
            <div className="grid gap-6 md:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="firstName">Ad</Label>
                <Input id="firstName" defaultValue="Deniz" />
              </div>
              <div className="space-y-2">
                <Label htmlFor="lastName">Soyad</Label>
                <Input id="lastName" defaultValue="Karaman" />
              </div>
              <div className="space-y-2 md:col-span-2">
                <Label htmlFor="email">E-posta</Label>
                <Input id="email" type="email" defaultValue="is.denizkaraman@gmail.com" />
              </div>
            </div>
            <div className="pt-4">
              <Button>Değişiklikleri Kaydet</Button>
            </div>
          </section>
        </TabsContent>

        <TabsContent value="security" className="flex flex-col gap-6 w-full max-w-4xl">
          <section className="space-y-4">
            <h3 className="text-lg font-medium">Şifre Değiştir</h3>
            <p className="text-sm text-muted-foreground mb-4">Güvenliğiniz için güçlü bir şifre kullanın.</p>
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="current-password">Mevcut Şifre</Label>
                <Input id="current-password" type="password" />
              </div>
              <div className="space-y-2">
                <Label htmlFor="new-password">Yeni Şifre</Label>
                <Input id="new-password" type="password" />
              </div>
              <div className="space-y-2">
                <Label htmlFor="confirm-password">Yeni Şifre (Tekrar)</Label>
                <Input id="confirm-password" type="password" />
              </div>
            </div>
            <div className="pt-4">
              <Button>Şifreyi Güncelle</Button>
            </div>
          </section>
        </TabsContent>
        
        <TabsContent value="notifications" className="flex flex-col gap-6 w-full max-w-4xl">
          <section className="space-y-4">
            <h3 className="text-lg font-medium">E-posta Bildirimleri</h3>
            <div className="flex items-center justify-between py-4 border-b">
              <div className="space-y-0.5">
                <Label className="text-base">Proje Tamamlandı</Label>
                <p className="text-sm text-muted-foreground">Video işleme tamamlandığında e-posta al</p>
              </div>
              <Switch defaultChecked />
            </div>
            <div className="flex items-center justify-between py-4 border-b">
              <div className="space-y-0.5">
                <Label className="text-base">Giriş Uyarıları</Label>
                <p className="text-sm text-muted-foreground">Yeni bir cihazdan giriş yapıldığında e-posta al</p>
              </div>
              <Switch defaultChecked />
            </div>
          </section>
        </TabsContent>
      </Tabs>
    </div>
  )
}
