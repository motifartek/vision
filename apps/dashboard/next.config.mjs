/** @type {import('next').NextConfig} */
const nextConfig = {
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
  async rewrites() {
    // Tüm /api/auth/* istekleri Gateway'e yönlendirilir (Port 8000)
    const gatewayUrl = process.env.GATEWAY_URL ?? "http://127.0.0.1:8000/api/auth"
    return {
      beforeFiles: [
        // Tüm /api/auth/* istekleri Gateway'e (şimdilik doğrudan Kratos'a) yönlendirilir.
        // Gateway Rust'a taşındığında sadece GATEWAY_URL env değişkeni güncellenir.
        {
          source: "/api/auth/:path*",
          destination: `${gatewayUrl}/:path*`,
        },
      ],
    }
  },
}

export default nextConfig
