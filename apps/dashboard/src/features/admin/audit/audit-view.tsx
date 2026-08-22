import { AppShell } from "@/components/app-shell/app-shell"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Activity } from "lucide-react"

const logs = [
  { id: "1", user: "Ahmet Yıldız", action: "Kullanıcı davet etti", target: "can@motif.ai", time: "2 dk önce", level: "info" },
  { id: "2", user: "Sistem", action: "Giriş başarısız (3 deneme)", target: "bilinmeyen@motif.ai", time: "15 dk önce", level: "warn" },
  { id: "3", user: "Merve Kaya", action: "Video sildi", target: "Podcast Highlight 03", time: "1 sa önce", level: "info" },
  { id: "4", user: "Sistem", action: "Hesap askıya alındı", target: "can@motif.ai", time: "2 sa önce", level: "error" },
]

const levelVariant = {
  info: "outline" as const,
  warn: "secondary" as const,
  error: "destructive" as const,
}

export function AuditView() {
  return (
    <AppShell title="Denetim Günlüğü" description="Sistemdeki tüm kritik işlemlerin kaydı">
      <div className="mx-auto flex max-w-7xl flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Activity className="size-4 text-primary" /> Audit Log
            </CardTitle>
            <CardDescription>Son sistem olayları</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y">
              {logs.map((log) => (
                <div key={log.id} className="flex items-center justify-between gap-4 py-3">
                  <div className="flex min-w-0 flex-1 flex-col">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium">{log.action}</span>
                      <Badge variant={levelVariant[log.level as keyof typeof levelVariant]}>
                        {log.level}
                      </Badge>
                    </div>
                    <span className="text-xs text-muted-foreground">
                      {log.user} → {log.target}
                    </span>
                  </div>
                  <span className="shrink-0 text-xs text-muted-foreground">{log.time}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </AppShell>
  )
}
