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
  const [searchError, setSearchError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => inputRef.current?.focus(), 100);
    } else {
      setQuery('');
      setResults([]);
      setSearchError(null);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      setIsLoading(false);
      setSearchError(null);
      return;
    }

    setResults([]);
    setIsLoading(true);
    setSearchError(null);
    if (new TextEncoder().encode(query.trim()).length > 100) {
      setIsLoading(false);
      setSearchError('Search terms must be 100 bytes or fewer.');
      return;
    }
    const controller = new AbortController();
    const timer = setTimeout(async () => {
      try {
        const res = await fetch(
          apiUrl(`/api/videos/search?q=${encodeURIComponent(query.trim())}`),
          { signal: controller.signal },
        );
        if (!res.ok) throw new Error(`Search returned status ${res.status}`);
        const data: unknown = await res.json();
        if (!Array.isArray(data)) throw new Error('Search returned an invalid response');
        if (!controller.signal.aborted) setResults(data as Video[]);
      } catch (e) {
        if (!(e instanceof DOMException && e.name === 'AbortError')) {
          console.error('Search failed:', e);
          if (!controller.signal.aborted) {
            setResults([]);
            setSearchError('Search is temporarily unavailable. Please try again.');
          }
        }
      } finally {
        if (!controller.signal.aborted) setIsLoading(false);
      }
    }, 250);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-20 px-4 bg-black/80 backdrop-blur-xl animate-in fade-in duration-200">
      <section role="dialog" aria-modal="true" aria-label="Search videos" className="relative w-full max-w-2xl">
        <div className="rounded-3xl bg-[#0e0e16] border border-white/10 shadow-2xl overflow-hidden">
          {/* Search Input Bar */}
          <div className="flex items-center px-5 py-4 border-b border-white/10">
            <Search className="w-5 h-5 text-slate-400 mr-3 flex-shrink-0" />
            <input
              ref={inputRef}
              type="text"
              aria-label="Search videos"
              maxLength={100}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search 1080p movies, live channels, documentaries, or tags..."
              className="w-full bg-transparent text-white placeholder-slate-500 text-base focus:outline-none"
            />
            {query && (
              <button
                onClick={() => setQuery('')}
                aria-label="Clear search"
                className="p-1 rounded-full text-slate-400 hover:text-white mr-2"
              >
                <X className="w-4 h-4" />
              </button>
            )}
            <button
              type="button"
              onClick={onClose}
              aria-label="Close search"
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
                <button
                  type="button"
                  key={video.id}
                  onClick={() => {
                    onSelectVideo(video);
                    onClose();
                  }}
                  aria-label={`Play ${video.title}`}
                  className="group w-full text-left flex items-center space-x-4 p-3 rounded-2xl hover:bg-white/5 border border-transparent hover:border-white/10 transition-all"
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
                </button>
              ))
            ) : searchError ? (
              <div role="alert" className="py-12 text-center text-slate-400 text-sm">
                {searchError}
              </div>
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
      </section>
    </div>
  );
};
