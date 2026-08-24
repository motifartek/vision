"use server"

import { ketoWrite, oryAdmin } from "@/lib/auth/ory"
import { revalidatePath } from "next/cache"

export async function addRoleMember(role: string, emailOrId: string) {
  let subjectId = emailOrId

  // Eğer bir email girildiyse, Kratos üzerinden ID'sini bul
  if (emailOrId.includes("@")) {
    try {
      const { data: identities } = await oryAdmin.listIdentities()
      const user = identities.find(
        (id) => (id.traits as any)?.email === emailOrId
      )
      
      if (!user) {
        return { error: "Kullanıcı bulunamadı." }
      }
      subjectId = user.id
    } catch (error) {
      console.error("Kratos fetch error:", error)
      return { error: "Kullanıcı sorgulanamadı." }
    }
  }

  try {
    await ketoWrite.createRelationship({
      createRelationshipBody: {
        namespace: "Group",
        object: role,
        relation: "members",
        subject_id: subjectId,
      },
    })
    revalidatePath("/roles")
    return { success: true }
  } catch (error) {
    console.error("Keto write error:", error)
    return { error: "Yetki eklenirken bir hata oluştu." }
  }
}

export async function removeRoleMember(role: string, subjectId: string) {
  try {
    await ketoWrite.deleteRelationships({
      namespace: "Group",
      object: role,
      relation: "members",
      subjectId,
    })
    revalidatePath("/roles")
    return { success: true }
  } catch (error) {
    console.error("Keto delete error:", error)
    return { error: "Yetki kaldırılırken bir hata oluştu." }
  }
}
