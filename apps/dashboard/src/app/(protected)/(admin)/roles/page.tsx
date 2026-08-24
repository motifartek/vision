import { RolesView } from "@/features/roles/roles-view"
import { ketoRelationRead, oryAdmin } from "@/lib/auth/ory"

export default async function RolesPage() {
  const rolesData: Record<string, any[]> = {
    admin: [],
    editor: [],
    viewer: [],
  }

  try {
    // Tüm kullanıcıları Kratos'tan çek (İsim ve E-postaları göstermek için)
    const { data: identities } = await oryAdmin.listIdentities()
    const identityMap = new Map(identities.map((id) => [id.id, id]))

    // Her bir rol için Keto'dan üyeleri sorgula
    for (const role of Object.keys(rolesData)) {
      const { data } = await ketoRelationRead.getRelationships({
        namespace: "Group",
        object: role,
        relation: "members",
      })

      if (data.relation_tuples) {
        rolesData[role] = data.relation_tuples
          .map((tuple) => {
            const userId = tuple.subject_id
            const user = identityMap.get(userId as string)
            if (!user) return null
            return {
              id: userId,
              email: (user.traits as any)?.email,
              name: `${(user.traits as any)?.name?.first || ""} ${(user.traits as any)?.name?.last || ""}`.trim(),
            }
          })
          .filter(Boolean)
      }
    }
  } catch (error) {
    console.error("Roles fetch error:", error)
  }

  return <RolesView roles={rolesData} />
}
