'use client';

import React, { useState, useEffect } from 'react';
import { Navbar } from '../components/Navbar';
import { HeroBanner } from '../components/HeroBanner';
import { VideoCard } from '../components/VideoCard';
import { VideoPlayerModal } from '../components/VideoPlayerModal';
import { SearchModal } from '../components/SearchModal';
import { AuthModal } from '../components/AuthModal';
import { Video, User } from '../types';
import { Radio, Film, Compass, ShieldCheck, Trophy, Newspaper, Sparkles, Tv } from 'lucide-react';

export default function Home() {
  const [videos, setVideos] = useState<Video[]>([]);
  const [activeVideo, setActiveVideo] = useState<Video | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<string>('All');
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isAuthOpen, setIsAuthOpen] = useState(false);
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Fetch initial videos
  useEffect(() => {
    const fetchVideos = async () => {
      try {
        const res = await fetch('/api/videos');
        if (res.ok) {
          const data = await res.json();
          setVideos(data);
        }
      } catch (err) {
        console.error('Failed to fetch videos:', err);
      } finally {
        setIsLoading(false);
      }
    };

    fetchVideos();
  }, []);

  // Check saved session
  useEffect(() => {
    const token = localStorage.getItem('streamflow_token');
    if (token) {
      fetch('/api/auth/me', {
        headers: { Authorization: `Bearer ${token}` },
      })
        .then((res) => (res.ok ? res.json() : null))
        .then((user) => {
          if (user) setCurrentUser(user);
        })
        .catch(() => {});
    }
  }, []);

  const handleSignOut = () => {
    localStorage.removeItem('streamflow_token');
    setCurrentUser(null);
  };

  // Keyboard shortcut for search
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === '/' && !isSearchOpen && !(e.target instanceof HTMLInputElement)) {
        e.preventDefault();
        setIsSearchOpen(true);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isSearchOpen]);

  // Featured video for Hero
  const featuredVideo = videos.find((v) => v.is_featured === 1) || videos[0] || null;

  // Filtered views
  const filteredVideos =
    selectedCategory === 'All'
      ? videos
      : selectedCategory === 'Live'
      ? videos.filter((v) => v.is_live === 1)
      : videos.filter((v) => v.category.toLowerCase() === selectedCategory.toLowerCase());

  // Categorized sections
  const liveChannels = videos.filter((v) => v.is_live === 1);
  const movies = videos.filter((v) => v.category === 'Movies' || v.category === 'Sci-Fi' || v.category === 'Animation');
  const sports = videos.filter((v) => v.category === 'Sports');
  const news = videos.filter((v) => v.category === 'News');
  const documentaries = videos.filter((v) => v.category === 'Documentary' || v.category === 'Science');
  const entertainment = videos.filter((v) => v.category === 'Entertainment' || v.category === 'Lifestyle');

  return (
    <main className="min-h-screen bg-[#07070a] text-slate-100 flex flex-col justify-between">
      <div>
        {/* Navigation Bar */}
        <Navbar
          onSearchClick={() => setIsSearchOpen(true)}
          onCategorySelect={(cat) => setSelectedCategory(cat)}
          selectedCategory={selectedCategory}
          onAuthClick={() => setIsAuthOpen(true)}
          onSignOut={handleSignOut}
          currentUser={currentUser}
        />

        {/* Hero Banner Spotlight */}
        {selectedCategory === 'All' && (
          <HeroBanner video={featuredVideo} onPlay={(v) => setActiveVideo(v)} />
        )}

        {/* Main Content Sections */}
        <div className={`max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 space-y-12 pb-24 ${selectedCategory !== 'All' ? 'pt-24' : '-mt-10 relative z-20'}`}>
          {selectedCategory !== 'All' ? (
            /* Filtered Category Grid */
            <div className="space-y-6">
              <div className="flex items-center justify-between border-b border-white/10 pb-4">
                <h2 className="text-2xl font-black text-white tracking-tight flex items-center gap-2">
                  <span>{selectedCategory} Streams</span>
                  <span className="text-sm font-semibold px-2.5 py-0.5 rounded-full bg-red-600/20 text-red-400 border border-red-600/30">
                    {filteredVideos.length} Channels Available
                  </span>
                </h2>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-6">
                {filteredVideos.map((video) => (
                  <div key={video.id} className="w-full">
                    <VideoCard video={video} onPlay={(v) => setActiveVideo(v)} />
                  </div>
                ))}
              </div>
            </div>
          ) : (
            /* Netflix-Style Categorized Carousels */
            <>
              {/* Row 1: Live Broadcasts */}
              {liveChannels.length > 0 && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div className="w-2.5 h-2.5 rounded-full bg-red-500 animate-pulse" />
                      <h2 className="text-xl sm:text-2xl font-bold text-white tracking-tight">
                        24/7 Live Broadcasts (1080p HD)
                      </h2>
                    </div>
                    <span className="text-xs text-slate-400 font-medium hidden sm:inline">
                      High-Throughput Akamai & Cloudfront Edge Streams
                    </span>
                  </div>

                  <div className="flex space-x-5 overflow-x-auto pb-4 pt-1 hide-scrollbar -mx-4 px-4 sm:mx-0 sm:px-0">
                    {liveChannels.map((video) => (
                      <VideoCard key={video.id} video={video} onPlay={(v) => setActiveVideo(v)} />
                    ))}
                  </div>
                </div>
              )}

              {/* Row 2: Sports & Action */}
              {sports.length > 0 && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <h2 className="text-xl sm:text-2xl font-bold text-white tracking-tight flex items-center gap-2">
                      <Trophy className="w-5 h-5 text-amber-500" />
                      Live Sports, Motorsports & Extreme
                    </h2>
                  </div>

                  <div className="flex space-x-5 overflow-x-auto pb-4 pt-1 hide-scrollbar -mx-4 px-4 sm:mx-0 sm:px-0">
                    {sports.map((video) => (
                      <VideoCard key={video.id} video={video} onPlay={(v) => setActiveVideo(v)} />
                    ))}
                  </div>
                </div>
              )}

              {/* Row 3: Movies & Cinema VOD */}
              {movies.length > 0 && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <h2 className="text-xl sm:text-2xl font-bold text-white tracking-tight flex items-center gap-2">
                      <Film className="w-5 h-5 text-red-500" />
                      Cinema & Multi-Bitrate VODs (60fps)
                    </h2>
                  </div>

                  <div className="flex space-x-5 overflow-x-auto pb-4 pt-1 hide-scrollbar -mx-4 px-4 sm:mx-0 sm:px-0">
                    {movies.map((video) => (
                      <VideoCard key={video.id} video={video} onPlay={(v) => setActiveVideo(v)} />
                    ))}
                  </div>
                </div>
              )}

              {/* Row 4: News & World Headlines */}
              {news.length > 0 && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <h2 className="text-xl sm:text-2xl font-bold text-white tracking-tight flex items-center gap-2">
                      <Newspaper className="w-5 h-5 text-sky-400" />
                      24/7 International News & Headlines
                    </h2>
                  </div>

                  <div className="flex space-x-5 overflow-x-auto pb-4 pt-1 hide-scrollbar -mx-4 px-4 sm:mx-0 sm:px-0">
                    {news.map((video) => (
                      <VideoCard key={video.id} video={video} onPlay={(v) => setActiveVideo(v)} />
                    ))}
                  </div>
                </div>
              )}

              {/* Row 5: Science & Documentary */}
              {documentaries.length > 0 && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <h2 className="text-xl sm:text-2xl font-bold text-white tracking-tight flex items-center gap-2">
                      <Compass className="w-5 h-5 text-indigo-400" />
                      Space, Science & Documentary
                    </h2>
                  </div>

                  <div className="flex space-x-5 overflow-x-auto pb-4 pt-1 hide-scrollbar -mx-4 px-4 sm:mx-0 sm:px-0">
                    {documentaries.map((video) => (
                      <VideoCard key={video.id} video={video} onPlay={(v) => setActiveVideo(v)} />
                    ))}
                  </div>
                </div>
              )}

              {/* Row 6: Entertainment & Reality */}
              {entertainment.length > 0 && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <h2 className="text-xl sm:text-2xl font-bold text-white tracking-tight flex items-center gap-2">
                      <Tv className="w-5 h-5 text-purple-400" />
                      Entertainment, Comedy & Reality
                    </h2>
                  </div>

                  <div className="flex space-x-5 overflow-x-auto pb-4 pt-1 hide-scrollbar -mx-4 px-4 sm:mx-0 sm:px-0">
                    {entertainment.map((video) => (
                      <VideoCard key={video.id} video={video} onPlay={(v) => setActiveVideo(v)} />
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Footer */}
      <footer className="border-t border-white/10 bg-[#050508] py-8 text-xs text-slate-500">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex flex-col sm:flex-row items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <span className="font-bold text-white tracking-tight">STREAMFLOW</span>
            <span>•</span>
            <span className="flex items-center gap-1 text-emerald-400">
              <ShieldCheck className="w-3.5 h-3.5" /> Netflix/YouTube Grade Protection
            </span>
          </div>
          <div>
            Powered by Rust 1.99, Tokio, Axum, SQLite WAL, and Next.js 14
          </div>
        </div>
      </footer>

      {/* Modals */}
      <VideoPlayerModal video={activeVideo} onClose={() => setActiveVideo(null)} />
      <SearchModal
        isOpen={isSearchOpen}
        onClose={() => setIsSearchOpen(false)}
        onSelectVideo={(v) => setActiveVideo(v)}
      />
      <AuthModal
        isOpen={isAuthOpen}
        onClose={() => setIsAuthOpen(false)}
        onSuccess={(user) => setCurrentUser(user)}
      />
    </main>
  );
}
