import type { Metadata, Viewport } from "next"
import { GeistSans } from "geist/font/sans"
import { GeistMono } from "geist/font/mono"
import { TooltipProvider } from "@/components/ui/tooltip"
import "./globals.css"

// Fontlar `geist` paketiyle yerelden geliyor. `next/font/google` derleme anında
// Google'dan indirirdi; proje %100 çevrimdışı çalışmak zorunda olduğu için
// ağa bağlı hiçbir varlık kullanılmıyor. Ayrıca paketteki dosyalar tam karakter
// setini içerdiğinden Türkçe ğ/ş/ı/İ karakterleri de doğru render ediliyor —
// eski `subsets: ["latin"]` ayarı bunları kapsamıyordu.

export const metadata: Metadata = {
  title: "Framecut — Video İşleme Stüdyosu",
  description: "Video seçimi, geliştirme ve dışa aktarma için profesyonel çalışma alanı.",
}

export const viewport: Viewport = {
  themeColor: "#070707",
  colorScheme: "dark",
  width: "device-width",
  initialScale: 1,
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="tr" className="bg-background dark">
      <body className={`${GeistSans.variable} ${GeistMono.variable} font-sans antialiased`}>
        <TooltipProvider>{children}</TooltipProvider>
      </body>
    </html>
  )
}
