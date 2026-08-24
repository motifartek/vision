"use client"

import Link from "next/link"
import { useSearchParams, useRouter } from "next/navigation"
import { useEffect, useState, Suspense } from "react"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
import { LoginFlow } from "@ory/client"
import { ory } from "@/lib/auth/ory"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

const schema = z.object({
  email: z.string().email("Geçerli bir e-posta girin"),
  password: z.string().min(8, "Şifre en az 8 karakter olmalı"),
})

type FormValues = z.infer<typeof schema>

function LoginForm() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const [flow, setFlow] = useState<LoginFlow | null>(null)
  const [csrfToken, setCsrfToken] = useState("")

  const {
    register,
    handleSubmit,
    setError,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) })

  useEffect(() => {
    const flowId = searchParams.get("flow")
    if (!flowId) {
      // Flow yoksa, başlatmak için browser endpoint'ine git
      window.location.href = "/api/auth/self-service/login/browser"
      return
    }

    // Flow ID varsa detaylarını çek
    ory.getLoginFlow({ id: flowId })
      .then(({ data }) => {
        setFlow(data)
        // CSRF Token'ı çıkart
        const csrfNode = data.ui.nodes.find((n: any) => n.attributes.name === "csrf_token")
        if (csrfNode) setCsrfToken((csrfNode.attributes as any).value)
      })
      .catch((err) => {
        console.error("Flow fetch error:", err)
        router.push("/auth/login") // Hata varsa baştan başla
      })
  }, [searchParams, router])

  async function onSubmit(values: FormValues) {
    if (!flow) return

    try {
      await ory.updateLoginFlow({
        flow: flow.id,
        updateLoginFlowBody: {
          method: "password",
          identifier: values.email,
          password: values.password,
          csrf_token: csrfToken,
        }
      })
      // Başarılı giriş sonrası yönlendir
      window.location.href = "/"
    } catch (error: any) {
      if (error.response?.status === 400) {
        // UI nodes içindeki hata mesajlarını yakala
        const data = error.response.data
        if (data.ui?.messages) {
          setError("email", { message: data.ui.messages[0]?.text || "Giriş başarısız" })
        }
      } else {
        setError("email", { message: "Bir hata oluştu. Lütfen tekrar deneyin." })
      }
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Giriş Yap</CardTitle>
        <CardDescription>MotifAI hesabınıza erişin</CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4" noValidate>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="email" className="text-sm font-medium">E-posta</label>
            <Input
              id="email"
              type="email"
              placeholder="ornek@motif.ai"
              autoComplete="email"
              disabled={!flow}
              {...register("email")}
            />
            {errors.email && (
              <p className="text-xs text-destructive">{errors.email.message}</p>
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between">
              <label htmlFor="password" className="text-sm font-medium">Şifre</label>
              <Link href="/auth/recovery" className="text-xs text-muted-foreground hover:text-foreground">
                Şifremi unuttum
              </Link>
            </div>
            <Input
              id="password"
              type="password"
              autoComplete="current-password"
              disabled={!flow}
              {...register("password")}
            />
            {errors.password && (
              <p className="text-xs text-destructive">{errors.password.message}</p>
            )}
          </div>
          <Button type="submit" className="mt-2" disabled={isSubmitting || !flow}>
            {!flow ? "Bağlanıyor..." : isSubmitting ? "Giriş yapılıyor..." : "Giriş Yap"}
          </Button>
          <p className="text-center text-xs text-muted-foreground">
            Hesabınız yok mu?{" "}
            <Link href="/auth/register" className="text-foreground underline underline-offset-4">
              Kayıt Ol
            </Link>
          </p>
        </form>
      </CardContent>
    </Card>
  )
}

export function LoginView() {
  return (
    <Suspense fallback={<div>Yükleniyor...</div>}>
      <LoginForm />
    </Suspense>
  )
}
