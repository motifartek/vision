"use server"

import { oryAdmin } from "@/lib/auth/ory"
import { revalidatePath } from "next/cache"

export async function toggleUserState(userId: string, currentState: string) {
  const newState = currentState === "active" ? "inactive" : "active"

  try {
    await oryAdmin.patchIdentity({
      id: userId,
      jsonPatch: [
        {
          op: "replace",
          path: "/state",
          value: newState,
        },
      ],
    })
    
    revalidatePath("/users")
    return { success: true, newState }
  } catch (error) {
    console.error("Identity patch error:", error)
    return { error: "Kullanıcı durumu güncellenirken bir hata oluştu." }
  }
}

export async function createTestUser() {
  const randomId = Math.floor(Math.random() * 10000)
  const email = `test_${randomId}@motif.ai`

  try {
    const { data } = await oryAdmin.createIdentity({
      createIdentityBody: {
        schema_id: "default",
        state: "active",
        traits: {
          email: email,
          name: {
            first: "Test",
            last: `Kullanıcı ${randomId}`,
          },
        },
        credentials: {
          password: {
            config: {
              password: "password123",
            },
          },
        },
      },
    })
    
    revalidatePath("/users")
    return { success: true, email }
  } catch (error: any) {
    console.error("Test user creation error:", error.response?.data || error)
    return { error: "Test kullanıcısı oluşturulamadı." }
  }
}
