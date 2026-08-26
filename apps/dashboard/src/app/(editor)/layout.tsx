import { requireSession } from "@/lib/auth/session"

/**
 * AppShell (Sidebar vb.) içermeyen tam ekran korumalı sayfalar (Örn: Video Editörü)
 */
export default async function EditorLayout({
  children,
}: {
  children: React.ReactNode
}) {
  await requireSession()

  return <>{children}</>
}
