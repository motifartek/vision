/** @type {import('next').NextConfig} */
const nextConfig = {
  // `ignoreBuildErrors` açıktı: tip hataları derlemeyi durdurmuyordu. Kod tabanı
  // şu an `tsc --noEmit` ile temiz, yani kapatmak için doğru an — açık kalsaydı
  // ilerideki her tip hatası sessizce üretime kadar giderdi.
  images: {
    unoptimized: true,
    remotePatterns: [
      {
        protocol: "https",
        hostname: "images.unsplash.com",
      },
    ],
  },
  allowedDevOrigins: ["127.0.0.1"],
  experimental: {
    // Rewrite (proxy) uzerinden gecen istek govdesi varsayilan olarak 10MB'da
    // kesiliyor; video yuklemede govde yarida kalinca inference servisi
    // multipart'i okuyamiyor ve baglanti kopuyor (ECONNRESET).
    proxyClientMaxBodySize: '512mb',
    // Buyuk dosyalarin diske yazilmasi varsayilan 30sn'yi asabiliyor.
    proxyTimeout: 30 * 60 * 1000,
  },
  async rewrites() {
    const gatewayUrl = process.env.GATEWAY_URL ?? "http://127.0.0.1:8000/api/auth"
    return {
      beforeFiles: [
        {
          source: "/api/auth/:path*",
          destination: `${gatewayUrl}/:path*`,
        },
      ],
      fallback: [
        {
          source: '/api/inference/:path*',
          destination: 'http://127.0.0.1:8081/:path*',
        },
      ]
    }
  },
}

export default nextConfig
