export interface Video {
  id: string;
  title: string;
  description: string;
  category: string;
  quality: string;
  stream_url: string;
  thumbnail_url: string;
  backdrop_url: string;
  duration_secs: number;
  is_live: number;
  is_featured: number;
  views_count: number;
  rating: number;
  tags: string;
  created_at: number;
}

export interface User {
  id: string;
  username: string;
  email: string;
  role: string;
  created_at: number;
}

export interface StreamMetrics {
  total_requests: number;
  cache_hits: number;
  cache_misses: number;
  cache_hit_ratio_percent: number;
  bytes_streamed: number;
  megabytes_streamed: number;
}

export interface HistoryItem {
  video_id: string;
  progress_secs: number;
  updated_at: number;
  title: string;
  thumbnail_url: string;
  duration_secs: number;
  category: string;
  is_live: number;
}
