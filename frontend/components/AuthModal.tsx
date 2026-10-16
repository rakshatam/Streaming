'use client';

import React, { useState } from 'react';
import { X, Lock, Mail, User, ShieldCheck, KeyRound, ArrowRight, CheckCircle2 } from 'lucide-react';
import { User as UserType } from '../types';

interface AuthModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (user: UserType, token: string) => void;
}

export const AuthModal: React.FC<AuthModalProps> = ({ isOpen, onClose, onSuccess }) => {
  const [isRegister, setIsRegister] = useState(false);
  const [otpSent, setOtpSent] = useState(false);
  const [smtpDispatched, setSmtpDispatched] = useState<boolean>(false);
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [otpCode, setOtpCode] = useState('');
  const [devOtpHint, setDevOtpHint] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  if (!isOpen) return null;

  // Step 1 of Register: Request OTP
  const handleRequestOtp = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMsg(null);
    setIsLoading(true);

    try {
      const res = await fetch('/api/auth/register/request-otp', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, email, password }),
      });

      const text = await res.text();
      let data: any = {};
      try {
        data = JSON.parse(text);
      } catch {
        data = { message: text };
      }

      if (!res.ok) {
        throw new Error(data.message || 'Failed to request verification code');
      }

      setOtpSent(true);
      setSmtpDispatched(!!data.smtp_dispatched);
      setDevOtpHint(data.dev_otp_preview);
      if (data.smtp_dispatched) {
        setSuccessMsg(`Security verification email sent to ${email}! Please check your inbox (and spam/junk folder).`);
      } else {
        setSuccessMsg(data.message || `Verification code generated. Configure SMTP in backend/.env to send real emails.`);
      }
    } catch (err: any) {
      setError(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  // Step 2 of Register: Verify OTP & Create Account
  const handleVerifyOtp = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    try {
      const res = await fetch('/api/auth/register/verify-otp', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, otp_code: otpCode }),
      });

      const text = await res.text();
      let data: any = {};
      try {
        data = JSON.parse(text);
      } catch {
        data = { message: text };
      }

      if (!res.ok) {
        throw new Error(data.message || 'OTP verification failed');
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

  // Login: Pure Username + Password
  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
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

  const resetForm = () => {
    setError(null);
    setSuccessMsg(null);
    setOtpSent(false);
    setSmtpDispatched(false);
    setDevOtpHint(null);
    setOtpCode('');
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xl animate-in fade-in duration-200">
      <div className="relative w-full max-w-md rounded-3xl bg-[#0e0e16] border border-white/10 p-6 sm:p-8 shadow-2xl space-y-6">
        <button
          onClick={onClose}
          className="absolute top-5 right-5 p-2 rounded-full text-slate-400 hover:text-white bg-white/5 hover:bg-white/10 transition-colors"
        >
          <X className="w-4 h-4" />
        </button>

        <div className="space-y-1">
          <div className="flex items-center gap-2 text-red-500 font-bold text-xs uppercase tracking-wider">
            <ShieldCheck className="w-4 h-4" />
            <span>Argon2id Enterprise Security</span>
          </div>
          <h3 className="text-2xl font-bold text-white tracking-tight">
            {isRegister
              ? otpSent
                ? 'Verify Security OTP'
                : 'Create Account with Email OTP'
              : 'Sign In with Username'}
          </h3>
          <p className="text-xs text-slate-400">
            {isRegister
              ? otpSent
                ? `Enter the 6-digit verification code dispatched to ${email}`
                : 'Create your account safely using our two-factor email verification protocol.'
              : 'Enter your username and password to access rating, liking, and watch history.'}
          </p>
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

        {/* Real Email Dispatched Banner or Local OTP Preview */}
        {otpSent && (
          smtpDispatched ? (
            <div className="p-3.5 rounded-2xl bg-emerald-500/10 border border-emerald-500/30 text-xs space-y-1.5">
              <div className="text-emerald-300 font-semibold flex items-center gap-1.5">
                <Mail className="w-4 h-4 text-emerald-400" /> Real Email Dispatched Over SMTP
              </div>
              <p className="text-slate-300 text-[11px] leading-relaxed">
                A verification email was sent to <strong className="text-white">{email}</strong>. Please check your inbox and spam folder.
              </p>
            </div>
          ) : (
            <div className="p-3.5 rounded-2xl bg-amber-500/10 border border-amber-500/30 text-xs space-y-2">
              <div className="text-amber-300 font-semibold flex items-center justify-between">
                <span className="flex items-center gap-1.5">
                  <KeyRound className="w-4 h-4 text-amber-400" /> Local Test Code (SMTP Unconfigured)
                </span>
                {devOtpHint && (
                  <button
                    type="button"
                    onClick={() => setOtpCode(devOtpHint)}
                    className="px-2 py-0.5 rounded bg-amber-500/20 text-amber-300 hover:bg-amber-500/30 font-mono text-[11px] font-bold tracking-wider transition-colors"
                    title="Click to auto-fill code"
                  >
                    Auto-Fill {devOtpHint}
                  </button>
                )}
              </div>
              <p className="text-slate-300 text-[11px] leading-relaxed">
                No email was received because SMTP is not yet configured in <code className="text-amber-200">backend/.env</code>. To receive live emails in your inbox, set your SMTP credentials in <code className="text-amber-200">backend/.env</code>.
              </p>
            </div>
          )
        )}

        {/* Forms */}
        {!isRegister ? (
          /* Login Form: Pure Username + Password */
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
              className="w-full py-3 rounded-xl bg-red-600 hover:bg-red-700 text-white font-bold text-sm shadow-lg shadow-red-600/30 transition-all disabled:opacity-50"
            >
              {isLoading ? 'Verifying Credentials...' : 'Sign In'}
            </button>
          </form>
        ) : !otpSent ? (
          /* Register Form Step 1: Input details & send OTP */
          <form onSubmit={handleRequestOtp} className="space-y-4">
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
                  placeholder="alex_streamer"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Email Address (For OTP)</label>
              <div className="relative flex items-center">
                <Mail className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="email"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="alex@example.com"
                  className="w-full pl-9 pr-3 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-sm focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Set Password (Min 8 chars)</label>
              <div className="relative flex items-center">
                <Lock className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="password"
                  required
                  minLength={8}
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
              className="w-full py-3 rounded-xl bg-red-600 hover:bg-red-700 text-white font-bold text-sm shadow-lg shadow-red-600/30 flex items-center justify-center gap-2 transition-all disabled:opacity-50"
            >
              <span>{isLoading ? 'Generating OTP...' : 'Send Security OTP Code'}</span>
              <ArrowRight className="w-4 h-4" />
            </button>
          </form>
        ) : (
          /* Register Form Step 2: Verify OTP */
          <form onSubmit={handleVerifyOtp} className="space-y-4">
            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">Enter 6-Digit OTP Code</label>
              <div className="relative flex items-center">
                <KeyRound className="absolute left-3 w-4 h-4 text-slate-500" />
                <input
                  type="text"
                  required
                  maxLength={6}
                  value={otpCode}
                  onChange={(e) => setOtpCode(e.target.value)}
                  placeholder="123456"
                  className="w-full pl-9 pr-3 py-3 rounded-xl bg-white/5 border border-white/10 text-white placeholder-slate-500 text-lg font-mono tracking-widest text-center focus:outline-none focus:border-red-500 transition-colors"
                />
              </div>
            </div>

            <button
              type="submit"
              disabled={isLoading}
              className="w-full py-3 rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white font-bold text-sm shadow-lg shadow-emerald-600/30 transition-all disabled:opacity-50"
            >
              {isLoading ? 'Verifying...' : 'Verify OTP & Complete Account'}
            </button>

            <button
              type="button"
              onClick={() => setOtpSent(false)}
              className="w-full text-xs text-slate-400 hover:text-slate-300 text-center"
            >
              Back to edit details
            </button>
          </form>
        )}

        <div className="text-center pt-2 border-t border-white/5">
          <button
            type="button"
            onClick={() => {
              setIsRegister(!isRegister);
              resetForm();
            }}
            className="text-xs text-slate-400 hover:text-white transition-colors"
          >
            {isRegister
              ? 'Already registered? Sign In with Username'
              : "Need an account? Create One with Secure Email OTP"}
          </button>
        </div>
      </div>
    </div>
  );
};
