/**
 * API configuration helper for StreamFlow.
 * Supports:
 * 1. Local development / Docker: defaults to relative '/api/...' via Next.js rewrites.
 * 2. Static deployment on GitHub Pages: uses NEXT_PUBLIC_API_URL (e.g., 'https://streamflow-backend.onrender.com').
 * 3. Client runtime override via localStorage ('streamflow_api_url').
 */

export const getApiBaseUrl = (): string => {
  if (process.env.NEXT_PUBLIC_API_URL) {
    return process.env.NEXT_PUBLIC_API_URL.replace(/\/$/, '');
  }
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('streamflow_api_url');
    if (saved) {
      return saved.replace(/\/$/, '');
    }
  }
  return '';
};

export function apiUrl(path: string): string {
  const base = getApiBaseUrl();
  const cleanPath = path.startsWith('/') ? path : `/${path}`;
  return base ? `${base}${cleanPath}` : cleanPath;
}
