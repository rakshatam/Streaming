import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'StreamFlow - Video Streaming',
  description: 'Video streaming platform built with Rust, Axum, and Next.js.',
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
