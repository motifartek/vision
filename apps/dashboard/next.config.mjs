/** @type {import('next').NextConfig} */
const nextConfig = {
  // `ignoreBuildErrors` açıktı: tip hataları derlemeyi durdurmuyordu. Kod tabanı
  // şu an `tsc --noEmit` ile temiz, yani kapatmak için doğru an — açık kalsaydı
  // ilerideki her tip hatası sessizce üretime kadar giderdi.
  images: {
    unoptimized: true,
  },
  experimental: {
    // Rewrite (proxy) uzerinden gecen istek govdesi varsayilan olarak 10MB'da
    // kesiliyor; video yuklemede govde yarida kalinca inference servisi
    // multipart'i okuyamiyor ve baglanti kopuyor (ECONNRESET).
    proxyClientMaxBodySize: '512mb',
    // Buyuk dosyalarin diske yazilmasi varsayilan 30sn'yi asabiliyor.
    proxyTimeout: 30 * 60 * 1000,
  },
  async rewrites() {
    return [
      {
        source: '/api/inference/:path*',
        destination: 'http://127.0.0.1:8081/:path*',
      },
    ]
  },
}

export default nextConfig
