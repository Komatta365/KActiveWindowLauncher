/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'export',
  devIndicators: false,
  trailingSlash: true,
  images: {
    unoptimized: true,
  },
};

export default nextConfig;
