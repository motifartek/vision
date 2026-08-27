import { redirect } from "next/navigation"
import { getSession } from "@/lib/auth/session"
import { ketoRead } from "@/lib/auth/ory"

/**
 * Admin route grubu layout'u.
 *
 * Keto üzerinden `Group:admin#members` üyeliği kontrol edilir; üye olmayan
 * anasayfaya döner.
 *
 * `redirect()` içeride `NEXT_REDIRECT` fırlatarak çalışıyor, bu yüzden
 * `try` bloğunun **dışında** çağrılmak zorunda: içeride çağrılırsa kendi
 * `catch`'imiz yönlendirmeyi yakalar, konsola sahte bir "Keto hatası" basar
 * ve gerçek Keto arızalarını yetki reddinden ayırt edilemez hâle getirir.
 */
export default async function AdminLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const session = await getSession()
  if (!session) redirect("/auth/login")

  let allowed = false
  try {
    const { data } = await ketoRead.checkPermission({
      namespace: "Group",
      object: "admin",
      relation: "members",
      subjectId: session.id,
    })
    allowed = data.allowed
  } catch (error) {
    // Keto'ya ulaşılamıyorsa kapıyı kapalı tut: yetki bilinmiyorsa yok say.
    console.error("Keto yetkilendirme hatası:", error)
  }

  if (!allowed) redirect("/")

  return <>{children}</>
}
