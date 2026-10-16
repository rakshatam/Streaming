'use client';

import React, { useEffect, useRef, useState, useCallback } from 'react';
import Hls from 'hls.js';
import {
  Play,
  Pause,
  Volume2,
  VolumeX,
  Maximize,
  Minimize,
  Settings,
  Shield,
  Activity,
  X,
  Radio,
  Sliders,
  Check,
  Heart,
  Star,
  Trash2,
} from 'lucide-react';
import { Video } from '../types';

interface QualityLevel {
  id: number;
  width: number;
  height: number;
  bitrate: number;
  label: string;
}

function formatQualityLabel(width?: number, height?: number): string {
  const w = width || 0;
  const h = height || 0;

  // 4K (standard 3840x2160 or cinematic widescreen >=3200x1350)
  if (w >= 3200 || h >= 2000) return '4K';

  // 1440p (standard 2560x1440 or widescreen 2560x1080 / 2576x1150)
  if (w >= 2500 || h >= 1400 || (h > 1080 && w >= 2000)) return '1440p';

  // 1080p (standard 1920x1080 or widescreen 1920x800 / 1680x750)
  if (w >= 1600 || h >= 1000 || (w >= 1600 && h >= 720)) return '1080p';

  // 720p (standard 1280x720 or widescreen 1280x544)
  if (w >= 1200 || h >= 700) return '720p';

  // 480p
  if (h >= 480) return '480p';

  // 360p
  if (h >= 360) return '360p';

  if (h > 0) return `${h}p`;

  return '1080p';
}

interface VideoPlayerModalProps {
  video: Video | null;
  onClose: () => void;
}

export const VideoPlayerModal: React.FC<VideoPlayerModalProps> = ({ video, onClose }) => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const hlsRef = useRef<Hls | null>(null);

  const [isPlaying, setIsPlaying] = useState(false);
  const [isMuted, setIsMuted] = useState(false);
  const [volume, setVolume] = useState(1);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [buffered, setBuffered] = useState(0);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [showControls, setShowControls] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  const [showDiagnostics, setShowDiagnostics] = useState(false);

  // User Rating & Like State
  const [likesCount, setLikesCount] = useState<number>(0);
  const [hasLiked, setHasLiked] = useState<boolean>(false);
  const [userRating, setUserRating] = useState<number | null>(null);
  const [avgRating, setAvgRating] = useState<number>(video?.rating || 4.8);
  const [totalRatings, setTotalRatings] = useState<number>(0);
  const [hoverRating, setHoverRating] = useState<number | null>(null);
  const [interactionToast, setInteractionToast] = useState<string | null>(null);

  // Stream Quality Ladder
  const [qualities, setQualities] = useState<QualityLevel[]>([]);
  const [selectedQuality, setSelectedQuality] = useState<number>(-1); // -1 = Auto
  const [currentQualityLabel, setCurrentQualityLabel] = useState<string>('Auto');
  const [playbackSpeed, setPlaybackSpeed] = useState<number>(1);

  // Diagnostics HUD Telemetry
  const [hudStats, setHudStats] = useState({
    resolution: '1920x1080',
    bandwidth: 0,
    bufferAheadSecs: 0,
    droppedFrames: 0,
  });

  const controlsTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const handleMouseMove = () => {
    setShowControls(true);
    if (controlsTimeoutRef.current) clearTimeout(controlsTimeoutRef.current);
    controlsTimeoutRef.current = setTimeout(() => {
      if (isPlaying) setShowControls(false);
    }, 3500);
  };

  // Fetch initial interactions (likes & ratings)
  useEffect(() => {
    if (!video) return;
    const token = localStorage.getItem('streamflow_token');
    const headers: HeadersInit = token ? { Authorization: `Bearer ${token}` } : {};

    fetch(`/api/videos/${video.id}/interactions`, { headers })
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data) {
          setLikesCount(data.likes_count);
          setHasLiked(data.has_liked);
          setUserRating(data.user_rating);
          setAvgRating(data.average_rating);
          setTotalRatings(data.total_ratings);
        }
      })
      .catch(() => {});
  }, [video]);

  // Handle Like Toggle
  const handleToggleLike = async () => {
    if (!video) return;
    const token = localStorage.getItem('streamflow_token');
    if (!token) {
      setInteractionToast('Please sign in to like this video');
      setTimeout(() => setInteractionToast(null), 3500);
      return;
    }

    try {
      const res = await fetch(`/api/videos/${video.id}/like`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setHasLiked(data.has_liked);
        setLikesCount(data.likes_count);
        setInteractionToast(data.has_liked ? 'Added to your liked videos!' : 'Removed like');
        setTimeout(() => setInteractionToast(null), 2500);
      }
    } catch (e) {
      console.error('Like failed', e);
    }
  };

  // Handle 1-5 Star Rating
  const handleRate = async (ratingVal: number) => {
    if (!video) return;
    const token = localStorage.getItem('streamflow_token');
    if (!token) {
      setInteractionToast('Please sign in to submit a rating');
      setTimeout(() => setInteractionToast(null), 3500);
      return;
    }

    try {
      const res = await fetch(`/api/videos/${video.id}/rate`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ rating: ratingVal }),
      });
      if (res.ok) {
        const data = await res.json();
        setUserRating(data.user_rating);
        setAvgRating(data.average_rating);
        setTotalRatings(data.total_ratings);
        setInteractionToast(`Rated ${ratingVal} ★! Thanks for your feedback.`);
        setTimeout(() => setInteractionToast(null), 2500);
      }
    } catch (e) {
      console.error('Rating failed', e);
    }
  };

  const [clearingCache, setClearingCache] = useState(false);
  const handleClearCache = async () => {
    setClearingCache(true);
    try {
      const res = await fetch('/api/cache/clear', { method: 'POST' });
      if (res.ok) {
        setInteractionToast('Stream cache cleared successfully');
      } else {
        setInteractionToast('Failed to clear cache');
      }
    } catch {
      setInteractionToast('Cache clear failed');
    } finally {
      setClearingCache(false);
      setTimeout(() => setInteractionToast(null), 2500);
    }
  };

  // Initialize Hls.js & Load Rewritten Stream from Rust Backend
  useEffect(() => {
    if (!video || !videoRef.current) return;

    const streamManifestUrl = `/api/stream/manifest/${video.id}`;
    const videoEl = videoRef.current;

    if (Hls.isSupported()) {
      const hls = new Hls({
        enableWorker: true,
        lowLatencyMode: true,
        startFragPrefetch: true,
        maxBufferLength: 10,
        maxMaxBufferLength: 20,
        maxBufferSize: 30 * 1000 * 1000,
        liveSyncDurationCount: 2,
        liveMaxLatencyDurationCount: 4,
        capLevelToPlayerSize: true,
        fragLoadingTimeOut: 12000,
        manifestLoadingTimeOut: 6000,
        levelLoadingTimeOut: 6000,
        progressive: true,
      });

      hlsRef.current = hls;
      hls.loadSource(streamManifestUrl);
      hls.attachMedia(videoEl);

      hls.on(Hls.Events.MANIFEST_PARSED, (_, data) => {
        const levels: QualityLevel[] = data.levels.map((lvl, index) => ({
          id: index,
          width: lvl.width || 0,
          height: lvl.height || 1080,
          bitrate: lvl.bitrate || 0,
          label: formatQualityLabel(lvl.width, lvl.height),
        }));
        setQualities(levels);
        videoEl.play().catch(() => {
          // Autoplay policy fallback: mute and play
          videoEl.muted = true;
          setIsMuted(true);
          videoEl.play().catch(() => {});
        });
      });

      hls.on(Hls.Events.LEVEL_SWITCHED, (_, data) => {
        const lvl = hls.levels[data.level];
        if (lvl) {
          const label = formatQualityLabel(lvl.width, lvl.height);
          setCurrentQualityLabel(label);
          setHudStats((prev) => ({
            ...prev,
            resolution: `${lvl.width || 1920}x${lvl.height || 1080} (${label})`,
            bandwidth: Math.round(lvl.bitrate / 1000),
          }));
        }
      });

      hls.on(Hls.Events.ERROR, (_, data) => {
        if (data.fatal) {
          switch (data.type) {
            case Hls.ErrorTypes.NETWORK_ERROR:
              hls.startLoad();
              break;
            case Hls.ErrorTypes.MEDIA_ERROR:
              hls.recoverMediaError();
              break;
            default:
              hls.destroy();
              break;
          }
        }
      });
    } else if (videoEl.canPlayType('application/vnd.apple.mpegurl')) {
      // Native HLS (Safari/iOS)
      videoEl.src = streamManifestUrl;
      videoEl.play().catch(() => {});
    }

    return () => {
      if (hlsRef.current) {
        hlsRef.current.destroy();
        hlsRef.current = null;
      }
    };
  }, [video]);

  // Video Time & Buffer Update
  const handleTimeUpdate = () => {
    if (!videoRef.current) return;
    const v = videoRef.current;
    setCurrentTime(v.currentTime);
    setDuration(v.duration || 0);

    // Calculate buffer ahead
    if (v.buffered.length > 0) {
      for (let i = 0; i < v.buffered.length; i++) {
        if (v.buffered.start(i) <= v.currentTime && v.buffered.end(i) >= v.currentTime) {
          const ahead = v.buffered.end(i) - v.currentTime;
          setBuffered(v.buffered.end(i));
          setHudStats((prev) => ({
            ...prev,
            bufferAheadSecs: Math.round(ahead * 10) / 10,
          }));
          break;
        }
      }
    }
  };

  const togglePlay = useCallback(() => {
    if (!videoRef.current) return;
    if (videoRef.current.paused) {
      videoRef.current.play();
      setIsPlaying(true);
    } else {
      videoRef.current.pause();
      setIsPlaying(false);
    }
  }, []);

  const toggleMute = () => {
    if (!videoRef.current) return;
    const nextMuted = !isMuted;
    videoRef.current.muted = nextMuted;
    setIsMuted(nextMuted);
  };

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseFloat(e.target.value);
    setVolume(val);
    if (videoRef.current) {
      videoRef.current.volume = val;
      const muted = val === 0;
      videoRef.current.muted = muted;
      setIsMuted(muted);
    }
  };

  const handleSeek = (e: React.ChangeEvent<HTMLInputElement>) => {
    const target = parseFloat(e.target.value);
    if (videoRef.current) {
      videoRef.current.currentTime = target;
      setCurrentTime(target);
    }
  };

  const toggleFullscreen = () => {
    if (!containerRef.current) return;
    if (!document.fullscreenElement) {
      containerRef.current.requestFullscreen().catch(() => {});
      setIsFullscreen(true);
    } else {
      document.exitFullscreen().catch(() => {});
      setIsFullscreen(false);
    }
  };

  const handleQualityChange = (levelIndex: number) => {
    if (!hlsRef.current) return;
    hlsRef.current.currentLevel = levelIndex;
    setSelectedQuality(levelIndex);
    if (levelIndex === -1) {
      setCurrentQualityLabel('Auto');
    } else {
      const lvl = qualities.find((q) => q.id === levelIndex);
      if (lvl) setCurrentQualityLabel(lvl.label);
    }
    setShowSettings(false);
  };

  const handleSpeedChange = (speed: number) => {
    if (!videoRef.current) return;
    videoRef.current.playbackRate = speed;
    setPlaybackSpeed(speed);
    setShowSettings(false);
  };

  // Keyboard Navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      switch (e.key) {
        case ' ':
          e.preventDefault();
          togglePlay();
          break;
        case 'm':
        case 'M':
          toggleMute();
          break;
        case 'f':
        case 'F':
          toggleFullscreen();
          break;
        case 'd':
        case 'D':
          setShowDiagnostics((prev) => !prev);
          break;
        case 'Escape':
          if (isFullscreen) {
            document.exitFullscreen().catch(() => {});
          } else {
            onClose();
          }
          break;
        case 'ArrowLeft':
          if (videoRef.current) videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime - 10);
          break;
        case 'ArrowRight':
          if (videoRef.current) videoRef.current.currentTime = Math.min(duration, videoRef.current.currentTime + 10);
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [togglePlay, isFullscreen, duration, onClose]);

  const formatTime = (seconds: number) => {
    if (isNaN(seconds) || seconds < 0) return '00:00';
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs < 10 ? '0' : ''}${secs}`;
  };

  if (!video) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/95 backdrop-blur-xl animate-in fade-in duration-200">
      <div
        ref={containerRef}
        onMouseMove={handleMouseMove}
        className="relative w-full h-full max-w-7xl max-h-[90vh] sm:rounded-3xl overflow-hidden bg-black shadow-2xl flex flex-col justify-center"
      >
        {/* Toast Feedback */}
        {interactionToast && (
          <div className="absolute top-16 left-1/2 -translate-x-1/2 z-40 px-4 py-2 rounded-xl bg-red-600/95 text-white font-semibold text-xs shadow-2xl backdrop-blur-md animate-in fade-in slide-in-from-top-2">
            {interactionToast}
          </div>
        )}

        {/* Video Element */}
        <video
          ref={videoRef}
          onTimeUpdate={handleTimeUpdate}
          onPlay={() => setIsPlaying(true)}
          onPause={() => setIsPlaying(false)}
          onClick={togglePlay}
          className="w-full h-full object-contain cursor-pointer"
          playsInline
        />

        {/* Top Header Controls Overlay */}
        <div
          className={`absolute top-0 left-0 right-0 p-4 sm:p-6 bg-gradient-to-b from-black/90 via-black/40 to-transparent flex items-center justify-between transition-opacity duration-300 ${
            showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
          }`}
        >
          <div className="flex items-center space-x-3">
            <button
              onClick={onClose}
              className="p-2 rounded-full bg-white/10 hover:bg-white/20 text-white transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
            <div>
              <h2 className="text-white font-bold text-base sm:text-lg line-clamp-1">{video.title}</h2>
              <div className="flex items-center gap-2 text-xs text-slate-300">
                <span className="text-emerald-400 font-semibold">{video.quality}</span>
                <span>•</span>
                <span>{video.category}</span>
                <span>•</span>
                <span className="text-slate-400">
                  {video.is_live === 1 ? 'Live Stream' : 'HLS'}
                </span>
              </div>
            </div>
          </div>

          {/* Right Header: Like Button, 5-Star Ratings, and Telemetry */}
          <div className="flex items-center space-x-2 sm:space-x-3">
            {/* Interactive Like Button */}
            <button
              onClick={handleToggleLike}
              className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-xl border text-xs font-semibold transition-all ${
                hasLiked
                  ? 'bg-rose-600/20 border-rose-500 text-rose-400'
                  : 'bg-white/10 hover:bg-white/20 border-white/10 text-slate-300 hover:text-white'
              }`}
              title="Like this video"
            >
              <Heart className={`w-4 h-4 ${hasLiked ? 'fill-current text-rose-500' : ''}`} />
              <span>{likesCount}</span>
            </button>

            {/* Interactive 1-5 Star Rating Selector */}
            <div className="hidden md:flex items-center space-x-1 px-3 py-1.5 rounded-xl bg-white/10 border border-white/10 text-xs">
              <span className="text-amber-400 font-bold mr-1">{avgRating}★</span>
              <div className="flex items-center">
                {[1, 2, 3, 4, 5].map((star) => (
                  <button
                    key={star}
                    onClick={() => handleRate(star)}
                    onMouseEnter={() => setHoverRating(star)}
                    onMouseLeave={() => setHoverRating(null)}
                    className="p-0.5 hover:scale-125 transition-transform"
                    title={`Rate ${star} Stars`}
                  >
                    <Star
                      className={`w-3.5 h-3.5 ${
                        (hoverRating !== null ? star <= hoverRating : userRating !== null ? star <= userRating : star <= Math.round(avgRating))
                          ? 'fill-amber-400 text-amber-400'
                          : 'text-slate-500'
                      }`}
                    />
                  </button>
                ))}
              </div>
              {userRating && (
                <span className="text-[10px] text-emerald-400 font-medium ml-1">Rated</span>
              )}
            </div>

            <button
              onClick={() => setShowDiagnostics((p) => !p)}
              className={`p-2 rounded-xl text-xs font-semibold flex items-center gap-1.5 transition-colors ${
                showDiagnostics ? 'bg-red-600 text-white' : 'bg-white/10 hover:bg-white/20 text-slate-300'
              }`}
              title="Stream Diagnostics (Hotkey: D)"
            >
              <Activity className="w-4 h-4" />
              <span className="hidden sm:inline">Telemetry</span>
            </button>
          </div>
        </div>

        {/* Stream Diagnostics HUD */}
        {showDiagnostics && (
          <div className="absolute top-20 right-6 z-30 p-4 rounded-2xl bg-black/85 backdrop-blur-md border border-white/15 text-xs font-mono text-slate-300 space-y-2 w-72 shadow-2xl">
            <div className="flex items-center justify-between pb-1 border-b border-white/10">
              <span className="text-red-400 font-bold uppercase tracking-wider">Stream Telemetry</span>
              <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            </div>
            <div className="flex justify-between">
              <span>Resolution:</span>
              <span className="text-white font-semibold">{hudStats.resolution}</span>
            </div>
            <div className="flex justify-between">
              <span>Bitrate:</span>
              <span className="text-white font-semibold">
                {hudStats.bandwidth > 0 ? `${hudStats.bandwidth} kbps` : 'Variable'}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Buffer Ahead:</span>
              <span className="text-emerald-400 font-semibold">{hudStats.bufferAheadSecs}s</span>
            </div>
            <div className="flex justify-between">
              <span>Rating:</span>
              <span className="text-amber-400 font-semibold">{avgRating} ★ ({totalRatings} votes)</span>
            </div>
            <div className="flex justify-between">
              <span>Likes:</span>
              <span className="text-rose-400 font-semibold">{likesCount}</span>
            </div>
            <div className="flex justify-between">
              <span>Delivery:</span>
              <span className="text-indigo-400 font-semibold">Proxy</span>
            </div>
            <div className="flex justify-between">
              <span>Token:</span>
              <span className="text-emerald-400 font-semibold">Verified</span>
            </div>
            <div className="flex justify-between items-center pt-2 border-t border-white/10">
              <span className="text-slate-400">Stream Cache:</span>
              <button
                onClick={handleClearCache}
                disabled={clearingCache}
                className="px-2 py-1 bg-white/10 hover:bg-rose-500/20 text-slate-300 hover:text-rose-400 rounded-md border border-white/10 text-[10px] font-sans flex items-center gap-1 transition-all active:scale-95 disabled:opacity-50"
                title="Purge in-memory segment cache"
              >
                <Trash2 className={`w-3 h-3 ${clearingCache ? 'animate-spin text-rose-400' : ''}`} />
                <span>{clearingCache ? 'Purging...' : 'Clear Cache'}</span>
              </button>
            </div>
          </div>
        )}

        {/* Center Play Button Overlay When Paused */}
        {!isPlaying && (
          <div
            onClick={togglePlay}
            className="absolute inset-0 flex items-center justify-center cursor-pointer pointer-events-auto"
          >
            <div className="w-20 h-20 rounded-full bg-red-600/90 text-white flex items-center justify-center shadow-2xl shadow-red-600/50 hover:scale-110 transition-transform">
              <Play className="w-9 h-9 fill-current ml-1" />
            </div>
          </div>
        )}

        {/* Bottom Custom Playback Bar Overlay */}
        <div
          className={`absolute bottom-0 left-0 right-0 p-4 sm:p-6 bg-gradient-to-t from-black/95 via-black/60 to-transparent transition-opacity duration-300 ${
            showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
          }`}
        >
          {/* Seekbar */}
          {video.is_live === 0 && duration > 0 && (
            <div className="relative group/seek mb-4 cursor-pointer">
              <input
                type="range"
                min="0"
                max={duration}
                value={currentTime}
                onChange={handleSeek}
                className="w-full h-1.5 bg-white/20 group-hover/seek:h-2 rounded-lg appearance-none cursor-pointer accent-red-600 transition-all"
              />
              {/* Buffered progress bar */}
              <div
                className="absolute top-0 left-0 h-1.5 bg-white/30 rounded-lg pointer-events-none -z-10"
                style={{ width: `${(buffered / duration) * 100}%` }}
              />
            </div>
          )}

          <div className="flex items-center justify-between">
            {/* Left Controls */}
            <div className="flex items-center space-x-3 sm:space-x-4">
              <button
                onClick={togglePlay}
                className="p-2 rounded-xl bg-white/10 hover:bg-white/20 text-white transition-colors"
              >
                {isPlaying ? <Pause className="w-5 h-5 fill-current" /> : <Play className="w-5 h-5 fill-current" />}
              </button>

              <div className="flex items-center space-x-2 group/vol">
                <button
                  onClick={toggleMute}
                  className="p-2 rounded-xl bg-white/10 hover:bg-white/20 text-white transition-colors"
                >
                  {isMuted || volume === 0 ? <VolumeX className="w-5 h-5" /> : <Volume2 className="w-5 h-5" />}
                </button>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.05"
                  value={isMuted ? 0 : volume}
                  onChange={handleVolumeChange}
                  className="w-16 sm:w-20 h-1.5 bg-white/20 rounded-lg appearance-none cursor-pointer accent-red-600"
                />
              </div>

              {/* Time or LIVE indicator */}
              {video.is_live === 1 ? (
                <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-red-600/90 text-white text-xs font-bold uppercase tracking-wider">
                  <Radio className="w-3.5 h-3.5 animate-pulse" />
                  LIVE
                </div>
              ) : (
                <div className="text-xs font-mono text-slate-300">
                  {formatTime(currentTime)} / {formatTime(duration)}
                </div>
              )}
            </div>

            {/* Right Controls */}
            <div className="flex items-center space-x-2 sm:space-x-3">
              {/* Settings / Resolution Toggle */}
              <div className="relative">
                <button
                  onClick={() => setShowSettings((p) => !p)}
                  className="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-white/10 hover:bg-white/20 text-white text-xs font-semibold transition-colors"
                >
                  <Settings className="w-4 h-4" />
                  <span>{currentQualityLabel}</span>
                </button>

                {showSettings && (
                  <div className="absolute bottom-12 right-0 w-48 rounded-2xl bg-black/90 backdrop-blur-xl border border-white/15 p-2 shadow-2xl text-xs z-40 space-y-2">
                    <div>
                      <div className="px-2 py-1 text-slate-400 font-bold uppercase tracking-wider text-[10px]">
                        Video Quality
                      </div>
                      <button
                        onClick={() => handleQualityChange(-1)}
                        className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left ${
                          selectedQuality === -1 ? 'bg-red-600 text-white font-bold' : 'text-slate-300 hover:bg-white/10'
                        }`}
                      >
                        <span>Auto</span>
                        {selectedQuality === -1 && <Check className="w-3.5 h-3.5" />}
                      </button>
                      {qualities.map((q) => (
                        <button
                          key={q.id}
                          onClick={() => handleQualityChange(q.id)}
                          className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left ${
                            selectedQuality === q.id ? 'bg-red-600 text-white font-bold' : 'text-slate-300 hover:bg-white/10'
                          }`}
                        >
                          <span>{q.label}</span>
                          {selectedQuality === q.id && <Check className="w-3.5 h-3.5" />}
                        </button>
                      ))}
                    </div>

                    <div className="border-t border-white/10 pt-1">
                      <div className="px-2 py-1 text-slate-400 font-bold uppercase tracking-wider text-[10px]">
                        Playback Speed
                      </div>
                      {[0.5, 1, 1.25, 1.5, 2].map((s) => (
                        <button
                          key={s}
                          onClick={() => handleSpeedChange(s)}
                          className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left ${
                            playbackSpeed === s ? 'bg-red-600 text-white font-bold' : 'text-slate-300 hover:bg-white/10'
                          }`}
                        >
                          <span>{s}x</span>
                          {playbackSpeed === s && <Check className="w-3.5 h-3.5" />}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>

              {/* Fullscreen Button */}
              <button
                onClick={toggleFullscreen}
                className="p-2 rounded-xl bg-white/10 hover:bg-white/20 text-white transition-colors"
                title="Fullscreen (Hotkey: F)"
              >
                {isFullscreen ? <Minimize className="w-5 h-5" /> : <Maximize className="w-5 h-5" />}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
