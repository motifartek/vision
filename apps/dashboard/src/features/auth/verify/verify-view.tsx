"use client"

import Link from "next/link"
import { useSearchParams, useRouter } from "next/navigation"
import { useEffect, useState, Suspense } from "react"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
import { VerificationFlow } from "@ory/client"
import { ory } from "@/lib/auth/ory"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

const schema = z.object({
  code: z.string().min(1, "Doğrulama kodu gereklidir"),
})

type FormValues = z.infer<typeof schema>

function VerifyForm() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const [flow, setFlow] = useState<VerificationFlow | null>(null)
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
      window.location.href = "/api/auth/self-service/verification/browser"
      return
    }

    ory.getVerificationFlow({ id: flowId })
      .then(({ data }) => {
        setFlow(data)
        const csrfNode = data.ui.nodes.find((n: any) => n.attributes.name === "csrf_token")
        if (csrfNode) setCsrfToken((csrfNode.attributes as any).value)
      })
      .catch((err) => {
        console.error("Flow fetch error:", err)
        router.push("/auth/verify")
      })
  }, [searchParams, router])

  async function onSubmit(values: FormValues) {
    if (!flow) return

    try {
      await ory.updateVerificationFlow({
        flow: flow.id,
        updateVerificationFlowBody: {
          method: "code",
          code: values.code,
          csrf_token: csrfToken,
        }
      })
      // Başarılı olursa Dashboard'a yönlendir
      window.location.href = "/"
    } catch (error: any) {
      if (error.response?.status === 400) {
        const data = error.response.data
        if (data.ui?.messages) {
          setError("code", { message: data.ui.messages[0]?.text || "İşlem başarısız" })
        }
      } else {
        setError("code", { message: "Bir hata oluştu." })
      }
    }
  }

  if (isSubmitSuccessful) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Doğrulama Başarılı</CardTitle>
          <CardDescription>
            Hesabınız başarıyla doğrulandı. Yönlendiriliyorsunuz...
          </CardDescription>
        </CardHeader>
      </Card>
    )
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>E-posta Doğrulama</CardTitle>
        <CardDescription>
          E-posta adresinize gönderilen 6 haneli doğrulama kodunu girin.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4" noValidate>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="code" className="text-sm font-medium">Doğrulama Kodu</label>
            <Input id="code" placeholder="000000" disabled={!flow} {...register("code")} />
            {errors.code && (
              <p className="text-xs text-destructive">{errors.code.message}</p>
            )}
          </div>
          <Button type="submit" disabled={isSubmitting || !flow}>
            {!flow ? "Bağlanıyor..." : isSubmitting ? "Doğrulanıyor..." : "Doğrula"}
          </Button>
          <p className="text-center text-xs text-muted-foreground mt-4">
            E-posta ulaşmadı mı?{" "}
            {/* Burada Kratos'un default resend akışına tekrar yönlendiriyoruz */}
            <Link href="/api/auth/self-service/verification/browser" className="text-foreground underline underline-offset-4">
              Tekrar gönder
            </Link>
          </p>
        </form>
      </CardContent>
    </Card>
  )
}

export function VerifyView() {
  return (
    <Suspense fallback={<div>Yükleniyor...</div>}>
      <VerifyForm />
    </Suspense>
  )
}
