'use client';

import React from 'react';
import { Play, Radio, Star, Eye } from 'lucide-react';
import { Video } from '../types';

interface VideoCardProps {
  video: Video;
  onPlay: (video: Video) => void;
}

export const VideoCard: React.FC<VideoCardProps> = ({ video, onPlay }) => {
  const formatDuration = (secs: number) => {
    if (secs <= 0) return 'Live';
    const mins = Math.floor(secs / 60);
    const remSecs = secs % 60;
    return `${mins}:${remSecs < 10 ? '0' : ''}${remSecs}`;
  };

  const formatViews = (views: number) => {
    if (views >= 1000000) return `${(views / 1000000).toFixed(1)}M`;
    if (views >= 1000) return `${(views / 1000).toFixed(1)}k`;
    return views.toString();
  };

  return (
    <div
      onClick={() => onPlay(video)}
      className="group relative flex-shrink-0 w-64 sm:w-72 cursor-pointer transition-all duration-300"
    >
      {/* Thumbnail Container */}
      <div className="relative aspect-video w-full rounded-2xl overflow-hidden bg-slate-900 border border-white/5 group-hover:border-red-500/50 shadow-md group-hover:shadow-2xl group-hover:shadow-red-600/20 group-hover:-translate-y-1.5 transition-all duration-300">
        <img
          src={video.thumbnail_url}
          alt={video.title}
          className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
          loading="lazy"
        />

        {/* Gradient Overlay */}
        <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent opacity-60 group-hover:opacity-40 transition-opacity" />

        {/* Live Badge or Duration */}
        <div className="absolute top-2.5 left-2.5 flex items-center gap-1.5">
          {video.is_live === 1 ? (
            <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-red-600/90 text-white text-[10px] font-bold uppercase tracking-wider shadow">
              <Radio className="w-3 h-3 animate-pulse" />
              LIVE
            </span>
          ) : (
            <span className="px-2 py-0.5 rounded-md bg-black/60 backdrop-blur-md text-white text-[10px] font-semibold">
              {formatDuration(video.duration_secs)}
            </span>
          )}

          <span className="px-1.5 py-0.5 rounded bg-black/60 backdrop-blur-md text-emerald-400 text-[10px] font-bold border border-emerald-400/20">
            {video.quality}
          </span>
        </div>

        {/* Hover Play Button */}
        <div className="absolute inset-0 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity duration-200">
          <div className="w-12 h-12 rounded-full bg-red-600 text-white flex items-center justify-center shadow-lg shadow-red-600/50 transform scale-75 group-hover:scale-100 transition-transform">
            <Play className="w-5 h-5 fill-current ml-0.5" />
          </div>
        </div>

        {/* View Count */}
        <div className="absolute bottom-2.5 right-2.5 flex items-center gap-1 px-1.5 py-0.5 rounded bg-black/60 backdrop-blur-md text-slate-300 text-[10px]">
          <Eye className="w-3 h-3" />
          <span>{formatViews(video.views_count)}</span>
        </div>
      </div>

      {/* Video Details */}
      <div className="mt-2.5 space-y-1">
        <h3 className="text-sm font-semibold text-white group-hover:text-red-400 transition-colors line-clamp-1">
          {video.title}
        </h3>

        <div className="flex items-center justify-between text-xs text-slate-400">
          <span className="hover:text-slate-300">{video.category}</span>
          <div className="flex items-center gap-1 text-amber-300">
            <Star className="w-3 h-3 fill-current" />
            <span className="font-semibold text-[11px]">{video.rating}</span>
          </div>
        </div>
      </div>
    </div>
  );
};
