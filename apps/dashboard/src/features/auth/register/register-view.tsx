"use client"

import Link from "next/link"
import { useSearchParams, useRouter } from "next/navigation"
import { useEffect, useState, Suspense } from "react"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
import { RegistrationFlow } from "@ory/client"
import { ory } from "@/lib/auth/ory"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

const schema = z.object({
  firstName: z.string().min(2, "İsim en az 2 karakter olmalı"),
  lastName: z.string().min(2, "Soyisim en az 2 karakter olmalı"),
  email: z.string().email("Geçerli bir e-posta girin"),
  password: z.string().min(8, "Şifre en az 8 karakter olmalı"),
})

type FormValues = z.infer<typeof schema>

function RegisterForm() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const [flow, setFlow] = useState<RegistrationFlow | null>(null)
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
      window.location.href = "/api/auth/self-service/registration/browser"
      return
    }

    // Flow ID varsa detaylarını çek
    ory.getRegistrationFlow({ id: flowId })
      .then(({ data }) => {
        setFlow(data)
        // CSRF Token'ı çıkart
        const csrfNode = data.ui.nodes.find((n: any) => n.attributes.name === "csrf_token")
        if (csrfNode) setCsrfToken((csrfNode.attributes as any).value)
      })
      .catch((err) => {
        console.error("Flow fetch error:", err)
        router.push("/auth/register") // Hata varsa baştan başla
      })
  }, [searchParams, router])

  async function onSubmit(values: FormValues) {
    if (!flow) return

    try {
      await ory.updateRegistrationFlow({
        flow: flow.id,
        updateRegistrationFlowBody: {
          method: "password",
          traits: {
            email: values.email,
            name: { first: values.firstName, last: values.lastName },
          },
          password: values.password,
          csrf_token: csrfToken,
        }
      })
      // Başarılı kayıt sonrası doğrulama sayfasına yönlendir
      window.location.href = "/auth/verify"
    } catch (error: any) {
      if (error.response?.status === 400) {
        // UI nodes içindeki hata mesajlarını yakala
        const data = error.response.data
        if (data.ui?.messages) {
          setError("email", { message: data.ui.messages[0]?.text || "Kayıt başarısız" })
        }
      } else {
        setError("email", { message: "Bir hata oluştu. Lütfen tekrar deneyin." })
      }
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Hesap Oluştur</CardTitle>
        <CardDescription>MotifAI'ya katılın</CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4" noValidate>
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <label htmlFor="firstName" className="text-sm font-medium">İsim</label>
              <Input id="firstName" {...register("firstName")} disabled={!flow} />
              {errors.firstName && (
                <p className="text-xs text-destructive">{errors.firstName.message}</p>
              )}
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="lastName" className="text-sm font-medium">Soyisim</label>
              <Input id="lastName" {...register("lastName")} disabled={!flow} />
              {errors.lastName && (
                <p className="text-xs text-destructive">{errors.lastName.message}</p>
              )}
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="email" className="text-sm font-medium">E-posta</label>
            <Input id="email" type="email" autoComplete="email" {...register("email")} disabled={!flow} />
            {errors.email && (
              <p className="text-xs text-destructive">{errors.email.message}</p>
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="password" className="text-sm font-medium">Şifre</label>
            <Input id="password" type="password" autoComplete="new-password" {...register("password")} disabled={!flow} />
            {errors.password && (
              <p className="text-xs text-destructive">{errors.password.message}</p>
            )}
          </div>
          <Button type="submit" className="mt-2" disabled={isSubmitting || !flow}>
            {!flow ? "Bağlanıyor..." : isSubmitting ? "Hesap oluşturuluyor..." : "Kayıt Ol"}
          </Button>
          <p className="text-center text-xs text-muted-foreground">
            Zaten hesabınız var mı?{" "}
            <Link href="/auth/login" className="text-foreground underline underline-offset-4">
              Giriş Yap
            </Link>
          </p>
        </form>
      </CardContent>
    </Card>
  )
}

export function RegisterView() {
  return (
    <Suspense fallback={<div>Yükleniyor...</div>}>
      <RegisterForm />
    </Suspense>
  )
}
