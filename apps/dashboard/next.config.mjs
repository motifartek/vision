/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    unoptimized: true,
  },
  async rewrites() {
    const gatewayUrl = process.env.GATEWAY_URL ?? "http://127.0.0.1:4433"
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
