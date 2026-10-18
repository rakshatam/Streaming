/** @type {import('next').NextConfig} */
const isStaticExport = process.env.STATIC_EXPORT === 'true';
const backendUrl = process.env.BACKEND_URL || (
  process.env.NODE_ENV === 'development'
    ? 'http://127.0.0.1:8080'
    : 'http://streamflow-backend:8080'
);

const nextConfig = {
  reactStrictMode: true,
  ...(isStaticExport ? { output: 'export' } : {}),
  images: {
    unoptimized: true,
    remotePatterns: [
      {
        protocol: 'https',
        hostname: '**',
      },
      {
        protocol: 'http',
        hostname: '**',
      }
    ],
  },
  basePath: process.env.NEXT_PUBLIC_BASE_PATH || '',
  ...(!isStaticExport ? {
    async rewrites() {
      return [
        {
          source: '/api/:path*',
          destination: `${backendUrl.replace(/\/$/, '')}/api/:path*`,
        },
      ];
    },
  } : {}),
};

export default nextConfig;
