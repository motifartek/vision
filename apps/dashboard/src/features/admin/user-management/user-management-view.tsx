import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Shield, Users, UserX } from "lucide-react"

const users = [
  { id: "1", name: "Ahmet Yıldız", email: "ahmet@motif.ai", role: "Admin", status: "Aktif" },
  { id: "2", name: "Merve Kaya", email: "merve@motif.ai", role: "Üye", status: "Aktif" },
  { id: "3", name: "Can Demir", email: "can@motif.ai", role: "Üye", status: "Askıya alındı" },
]

export function UserManagementView() {
  return (
    <div className="flex flex-col gap-4"><div className="mb-2"><h1 className="text-2xl font-bold tracking-tight"></h1><p className="text-muted-foreground"></p></div>
      <div className="mx-auto flex max-w-7xl flex-col gap-6">
        <section className="flex items-center justify-between">
          <div>
            <h2 className="text-xl font-semibold">Kullanıcılar</h2>
            <p className="text-sm text-muted-foreground">{users.length} kullanıcı kayıtlı</p>
          </div>
          <Button><Users className="size-4" /> Kullanıcı Davet Et</Button>
        </section>
        <Card>
          <CardHeader>
            <CardTitle>Kullanıcı Listesi</CardTitle>
            <CardDescription>Tüm kayıtlı kullanıcılar ve rolleri</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y">
              {users.map((user) => (
                <div key={user.id} className="flex items-center justify-between py-3">
                  <div className="flex flex-col">
                    <span className="text-sm font-medium">{user.name}</span>
                    <span className="text-xs text-muted-foreground">{user.email}</span>
                  </div>
                  <div className="flex items-center gap-3">
                    <Badge variant={user.role === "Admin" ? "default" : "secondary"}>
                      {user.role === "Admin" && <Shield className="size-3" />}
                      {user.role}
                    </Badge>
                    <Badge variant={user.status === "Aktif" ? "outline" : "destructive"}>
                      {user.status}
                    </Badge>
                    <Button variant="ghost" size="icon" aria-label="Kullanıcıyı askıya al">
                      <UserX className="size-4" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
