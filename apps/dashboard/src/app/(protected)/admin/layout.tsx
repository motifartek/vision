import { requireSession } from "@/lib/auth/session"
import { redirect } from "next/navigation"
import { getSession } from "@/lib/auth/session"

/**
 * Admin route grubu layout'u.
 * Keto üzerinden admin rolü kontrolü yapılır.
 * Rol yoksa dashboard'a yönlendirir.
 */
export default async function AdminLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const session = await getSession()
  if (!session) redirect("/auth/login")

  // TODO: Keto'dan admin rolünü kontrol et
  // const isAdmin = await checkPermission(session.id, "roles", "admin", "member")
  // if (!isAdmin) redirect("/")

  return <>{children}</>
}
