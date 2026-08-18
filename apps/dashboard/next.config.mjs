/** @type {import('next').NextConfig} */
const nextConfig = {
  typescript: {
    ignoreBuildErrors: true,
  },
  images: {
    unoptimized: true,
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
