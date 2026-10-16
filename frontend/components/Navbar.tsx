'use client';

import React, { useState, useEffect } from 'react';
import { Search, Shield, Zap, User as UserIcon, Activity, Radio, Film, LogOut } from 'lucide-react';
import { StreamMetrics, User } from '../types';

interface NavbarProps {
  onSearchClick: () => void;
  onCategorySelect: (category: string) => void;
  selectedCategory: string;
  onAuthClick: () => void;
  onSignOut: () => void;
  currentUser: User | null;
}

export const Navbar: React.FC<NavbarProps> = ({
  onSearchClick,
  onCategorySelect,
  selectedCategory,
  onAuthClick,
  onSignOut,
  currentUser,
}) => {
  const [scrolled, setScrolled] = useState(false);
  const [metrics, setMetrics] = useState<StreamMetrics | null>(null);

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 40);
    };
    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  useEffect(() => {
    const fetchMetrics = async () => {
      try {
        const res = await fetch('/api/metrics');
        if (res.ok) {
          const data = await res.json();
          setMetrics(data);
        }
      } catch {
        // Rust backend may be spinning up
      }
    };

    fetchMetrics();
    const interval = setInterval(fetchMetrics, 3000);
    return () => clearInterval(interval);
  }, []);

  const navCategories = ['All', 'Live', 'Movies', 'Sports', 'News', 'Sci-Fi', 'Documentary', 'Entertainment'];

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-40 transition-all duration-300 ${
        scrolled
          ? 'bg-[#08080c]/90 backdrop-blur-md border-b border-white/5 py-3 shadow-2xl'
          : 'bg-gradient-to-b from-[#08080c]/90 via-[#08080c]/40 to-transparent py-5'
      }`}
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex items-center justify-between">
        {/* Brand Logo */}
        <div className="flex items-center space-x-8">
          <div
            onClick={() => onCategorySelect('All')}
            className="flex items-center space-x-2.5 cursor-pointer group"
          >
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-red-600 to-rose-500 flex items-center justify-center shadow-lg shadow-red-600/30 group-hover:scale-105 transition-transform">
              <Zap className="w-5 h-5 text-white fill-current" />
            </div>
            <div className="flex flex-col">
              <span className="text-xl font-extrabold tracking-tight text-white flex items-center gap-1">
                STREAM<span className="text-red-500">FLOW</span>
              </span>
              <span className="text-[10px] tracking-widest uppercase text-slate-400 font-semibold -mt-1">
                Rust Edge Engine
              </span>
            </div>
          </div>

          {/* Navigation Links */}
          <nav className="hidden md:flex items-center space-x-1">
            {navCategories.map((cat) => (
              <button
                key={cat}
                onClick={() => onCategorySelect(cat)}
                className={`px-3.5 py-1.5 rounded-lg text-sm font-medium transition-colors ${
                  selectedCategory === cat
                    ? 'bg-white/10 text-white font-semibold shadow-sm'
                    : 'text-slate-400 hover:text-white hover:bg-white/5'
                }`}
              >
                {cat === 'Live' ? (
                  <span className="flex items-center gap-1.5">
                    <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                    Live TV
                  </span>
                ) : (
                  cat
                )}
              </button>
            ))}
          </nav>
        </div>

        {/* Right Actions */}
        <div className="flex items-center space-x-3 sm:space-x-4">
          {/* Live Rust Engine Metrics Pill */}
          {metrics && (
            <div className="hidden lg:flex items-center space-x-3 px-3 py-1.5 rounded-full bg-white/5 border border-white/10 text-xs text-slate-300">
              <div className="flex items-center gap-1.5">
                <span className="relative flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
                </span>
                <span className="font-mono text-emerald-400 font-medium">Rust 0-Copy</span>
              </div>
              <span className="text-slate-600">|</span>
              <div>
                Hit Ratio: <span className="text-white font-semibold">{metrics.cache_hit_ratio_percent}%</span>
              </div>
              <span className="text-slate-600">|</span>
              <div>
                Streamed: <span className="text-white font-semibold">{metrics.megabytes_streamed} MB</span>
              </div>
            </div>
          )}

          {/* Search Trigger */}
          <button
            onClick={onSearchClick}
            className="flex items-center space-x-2 px-3.5 py-1.5 rounded-xl bg-white/5 hover:bg-white/10 border border-white/5 text-slate-400 hover:text-white text-sm transition-all"
            title="Search channels and movies"
          >
            <Search className="w-4 h-4" />
            <span className="hidden sm:inline">Search...</span>
            <kbd className="hidden sm:inline-block px-1.5 py-0.5 text-[10px] font-mono text-slate-400 bg-white/5 rounded border border-white/10">
              /
            </kbd>
          </button>

          {/* User Profile & Sign Out Actions */}
          {currentUser ? (
            <div className="flex items-center space-x-2">
              <div className="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-white/10 border border-white/10 text-white text-xs font-semibold">
                <UserIcon className="w-3.5 h-3.5 text-red-500" />
                <span className="max-w-[120px] truncate">{currentUser.username}</span>
              </div>
              <button
                onClick={onSignOut}
                className="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-white/5 hover:bg-red-600/20 text-slate-400 hover:text-red-400 border border-white/10 hover:border-red-500/30 text-xs font-medium transition-all active:scale-95"
                title="Sign out of your account"
              >
                <LogOut className="w-3.5 h-3.5" />
                <span className="hidden sm:inline">Sign Out</span>
              </button>
            </div>
          ) : (
            <button
              onClick={onAuthClick}
              className="flex items-center space-x-2 px-3.5 py-1.5 rounded-xl bg-red-600 hover:bg-red-700 text-white text-sm font-medium transition-all shadow-lg shadow-red-600/20 active:scale-95"
            >
              <UserIcon className="w-4 h-4" />
              <span>Sign In</span>
            </button>
          )}
        </div>
      </div>
    </header>
  );
};
