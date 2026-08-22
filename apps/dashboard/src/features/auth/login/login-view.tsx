"use client"

import Link from "next/link"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

const schema = z.object({
  email: z.string().email("Geçerli bir e-posta girin"),
  password: z.string().min(8, "Şifre en az 8 karakter olmalı"),
})

type FormValues = z.infer<typeof schema>

export function LoginView() {
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) })

  async function onSubmit(values: FormValues) {
    // Kratos Login Flow — Gateway üzerinden /api/auth/self-service/login endpoint'ine gider
    const res = await fetch("/api/auth/self-service/login?flow=password", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        identifier: values.email,
        password: values.password,
        method: "password",
      }),
      credentials: "include",
    })

    if (res.ok) {
      window.location.href = "/"
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
              {...register("password")}
            />
            {errors.password && (
              <p className="text-xs text-destructive">{errors.password.message}</p>
            )}
          </div>
          <Button type="submit" className="mt-2" disabled={isSubmitting}>
            {isSubmitting ? "Giriş yapılıyor..." : "Giriş Yap"}
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
