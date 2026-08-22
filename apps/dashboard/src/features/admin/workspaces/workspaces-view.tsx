import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Briefcase, Users } from "lucide-react"

const workspaces = [
  { id: "1", name: "MotifAI Prodüksiyon", members: 4, plan: "Pro", status: "Aktif" },
  { id: "2", name: "Demo Workspace", members: 1, plan: "Ücretsiz", status: "Aktif" },
  { id: "3", name: "Eski Proje", members: 0, plan: "Ücretsiz", status: "Pasif" },
]

export function WorkspacesView() {
  return (
    <AppShell title="Workspace Yönetimi" description="Tüm çalışma alanlarını görüntüleyin ve yönetin">
      <div className="mx-auto flex max-w-7xl flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Workspace Listesi</CardTitle>
            <CardDescription>{workspaces.length} workspace kayıtlı</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y">
              {workspaces.map((ws) => (
                <div key={ws.id} className="flex items-center justify-between py-3">
                  <div className="flex items-center gap-3">
                    <div className="flex size-9 items-center justify-center rounded-lg bg-muted">
                      <Briefcase className="size-4 text-primary" />
                    </div>
                    <div className="flex flex-col">
                      <span className="text-sm font-medium">{ws.name}</span>
                      <span className="flex items-center gap-1 text-xs text-muted-foreground">
                        <Users className="size-3" /> {ws.members} üye
                      </span>
                    </div>
                  </div>
                  <div className="flex items-center gap-3">
                    <Badge variant="secondary">{ws.plan}</Badge>
                    <Badge variant={ws.status === "Aktif" ? "outline" : "secondary"}>
                      {ws.status}
                    </Badge>
                    <Button variant="ghost" size="sm">Yönet</Button>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </AppShell>
  )
}
