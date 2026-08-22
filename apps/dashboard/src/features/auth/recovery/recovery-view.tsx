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
})

type FormValues = z.infer<typeof schema>

export function RecoveryView() {
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting, isSubmitSuccessful },
  } = useForm<FormValues>({ resolver: zodResolver(schema) })

  async function onSubmit(values: FormValues) {
    // Kratos Recovery Flow — Gateway üzerinden
    await fetch("/api/auth/self-service/recovery?flow=code", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: values.email, method: "code" }),
      credentials: "include",
    })
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
            <Input id="email" type="email" autoComplete="email" {...register("email")} />
            {errors.email && (
              <p className="text-xs text-destructive">{errors.email.message}</p>
            )}
          </div>
          <Button type="submit" disabled={isSubmitting}>
            {isSubmitting ? "Gönderiliyor..." : "Bağlantı Gönder"}
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
