import { MailCheck } from "lucide-react"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"

export function VerifyView() {
  return (
    <Card>
      <CardHeader>
        <div className="mb-2 flex size-10 items-center justify-center rounded-full bg-primary/10">
          <MailCheck className="size-5 text-primary" />
        </div>
        <CardTitle>E-postanızı doğrulayın</CardTitle>
        <CardDescription>
          Kayıt sırasında girdiğiniz adrese bir doğrulama bağlantısı gönderdik.
          Lütfen gelen kutunuzu kontrol edin.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">
          E-posta gelmedi mi?{" "}
          <button className="text-foreground underline underline-offset-4">
            Tekrar gönder
          </button>
        </p>
      </CardContent>
    </Card>
  )
}
