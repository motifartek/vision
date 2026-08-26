import Link from "next/link"
import { Gauge } from "lucide-react"

export default function AuthLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center bg-background p-4">
      <div className="mb-8 flex flex-col items-center gap-3">
        <Link
          href="/"
          className="flex size-10 items-center justify-center rounded-xl bg-primary text-primary-foreground"
          aria-label="MotifAI ana sayfa"
        >
          <Gauge className="size-5" />
        </Link>
        <p className="text-sm text-muted-foreground">MotifAI</p>
      </div>
      <div className="w-full max-w-sm">{children}</div>
    </div>
  )
}
