import React, { useState } from 'react';
import { Terminal, Lock, User, AlertCircle, ArrowRight } from 'lucide-react';
import { api } from '../lib/api';

export interface LoginPageProps {
  onLoginSuccess: (username: string) => void;
}

export const LoginPage: React.FC<LoginPageProps> = ({ onLoginSuccess }) => {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username || !password) return;
    setLoading(true);
    setError(null);

    try {
      const res = await api.login(username, password);
      localStorage.setItem('nexdns_token', res.token);
      localStorage.setItem('nexdns_user', res.username);
      onLoginSuccess(res.username);
    } catch (err: any) {
      setError(err.message || '用户名或密码错误');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-[#f6f6f5] dark:bg-[#0b0b0a] flex items-center justify-center p-4">
      <div className="w-full max-w-[400px] bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 rounded-xl shadow-xl p-8 space-y-6 animate-in fade-in-50 zoom-in-95 duration-200">
        {/* Logo 与产品名称 */}
        <div className="text-center space-y-2">
          <div className="w-12 h-12 rounded-xl bg-gradient-to-tr from-cyan-600 to-blue-600 flex items-center justify-center text-white mx-auto shadow-md shadow-cyan-950/40">
            <Terminal className="w-6 h-6 align-icon" />
          </div>
          <h2 className="text-xl font-bold tracking-tight text-neutral-900 dark:text-neutral-100">
            NexDNS
          </h2>
          <p className="text-xs text-neutral-500 dark:text-neutral-400">
            请登录控制台管理 DNS 服务与分流策略
          </p>
        </div>

        {/* 错误提示 */}
        {error && (
          <div className="p-3 rounded-lg bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900/50 flex items-center gap-2 text-xs text-red-600 dark:text-red-400">
            <AlertCircle className="w-4 h-4 shrink-0 align-icon" />
            <span>{error}</span>
          </div>
        )}

        {/* 登录表单 */}
        <form onSubmit={handleSubmit} className="space-y-4 text-xs">
          <div className="space-y-1">
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium">
              用户名
            </label>
            <div className="relative">
              <User className="w-4 h-4 text-neutral-400 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                required
                className="w-full min-h-[40px] pl-9 pr-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-lg text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500 transition-colors"
                placeholder="请输入管理员账户"
              />
            </div>
          </div>

          <div className="space-y-1">
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium">
              密码
            </label>
            <div className="relative">
              <Lock className="w-4 h-4 text-neutral-400 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                className="w-full min-h-[40px] pl-9 pr-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-lg text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500 transition-colors"
                placeholder="请输入密码"
              />
            </div>
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full min-h-[40px] mt-2 inline-flex items-center justify-center gap-2 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-lg shadow-sm transition-colors disabled:opacity-50"
          >
            <span>{loading ? '正在验证...' : '登录系统'}</span>
            <ArrowRight className="w-4 h-4 align-icon" />
          </button>
        </form>
      </div>
    </div>
  );
};
