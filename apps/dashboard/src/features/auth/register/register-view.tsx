"use client"

import Link from "next/link"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
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

export function RegisterView() {
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) })

  async function onSubmit(values: FormValues) {
    // Kratos Registration Flow — Gateway üzerinden
    const res = await fetch("/api/auth/self-service/registration?flow=password", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        traits: {
          email: values.email,
          name: { first: values.firstName, last: values.lastName },
        },
        password: values.password,
        method: "password",
      }),
      credentials: "include",
    })

    if (res.ok) {
      window.location.href = "/auth/verify"
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
              <Input id="firstName" {...register("firstName")} />
              {errors.firstName && (
                <p className="text-xs text-destructive">{errors.firstName.message}</p>
              )}
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="lastName" className="text-sm font-medium">Soyisim</label>
              <Input id="lastName" {...register("lastName")} />
              {errors.lastName && (
                <p className="text-xs text-destructive">{errors.lastName.message}</p>
              )}
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="email" className="text-sm font-medium">E-posta</label>
            <Input id="email" type="email" autoComplete="email" {...register("email")} />
            {errors.email && (
              <p className="text-xs text-destructive">{errors.email.message}</p>
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="password" className="text-sm font-medium">Şifre</label>
            <Input id="password" type="password" autoComplete="new-password" {...register("password")} />
            {errors.password && (
              <p className="text-xs text-destructive">{errors.password.message}</p>
            )}
          </div>
          <Button type="submit" className="mt-2" disabled={isSubmitting}>
            {isSubmitting ? "Hesap oluşturuluyor..." : "Kayıt Ol"}
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
