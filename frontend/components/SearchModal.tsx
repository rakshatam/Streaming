'use client';

import React, { useState, useEffect, useRef } from 'react';
import { Search, X, Play, Radio, Star, Film } from 'lucide-react';
import { Video } from '../types';
import { apiUrl } from '../lib/api';

interface SearchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectVideo: (video: Video) => void;
}

export const SearchModal: React.FC<SearchModalProps> = ({
  isOpen,
  onClose,
  onSelectVideo,
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Video[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => inputRef.current?.focus(), 100);
    } else {
      setQuery('');
      setResults([]);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      setIsLoading(true);
      try {
        const res = await fetch(apiUrl(`/api/videos/search?q=${encodeURIComponent(query)}`));
        if (res.ok) {
          const data = await res.json();
          setResults(data);
        }
      } catch (e) {
        console.error('Search failed:', e);
      } finally {
        setIsLoading(false);
      }
    }, 250);

    return () => clearTimeout(timer);
  }, [query]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-20 px-4 bg-black/80 backdrop-blur-xl animate-in fade-in duration-200">
      <div className="relative w-full max-w-2xl rounded-3xl bg-[#0e0e16] border border-white/10 shadow-2xl overflow-hidden">
        {/* Search Input Bar */}
        <div className="flex items-center px-5 py-4 border-b border-white/10">
          <Search className="w-5 h-5 text-slate-400 mr-3 flex-shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search 1080p movies, live channels, documentaries, or tags..."
            className="w-full bg-transparent text-white placeholder-slate-500 text-base focus:outline-none"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              className="p-1 rounded-full text-slate-400 hover:text-white mr-2"
            >
              <X className="w-4 h-4" />
            </button>
          )}
          <button
            onClick={onClose}
            className="px-2.5 py-1 rounded-lg text-xs font-semibold bg-white/10 hover:bg-white/20 text-slate-300 transition-colors"
          >
            ESC
          </button>
        </div>

        {/* Results Container */}
        <div className="max-h-[60vh] overflow-y-auto p-4 space-y-2">
          {isLoading ? (
            <div className="py-12 flex flex-col items-center justify-center space-y-3">
              <div className="w-6 h-6 border-2 border-red-500 border-t-transparent rounded-full animate-spin" />
              <span className="text-xs text-slate-400 font-mono">Querying Rust FTS5 Index...</span>
            </div>
          ) : results.length > 0 ? (
            results.map((video) => (
              <div
                key={video.id}
                onClick={() => {
                  onSelectVideo(video);
                  onClose();
                }}
                className="group flex items-center space-x-4 p-3 rounded-2xl hover:bg-white/5 cursor-pointer border border-transparent hover:border-white/10 transition-all"
              >
                <div className="relative w-28 aspect-video rounded-xl overflow-hidden bg-slate-800 flex-shrink-0">
                  <img
                    src={video.thumbnail_url}
                    alt={video.title}
                    className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                  />
                  <div className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity">
                    <Play className="w-4 h-4 text-white fill-current" />
                  </div>
                  {video.is_live === 1 && (
                    <span className="absolute top-1 left-1 px-1.5 py-0.5 rounded bg-red-600 text-white text-[8px] font-bold">
                      LIVE
                    </span>
                  )}
                </div>

                <div className="flex-1 min-w-0">
                  <h4 className="text-sm font-semibold text-white group-hover:text-red-400 transition-colors line-clamp-1">
                    {video.title}
                  </h4>
                  <p className="text-xs text-slate-400 line-clamp-1 mt-0.5">
                    {video.description}
                  </p>
                  <div className="flex items-center gap-2 mt-1 text-[11px] text-slate-500">
                    <span className="text-emerald-400 font-medium">{video.quality}</span>
                    <span>•</span>
                    <span>{video.category}</span>
                    <span>•</span>
                    <span className="flex items-center text-amber-400 gap-0.5">
                      <Star className="w-3 h-3 fill-current" /> {video.rating}
                    </span>
                  </div>
                </div>
              </div>
            ))
          ) : query ? (
            <div className="py-12 text-center text-slate-500 text-sm">
              No streams found matching "{query}".
            </div>
          ) : (
            <div className="py-8 text-center text-slate-500 text-xs">
              Type keywords like "NASA", "Sci-Fi", "Action", or "1080p" to search.
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
