'use client';

import React from 'react';
import { Play, Radio, Sparkles } from 'lucide-react';
import { Video } from '../types';

interface HeroBannerProps {
  video: Video | null;
  onPlay: (video: Video) => void;
}

export const HeroBanner: React.FC<HeroBannerProps> = ({ video, onPlay }) => {
  if (!video) return null;

  return (
    <div className="relative w-full h-[70vh] min-h-[500px] max-h-[750px] flex items-end pb-16 overflow-hidden">
      {/* Background Backdrop Image */}
      <div
        className="absolute inset-0 bg-cover bg-center transition-transform duration-1000 scale-105"
        style={{ backgroundImage: `url(${video.backdrop_url})` }}
      >
        {/* Cinematic Vignettes */}
        <div className="absolute inset-0 bg-gradient-to-t from-[#07070a] via-[#07070a]/60 to-transparent" />
        <div className="absolute inset-0 bg-gradient-to-r from-[#07070a] via-[#07070a]/80 to-transparent w-full md:w-3/4" />
        <div className="absolute inset-0 bg-black/20 backdrop-blur-[1px]" />
      </div>

      {/* Hero Content */}
      <div className="relative max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 w-full z-10">
        <div className="max-w-2xl space-y-4">
          {/* Metadata Badges */}
          <div className="flex flex-wrap items-center gap-2.5">
            {video.is_live === 1 ? (
              <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-red-600/90 text-white text-xs font-bold uppercase tracking-wider shadow-lg shadow-red-600/30">
                <Radio className="w-3.5 h-3.5 animate-pulse" />
                Live
              </span>
            ) : (
              <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-indigo-600/90 text-white text-xs font-bold uppercase tracking-wider">
                <Sparkles className="w-3.5 h-3.5" />
                Featured
              </span>
            )}

            <span className="px-2.5 py-0.5 rounded-md bg-white/10 backdrop-blur-md text-white text-xs font-semibold border border-white/10">
              {video.quality}
            </span>

            <span className="px-2.5 py-0.5 rounded-md bg-white/10 backdrop-blur-md text-amber-300 text-xs font-bold border border-white/10">
              ★ {video.rating}
            </span>

            <span className="px-2.5 py-0.5 rounded-md bg-white/10 backdrop-blur-md text-slate-300 text-xs border border-white/10">
              {video.category}
            </span>
          </div>

          {/* Title */}
          <h1 className="text-3xl sm:text-5xl lg:text-6xl font-black text-white tracking-tight leading-none drop-shadow-md">
            {video.title}
          </h1>

          {/* Description */}
          <p className="text-slate-300 text-sm sm:text-base line-clamp-3 leading-relaxed drop-shadow">
            {video.description}
          </p>

          {/* Action Buttons */}
          <div className="flex items-center space-x-4 pt-2">
            <button
              onClick={() => onPlay(video)}
              className="flex items-center space-x-3 px-6 py-3.5 rounded-xl bg-red-600 hover:bg-red-700 text-white font-bold text-base transition-all duration-200 shadow-xl shadow-red-600/40 hover:scale-105 active:scale-95"
            >
              <Play className="w-5 h-5 fill-current" />
              <span>Watch Now</span>
            </button>

          </div>
        </div>
      </div>
    </div>
  );
};
