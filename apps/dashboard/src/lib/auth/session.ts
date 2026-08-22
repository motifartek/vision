import { FrontendApi, Configuration } from "@ory/client"
import { redirect } from "next/navigation"
import { cookies } from "next/headers"

// Gateway üzerinden Kratos'a ulaşır.
// GATEWAY_URL env değişkeni hazır olduğunda otomatik olarak Rust Gateway'e yönlenir.
const kratos = new FrontendApi(
  new Configuration({
    basePath: process.env.GATEWAY_URL ?? "http://127.0.0.1:4433",
    baseOptions: {
      withCredentials: true,
    },
  })
)

export type SessionIdentity = {
  id: string
  firstName: string
  lastName: string
  email: string
}

/**
 * Mevcut oturumu döner. Oturum yoksa null döner.
 * Server Component ve Server Action'larda kullanılır.
 */
export async function getSession(): Promise<SessionIdentity | null> {
  const cookieStore = await cookies()
  const sessionCookie = cookieStore
    .getAll()
    .map((c) => `${c.name}=${c.value}`)
    .join("; ")

  try {
    const { data: session } = await kratos.toSession({
      cookie: sessionCookie,
    })

    const traits = session.identity?.traits as Record<string, unknown>

    return {
      id: session.identity?.id ?? "",
      firstName: (traits?.name as Record<string, string>)?.first ?? "",
      lastName: (traits?.name as Record<string, string>)?.last ?? "",
      email: (traits?.email as string) ?? "",
    }
  } catch {
    return null
  }
}

/**
 * Oturum yoksa /auth/login sayfasına yönlendirir.
 * Korumalı layout'larda çağrılır.
 */
export async function requireSession(): Promise<SessionIdentity> {
  const session = await getSession()
  if (!session) {
    redirect("/auth/login")
  }
  return session
}
