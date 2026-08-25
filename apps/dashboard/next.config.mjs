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
    const gatewayBaseUrl = process.env.GATEWAY_BASE_URL ?? "http://127.0.0.1:8000"
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
          destination: 'http://127.0.0.1:8081/:path*',
        },
      ]
    }
  },
}

export default nextConfig