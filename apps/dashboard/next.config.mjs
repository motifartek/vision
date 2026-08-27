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
  experimental: {
    proxyClientMaxBodySize: '512mb',
    proxyTimeout: 30 * 60 * 1000,
  },
  async rewrites() {
    const gatewayUrl = process.env.GATEWAY_URL ?? "http://127.0.0.1:8000/api/auth"
<<<<<<< HEAD
    const gatewayBaseUrl = process.env.GATEWAY_BASE_URL ?? "http://127.0.0.1:8000"
=======
    const inferenceUrl = process.env.INFERENCE_URL ?? "http://127.0.0.1:8081"
    const streamUrl = process.env.STREAM_URL ?? "http://127.0.0.1:8100"
    const visionUrl = process.env.VISION_URL ?? "http://127.0.0.1:8110"
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
    return {
      beforeFiles: [
        {
          source: "/api/auth/:path*",
          destination: `${gatewayUrl}/:path*`,
        },
        {
          source: '/api/stream/:path*',
          destination: `${gatewayBaseUrl}/api/stream/:path*`,
        },
      ],
      fallback: [
        {
          source: '/api/inference/:path*',
          destination: `${inferenceUrl}/:path*`,
        },
        // Video alımı, klip üretimi ve hareket profili.
        {
          source: '/api/stream/:path*',
          destination: `${streamUrl}/:path*`,
        },
        // Analiz ajanı: şartname raporunu üreten servis.
        {
          source: '/api/vision/:path*',
          destination: `${visionUrl}/:path*`,
        },
      ]
    }
  },
}

export default nextConfig