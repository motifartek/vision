import { requireSession } from "@/lib/auth/session"
import { AppShell } from "@/components/app-shell/app-shell"

/**
 * Tüm korumalı sayfalar bu layout altında yer alır.
 * requireSession() oturum yoksa otomatik olarak /auth/login'e yönlendirir.
 */
export default async function ProtectedLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const session = await requireSession()

  return (
    <AppShell
      firstName={session.firstName}
      lastName={session.lastName}
      email={session.email}
    >
      {children}
    </AppShell>
  )
}
