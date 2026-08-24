"use client"

import Link from "next/link"
import { useSearchParams, useRouter } from "next/navigation"
import { useEffect, useState, Suspense } from "react"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
import { RecoveryFlow } from "@ory/client"
import { ory } from "@/lib/auth/ory"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

const schema = z.object({
  email: z.string().email("Geçerli bir e-posta girin"),
})

type FormValues = z.infer<typeof schema>

function RecoveryForm() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const [flow, setFlow] = useState<RecoveryFlow | null>(null)
  const [csrfToken, setCsrfToken] = useState("")

  const {
    register,
    handleSubmit,
    setError,
    formState: { errors, isSubmitting, isSubmitSuccessful },
  } = useForm<FormValues>({ resolver: zodResolver(schema) })

  useEffect(() => {
    const flowId = searchParams.get("flow")
    if (!flowId) {
      window.location.href = "/api/auth/self-service/recovery/browser"
      return
    }

    ory.getRecoveryFlow({ id: flowId })
      .then(({ data }) => {
        setFlow(data)
        const csrfNode = data.ui.nodes.find((n: any) => n.attributes.name === "csrf_token")
        if (csrfNode) setCsrfToken((csrfNode.attributes as any).value)
      })
      .catch((err) => {
        console.error("Flow fetch error:", err)
        router.push("/auth/recovery")
      })
  }, [searchParams, router])

  async function onSubmit(values: FormValues) {
    if (!flow) return

    try {
      await ory.updateRecoveryFlow({
        flow: flow.id,
        updateRecoveryFlowBody: {
          method: "code",
          email: values.email,
          csrf_token: csrfToken,
        }
      })
    } catch (error: any) {
      if (error.response?.status === 400) {
        const data = error.response.data
        if (data.ui?.messages) {
          setError("email", { message: data.ui.messages[0]?.text || "İşlem başarısız" })
        }
      } else {
        setError("email", { message: "Bir hata oluştu." })
      }
    }
  }

  if (isSubmitSuccessful) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Bağlantı Gönderildi</CardTitle>
          <CardDescription>
            Şifre sıfırlama bağlantısı e-posta adresinize gönderildi.
          </CardDescription>
        </CardHeader>
      </Card>
    )
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Şifremi Unuttum</CardTitle>
        <CardDescription>
          Kayıtlı e-posta adresinizi girin, şifre sıfırlama bağlantısı göndereceğiz.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4" noValidate>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="email" className="text-sm font-medium">E-posta</label>
            <Input id="email" type="email" autoComplete="email" disabled={!flow} {...register("email")} />
            {errors.email && (
              <p className="text-xs text-destructive">{errors.email.message}</p>
            )}
          </div>
          <Button type="submit" disabled={isSubmitting || !flow}>
            {!flow ? "Bağlanıyor..." : isSubmitting ? "Gönderiliyor..." : "Bağlantı Gönder"}
          </Button>
          <p className="text-center text-xs text-muted-foreground">
            <Link href="/auth/login" className="text-foreground underline underline-offset-4">
              Giriş sayfasına dön
            </Link>
          </p>
        </form>
      </CardContent>
    </Card>
  )
}

export function RecoveryView() {
  return (
    <Suspense fallback={<div>Yükleniyor...</div>}>
      <RecoveryForm />
    </Suspense>
  )
}
