"use client"

import { useState, useTransition } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardAction } from "@/components/ui/card"
import { ShieldPlus, X, UserPlus, Loader2, ShieldAlert } from "lucide-react"
import { addRoleMember, removeRoleMember } from "@/app/(protected)/(admin)/roles/actions"
import { Badge } from "@/components/ui/badge"

interface RolesViewProps {
  roles: Record<string, any[]>
}

export function RolesView({ roles }: RolesViewProps) {
  const [isPending, startTransition] = useTransition()
  const [inputs, setInputs] = useState<Record<string, string>>({
    admin: "",
    editor: "",
    viewer: "",
  })

  const handleAdd = (role: string) => {
    const value = inputs[role]
    if (!value) return

    startTransition(async () => {
      const res = await addRoleMember(role, value)
      if (res.error) {
        alert(res.error) // TODO: toast
      } else {
        setInputs((prev) => ({ ...prev, [role]: "" }))
      }
    })
  }

  const handleRemove = (role: string, userId: string) => {
    if (!confirm("Bu kullanıcının yetkisini almak istediğinize emin misiniz?")) return
    startTransition(async () => {
      const res = await removeRoleMember(role, userId)
      if (res.error) {
        alert(res.error) // TODO: toast
      }
    })
  }

  const roleLabels: Record<string, string> = {
    admin: "Sistem Yöneticisi (Admin)",
    editor: "Editör (Düzenleyici)",
    viewer: "İzleyici (Sadece Okur)",
  }

  return (
    <div className="w-full">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Roller ve Yetkiler</h1>
          <p className="text-sm text-muted-foreground">Keto (Zanzibar) üzerinden grup ve izinleri yönetin.</p>
        </div>
      </div>
      
      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
        {Object.entries(roles).map(([role, members]) => (
          <Card key={role} className="flex flex-col h-full">
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <CardTitle className="text-lg">{roleLabels[role]}</CardTitle>
                <Badge variant={role === "admin" ? "default" : "secondary"}>
                  {members.length} Üye
                </Badge>
              </div>
              <CardDescription>
                Bu gruptaki kullanıcıların yetki kapsamı.
              </CardDescription>
            </CardHeader>
            <CardContent className="flex-1">
              <div className="space-y-3">
                {members.length > 0 ? (
                  members.map((member) => (
                    <div key={member.id} className="flex items-center justify-between bg-muted/50 p-2 rounded-md">
                      <div className="flex flex-col overflow-hidden">
                        <span className="text-sm font-medium truncate">{member.name || "İsimsiz"}</span>
                        <span className="text-xs text-muted-foreground truncate">{member.email}</span>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 text-muted-foreground hover:text-destructive"
                        onClick={() => handleRemove(role, member.id)}
                        disabled={isPending}
                      >
                        <X className="h-4 w-4" />
                      </Button>
                    </div>
                  ))
                ) : (
                  <div className="text-sm text-muted-foreground text-center py-4 border-dashed border-2 rounded-md">
                    Henüz üye yok.
                  </div>
                )}
              </div>
            </CardContent>
            <div className="p-4 border-t bg-muted/20">
              <div className="flex gap-2">
                <Input
                  placeholder="Kullanıcı E-posta / ID"
                  className="h-9"
                  value={inputs[role]}
                  onChange={(e) => setInputs({ ...inputs, [role]: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleAdd(role)
                  }}
                  disabled={isPending}
                />
                <Button size="sm" className="h-9 px-3" onClick={() => handleAdd(role)} disabled={isPending || !inputs[role]}>
                  {isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <UserPlus className="h-4 w-4" />}
                </Button>
              </div>
            </div>
          </Card>
        ))}
      </div>
    </div>
  )
}
