import React, { useEffect, useState } from 'react';
import {
  Activity,
  SlidersHorizontal,
  FileText,
  Shield,
  Server,
  Settings,
  Moon,
  Sun,
  Terminal,
  FileCode,
} from 'lucide-react';
import { OverviewPage } from './pages/OverviewPage';
import { PipelinePage } from './pages/PipelinePage';
import { QueryLogPage } from './pages/QueryLogPage';
import { FilteringPage } from './pages/FilteringPage';
import { RoutingPage } from './pages/RoutingPage';
import { UpstreamsPage } from './pages/UpstreamsPage';
import { SettingsPage } from './pages/SettingsPage';

export function App() {
  // 持久化 Tab 状态至 URL，避免页面刷新画面抖动
  const [activeTab, setActiveTab] = useState<string>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get('tab') || 'overview';
  });
  const [isDark, setIsDark] = useState<boolean>(true);

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

  const navItems = [
    { id: 'overview', label: '工作台概览', icon: <Activity className="w-4 h-4 align-icon" /> },
    { id: 'querylog', label: '实时查询日志', icon: <FileText className="w-4 h-4 align-icon" /> },
    { id: 'filtering', label: '拦截与规则库', icon: <Shield className="w-4 h-4 align-icon" /> },
    { id: 'routing', label: '分流规则集', icon: <FileCode className="w-4 h-4 align-icon" /> },
    { id: 'pipeline', label: '流水线拓扑', icon: <SlidersHorizontal className="w-4 h-4 align-icon" /> },
    { id: 'upstreams', label: '上游与测速优选', icon: <Server className="w-4 h-4 align-icon" /> },
    { id: 'settings', label: '重写与系统设置', icon: <Settings className="w-4 h-4 align-icon" /> },
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
          <nav className="hidden md:flex items-center gap-1 bg-neutral-100 dark:bg-surface-2 p-1 rounded-lg border border-neutral-200 dark:border-neutral-800">
            {navItems.map((item) => {
              const isActive = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setActiveTab(item.id)}
                  className={`min-h-[34px] px-3 text-xs font-medium rounded-md flex items-center gap-1.5 transition-all ${
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
          </div>
        </div>

        {/* 移动端横向滚动 Tab */}
        <div className="md:hidden flex overflow-x-auto px-4 py-2 border-t border-neutral-200 dark:border-neutral-800/60 gap-1 bg-white dark:bg-surface-1">
          {navItems.map((item) => {
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => setActiveTab(item.id)}
                className={`whitespace-nowrap px-3 py-1.5 text-xs font-medium rounded-md flex items-center gap-1.5 shrink-0 ${
                  isActive
                    ? 'bg-neutral-100 dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100 border border-neutral-300 dark:border-neutral-700/60'
                    : 'text-neutral-500 dark:text-neutral-400'
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
        {activeTab === 'querylog' && <QueryLogPage />}
        {activeTab === 'filtering' && <FilteringPage />}
        {activeTab === 'routing' && <RoutingPage />}
        {activeTab === 'upstreams' && <UpstreamsPage />}
        {activeTab === 'settings' && <SettingsPage />}
      </main>
    </div>
  );
}
