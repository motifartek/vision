import { redirect } from "next/navigation"
import { getSession } from "@/lib/auth/session"
import { ketoRead } from "@/lib/auth/ory"

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

  try {
    const { data } = await ketoRead.checkPermission({
      namespace: "Group",
      object: "admin",
      relation: "members",
      subjectId: session.id,
    })

    if (!data.allowed) {
      // Yetkisi yoksa anasayfaya at
      redirect("/")
    }
  } catch (error) {
    console.error("Keto yetkilendirme hatası:", error)
    redirect("/")
  }

  return <>{children}</>
}
