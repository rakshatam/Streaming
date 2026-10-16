import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'StreamFlow - Ultra High-Performance 1080p Video Streaming',
  description: 'Next-generation video streaming platform powered by Rust, Axum, and low-latency chunked HLS delivery.',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen bg-[#07070a] text-slate-100 antialiased selection:bg-red-600 selection:text-white">
        {children}
      </body>
    </html>
  );
}
