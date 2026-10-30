import React, { useEffect, useState } from 'react';
import {
  Activity,
  SlidersHorizontal,
  FileText,
  Shield,
  Server,
  Settings,
  GitFork,
  Moon,
  Sun,
  Terminal,
  FileCode,
  LogOut,
  UserCheck,
} from 'lucide-react';
import { api } from './lib/api';
import { LoginPage } from './pages/LoginPage';
import { OverviewPage } from './pages/OverviewPage';
import { PipelinePage } from './pages/PipelinePage';
import { QueryLogPage } from './pages/QueryLogPage';
import { FilteringPage } from './pages/FilteringPage';
import { RoutingPage } from './pages/RoutingPage';
import { UpstreamsPage } from './pages/UpstreamsPage';
import { RewritesPage } from './pages/RewritesPage';
import { SettingsPage } from './pages/SettingsPage';

export function App() {
  const [currentUser, setCurrentUser] = useState<string | null>(() => {
    return localStorage.getItem('nexdns_user');
  });
  const [authChecked, setAuthChecked] = useState(false);

  // 持久化 Tab 状态至 URL，避免页面刷新画面抖动
  const [activeTab, setActiveTab] = useState<string>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get('tab') || 'overview';
  });
  const [isDark, setIsDark] = useState<boolean>(true);

  useEffect(() => {
    api.getMe().then((res) => {
      if (res.authenticated) {
        setCurrentUser(res.username);
        localStorage.setItem('nexdns_user', res.username);
      } else {
        setCurrentUser(null);
        localStorage.removeItem('nexdns_token');
        localStorage.removeItem('nexdns_user');
      }
      setAuthChecked(true);
    }).catch(() => {
      setAuthChecked(true);
    });
  }, []);

  const handleLogout = async () => {
    await api.logout();
    setCurrentUser(null);
  };

  useEffect(() => {
    const url = new URL(window.location.href);
    url.searchParams.set('tab', activeTab);
    window.history.replaceState({}, '', url.toString());
  }, [activeTab]);

  useEffect(() => {
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [isDark]);

  if (!authChecked) {
    return <div className="min-h-screen bg-[#f6f6f5] dark:bg-[#0b0b0a]" />;
  }

  if (!currentUser) {
    return <LoginPage onLoginSuccess={(u) => setCurrentUser(u)} />;
  }

  const navItems = [
    { id: 'overview', label: '工作台概览', icon: <Activity className="w-4 h-4 align-icon" /> },
    { id: 'querylog', label: '实时查询日志', icon: <FileText className="w-4 h-4 align-icon" /> },
    { id: 'filtering', label: '拦截与规则库', icon: <Shield className="w-4 h-4 align-icon" /> },
    { id: 'routing', label: '分流规则集', icon: <FileCode className="w-4 h-4 align-icon" /> },
    { id: 'pipeline', label: '流水线拓扑', icon: <SlidersHorizontal className="w-4 h-4 align-icon" /> },
    { id: 'upstreams', label: '上游与测速优选', icon: <Server className="w-4 h-4 align-icon" /> },
    { id: 'rewrites', label: '自定义重写', icon: <GitFork className="w-4 h-4 align-icon" /> },
    { id: 'settings', label: '系统设置', icon: <Settings className="w-4 h-4 align-icon" /> },
  ];

  return (
    <div className="min-h-screen bg-[#f6f6f5] dark:bg-[#0b0b0a] text-neutral-900 dark:text-neutral-100 flex flex-col font-sans transition-colors duration-150">
      {/* 顶部固定导航栏 */}
      <header className="sticky top-0 z-40 bg-white/90 dark:bg-surface-1/90 backdrop-blur-md border-b border-neutral-200 dark:border-neutral-800">
        <div className="max-w-[1700px] w-full mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
          {/* 品牌 Logo */}
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-lg bg-gradient-to-tr from-cyan-600 to-blue-600 flex items-center justify-center text-white shadow-md shadow-cyan-950/40">
              <Terminal className="w-5 h-5 align-icon" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="font-bold text-base tracking-tight text-neutral-900 dark:text-neutral-100">NexDNS</span>
                <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" title="运行正常" />
              </div>
            </div>
          </div>

          {/* 中间 Tab 导航栏 */}
          <nav className="hidden xl:flex items-center gap-1 bg-neutral-100 dark:bg-surface-2 p-1 rounded-lg border border-neutral-200 dark:border-neutral-800 shrink-0">
            {navItems.map((item) => {
              const isActive = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setActiveTab(item.id)}
                  className={`min-h-[34px] px-3 text-xs font-medium rounded-md flex items-center gap-1.5 transition-all whitespace-nowrap shrink-0 ${
                    isActive
                      ? 'bg-white dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100 shadow-xs border border-neutral-200 dark:border-neutral-700/60'
                      : 'text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-200 hover:bg-neutral-200/50 dark:hover:bg-surface-3/50'
                  }`}
                >
                  {item.icon}
                  <span>{item.label}</span>
                </button>
              );
            })}
          </nav>

          {/* 右侧工具 */}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setIsDark(!isDark)}
              className="min-w-[40px] min-h-[40px] flex items-center justify-center rounded-md text-neutral-500 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 hover:bg-neutral-100 dark:hover:bg-surface-2 border border-transparent hover:border-neutral-200 dark:hover:border-neutral-800 transition-colors"
              title={isDark ? '切换至浅色模式' : '切换至深色模式'}
            >
              {isDark ? <Sun className="w-4 h-4 align-icon" /> : <Moon className="w-4 h-4 align-icon" />}
            </button>

            {/* 当前用户与退出登录 */}
            <div className="flex items-center gap-1 pl-2 border-l border-neutral-200 dark:border-neutral-800">
              <span className="hidden sm:inline-flex items-center gap-1 px-2.5 py-1 text-xs font-mono font-medium rounded bg-neutral-100 dark:bg-surface-2 text-neutral-700 dark:text-neutral-300">
                <UserCheck className="w-3.5 h-3.5 text-cyan-600 dark:text-cyan-400 align-icon" />
                {currentUser}
              </span>
              <button
                type="button"
                onClick={handleLogout}
                className="min-w-[40px] min-h-[40px] flex items-center justify-center text-neutral-500 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-950/20 rounded-md transition-colors"
                title="退出登录"
              >
                <LogOut className="w-4 h-4 align-icon" />
              </button>
            </div>
          </div>
        </div>

        {/* 平板及移动端横向滚动 Tab 栏: 宽 <1280px 时平滑左右滑动，按钮绝不折行变形 */}
        <div className="xl:hidden flex overflow-x-auto px-3 sm:px-4 py-2 border-t border-neutral-200 dark:border-neutral-800/60 gap-1.5 bg-neutral-50/90 dark:bg-surface-1/95 backdrop-blur-md no-scrollbar">
          {navItems.map((item) => {
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => setActiveTab(item.id)}
                className={`whitespace-nowrap min-h-[36px] px-3.5 text-xs font-medium rounded-md flex items-center gap-1.5 shrink-0 transition-colors ${
                  isActive
                    ? 'bg-white dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100 shadow-xs border border-neutral-300 dark:border-neutral-700/60'
                    : 'text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-200 hover:bg-neutral-200/50 dark:hover:bg-surface-3/50'
                }`}
              >
                {item.icon}
                <span>{item.label}</span>
              </button>
            );
          })}
        </div>
      </header>

      {/* 主体内容视图 */}
      <main className="flex-1 max-w-[1700px] w-full mx-auto px-4 sm:px-6 lg:px-8 py-6">
        {activeTab === 'overview' && <OverviewPage onNavigate={setActiveTab} />}
        {activeTab === 'pipeline' && <PipelinePage />}
        {activeTab === 'querylog' && <QueryLogPage onNavigate={setActiveTab} />}
        {activeTab === 'filtering' && <FilteringPage />}
        {activeTab === 'routing' && <RoutingPage />}
        {activeTab === 'upstreams' && <UpstreamsPage />}
        {activeTab === 'rewrites' && <RewritesPage />}
        {activeTab === 'settings' && <SettingsPage />}
      </main>
    </div>
  );
}
