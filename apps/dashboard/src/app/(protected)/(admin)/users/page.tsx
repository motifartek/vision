import { UsersView } from "@/features/users/users-view"
import { oryAdmin } from "@/lib/auth/ory"

export default async function UsersPage() {
  // TODO: Add requireAdmin check here once Keto is wired up
  let identities: any[] = []
  try {
    const { data } = await oryAdmin.listIdentities()
    identities = data
  } catch (err) {
    console.error("Failed to fetch identities", err)
  }

  return <UsersView users={identities} />
}
