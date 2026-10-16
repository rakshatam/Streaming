'use client';

import React, { useState } from 'react';
import { X, Lock, User, ShieldCheck, ArrowRight, CheckCircle2, UserPlus } from 'lucide-react';
import { User as UserType } from '../types';
import { apiUrl } from '../lib/api';

interface AuthModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (user: UserType, token: string) => void;
}

export const AuthModal: React.FC<AuthModalProps> = ({ isOpen, onClose, onSuccess }) => {
  const [isRegister, setIsRegister] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const successTimeoutRef = React.useRef<NodeJS.Timeout | null>(null);

  React.useEffect(() => {
    return () => {
      if (successTimeoutRef.current) clearTimeout(successTimeoutRef.current);
    };
  }, []);

  if (!isOpen) return null;

  // Direct, Secure Registration (Pure Username + Argon2id Password)
  const handleRegister = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMsg(null);

    const cleanUsername = username.trim();
    if (cleanUsername.length < 3 || cleanUsername.length > 30) {
      setError('Username must be between 3 and 30 characters.');
      return;
    }

    if (!/^[a-zA-Z0-9_-]+$/.test(cleanUsername)) {
      setError('Username can only contain letters, numbers, underscores, and hyphens.');
      return;
    }

    if (password.length < 8) {
      setError('Password must be at least 8 characters long.');
      return;
    }

    if (password !== confirmPassword) {
      setError('Passwords do not match. Please re-enter.');
      return;
    }

    setIsLoading(true);

    try {
      const res = await fetch(apiUrl('/api/auth/register'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: cleanUsername, password }),
      });

      const text = await res.text();
      let data: any = {};
      try {
        data = JSON.parse(text);
      } catch {
        data = { message: text };
      }

      if (!res.ok) {
        throw new Error(data.message || 'Registration failed');
      }

      localStorage.setItem('streamflow_token', data.token);
      setSuccessMsg(`Welcome, ${data.user.username}! Account created securely.`);
      if (successTimeoutRef.current) clearTimeout(successTimeoutRef.current);
      successTimeoutRef.current = setTimeout(() => {
        onSuccess(data.user, data.token);
        onClose();
      }, 600);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  // Direct, Secure Sign In (Username + Password)
  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    try {
      const res = await fetch(apiUrl('/api/auth/login'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: username.trim(), password }),
      });

      const text = await res.text();
      let data: any = {};
      try {
        data = JSON.parse(text);
      } catch {
        data = { message: text };
      }

      if (!res.ok) {
        throw new Error(data.message || 'Invalid username or password');
      }

      localStorage.setItem('streamflow_token', data.token);
      onSuccess(data.user, data.token);
      onClose();
    } catch (err: any) {
      setError(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const switchMode = (toRegister: boolean) => {
    setIsRegister(toRegister);
    setError(null);
    setSuccessMsg(null);
    setPassword('');
    setConfirmPassword('');
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-xl animate-in fade-in duration-200">
      <div className="relative w-full max-w-md rounded-3xl bg-[#0e0e16] border border-white/10 p-6 sm:p-8 shadow-2xl space-y-6">
        <button
          onClick={onClose}
          className="absolute top-5 right-5 p-2 rounded-full text-slate-400 hover:text-white bg-white/5 hover:bg-white/10 transition-colors"
        >
          <X className="w-4 h-4" />
        </button>

        {/* Header */}
        <div className="space-y-1">
          <h3 className="text-2xl font-bold text-white tracking-tight">
            {isRegister ? 'Create Account' : 'Sign In'}
          </h3>
          <p className="text-xs text-slate-400">
            {isRegister
              ? 'Choose a username and password to create your account.'
              : 'Sign in to like and rate videos.'}
          </p>
        </div>

        {/* Tab Toggle */}
        <div className="flex rounded-xl bg-white/5 p-1 border border-white/5">
          <button
            type="button"
            onClick={() => switchMode(false)}
            className={`flex-1 py-2 text-xs font-semibold rounded-lg transition-all ${
              !isRegister ? 'bg-red-600 text-white shadow-md' : 'text-slate-400 hover:text-white'
            }`}
          >
            Sign In
          </button>
          <button
            type="button"
            onClick={() => switchMode(true)}
            className={`flex-1 py-2 text-xs font-semibold rounded-lg transition-all ${
              isRegister ? 'bg-red-600 text-white shadow-md' : 'text-slate-400 hover:text-white'
            }`}
          >
            Create Account
          </button>
        </div>

        {error && (
          <div className="p-3.5 rounded-xl bg-red-500/15 border border-red-500/30 text-red-300 text-xs font-medium">
            {error}
          </div>
        )}

        {successMsg && (
          <div className="p-3.5 rounded-xl bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 text-xs font-medium flex items-center gap-2">
            <CheckCircle2 className="w-4 h-4 flex-shrink-0" />
            <span>{successMsg}</span>
          </div>
        )}

        {/* Sign In Form */}
        {!isRegister ? (
          <form onSubmit={handleLogin} className="space-y-4">
            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Username</label>
              <div className="relative flex items-center">
                <User className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="text"
                  required
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="Enter your username"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Password</label>
              <div className="relative flex items-center">
                <Lock className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="password"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="••••••••"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <button
              type="submit"
              disabled={isLoading}
              className="w-full py-3 rounded-xl bg-red-600 hover:bg-red-700 text-white font-bold text-sm shadow-lg shadow-red-600/30 transition-all disabled:opacity-50 flex items-center justify-center gap-2"
            >
              <span>{isLoading ? 'Verifying...' : 'Sign In'}</span>
              <ArrowRight className="w-4 h-4" />
            </button>
          </form>
        ) : (
          /* Create Account Form (Pure Username + Password) */
          <form onSubmit={handleRegister} className="space-y-4">
            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Choose Username</label>
              <div className="relative flex items-center">
                <User className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="text"
                  required
                  minLength={3}
                  maxLength={30}
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="e.g. alex_hunter"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
              <span className="text-[10px] text-slate-500">3–30 characters (letters, numbers, _ and -)</span>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Set Password</label>
              <div className="relative flex items-center">
                <Lock className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="password"
                  required
                  minLength={8}
                  maxLength={128}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="At least 8 characters"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Confirm Password</label>
              <div className="relative flex items-center">
                <Lock className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="password"
                  required
                  minLength={8}
                  maxLength={128}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Repeat your password"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <button
              type="submit"
              disabled={isLoading}
              className="w-full py-3 rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white font-bold text-sm shadow-lg shadow-emerald-600/30 flex items-center justify-center gap-2 transition-all disabled:opacity-50"
            >
              <UserPlus className="w-4 h-4" />
              <span>{isLoading ? 'Creating Account...' : 'Create Account'}</span>
            </button>
          </form>
        )}

        <div className="text-center pt-2 border-t border-white/5">
          <button
            type="button"
            onClick={() => switchMode(!isRegister)}
            className="text-xs text-slate-400 hover:text-white transition-colors"
          >
            {isRegister
              ? 'Already have an account? Sign In here'
              : "Don't have an account yet? Create one in seconds"}
          </button>
        </div>
      </div>
    </div>
  );
};
