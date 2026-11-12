import React, { useEffect, useState } from 'react';
import {
  Activity,
  ShieldCheck,
  Database,
  Clock,
  RefreshCw,
  RotateCcw,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { api, MetricsSnapshot, StatusResponse } from '../lib/api';
import { ActivityChart } from '../components/ui/ActivityChart';
import { DnsCacheStatusTable } from '../components/ui/DnsCacheStatusTable';
import { Popconfirm } from '../components/ui/Popconfirm';
import { Snackbar } from '../components/ui/Snackbar';
import { isPrivateIp } from '../lib/net';

export interface OverviewPageProps {
  onNavigate: (tab: string) => void;
}

export const OverviewPage: React.FC<OverviewPageProps> = ({ onNavigate }) => {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [metrics, setMetrics] = useState<MetricsSnapshot | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [resettingScope, setResettingScope] = useState<string | null>(null);
  const [cacheRefreshKey, setCacheRefreshKey] = useState(0);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);
  const [domainsPage, setDomainsPage] = useState(1);
  const [blockedPage, setBlockedPage] = useState(1);
  const [clientsPage, setClientsPage] = useState(1);
  const cardPageSize = 5;
  const MAX_TOP_LIMIT = 50;

  const fetchData = async () => {
    try {
      const [s, m] = await Promise.all([api.getStatus(), api.getMetrics()]);
      setStatus(s);
      setMetrics(m);
    } catch (e) {
      console.error('Failed to fetch overview data', e);
    }
  };

  const handleManualRefresh = async () => {
    setIsRefreshing(true);
    setCacheRefreshKey((k) => k + 1);
    await fetchData();
    setTimeout(() => setIsRefreshing(false), 700);
  };

  const handleResetMetrics = async (scope: string, label: string) => {
    setResettingScope(scope);
    try {
      await api.resetMetrics(scope);
      if (scope === 'all' || scope === 'top_domains') setDomainsPage(1);
      if (scope === 'all' || scope === 'top_blocked') setBlockedPage(1);
      if (scope === 'all' || scope === 'top_clients') setClientsPage(1);
      if (scope === 'all' || scope === 'cache_stats') setCacheRefreshKey((k) => k + 1);
      await fetchData();
      setSnackbarMessage(`已重置${label}`);
    } catch (e) {
      console.error('Failed to reset metrics', e);
      setSnackbarMessage(`重置${label}失败`);
    } finally {
      setResettingScope(null);
    }
  };

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 3000);
    return () => clearInterval(interval);
  }, []);

  const formatUptime = (secs: number) => {
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (d > 0) return `${d}天 ${h}时 ${m}分`;
    if (h > 0) return `${h}时 ${m}分 ${s}秒`;
    return `${m}分 ${s}秒`;
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部服务状态 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3.5 p-4 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex items-start sm:items-center gap-3 min-w-0">
          <div className="relative flex items-center justify-center w-3 h-3 mt-1 sm:mt-0 shrink-0">
            <span className="absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75 animate-ping" />
            <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 whitespace-nowrap">服务正常运行中</span>
              <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800/40 whitespace-nowrap shrink-0">
                v{status?.version || '0.1.9'}
              </span>
            </div>
            <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">
              <span className="whitespace-nowrap">
                DNS 端口: <span className="tabular-nums font-mono font-medium text-neutral-700 dark:text-neutral-300">{status?.dns_port || 53}</span>
              </span>
              <span className="text-neutral-300 dark:text-neutral-700">|</span>
              <span className="whitespace-nowrap">
                Web 端口: <span className="tabular-nums font-mono font-medium text-neutral-700 dark:text-neutral-300">{status?.web_port || 3000}</span>
              </span>
              <span className="text-neutral-300 dark:text-neutral-700">|</span>
              <span className="whitespace-nowrap">
                运行时长: <span className="tabular-nums text-neutral-700 dark:text-neutral-300">{status ? formatUptime(status.uptime_seconds) : '-'}</span>
              </span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <Popconfirm
            title="确认重置首页全部近 24 小时统计数据？"
            actionText="确认重置"
            onConfirm={() => handleResetMetrics('all', '首页全部近 24 小时统计数据')}
          >
            <button
              type="button"
              disabled={resettingScope === 'all'}
              className="inline-flex items-center justify-center gap-1.5 min-h-[36px] px-3.5 text-xs whitespace-nowrap shrink-0 text-neutral-700 dark:text-neutral-300 hover:text-rose-600 dark:hover:text-rose-400 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-rose-300 dark:hover:border-rose-800 rounded-md transition-all active:scale-95 disabled:opacity-75"
            >
              <RotateCcw className="w-3.5 h-3.5 align-icon shrink-0" />
              重置全部统计
            </button>
          </Popconfirm>
          <button
            type="button"
            onClick={handleManualRefresh}
            disabled={isRefreshing}
            className="inline-flex items-center justify-center gap-1.5 min-h-[36px] px-3.5 text-xs whitespace-nowrap shrink-0 text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-all active:scale-95 disabled:opacity-75"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon shrink-0 ${isRefreshing ? 'animate-spin' : ''}`} />
            刷新
          </button>
        </div>
      </div>

      {/* 核心指标 KPI 卡片矩阵 (近 24 小时) */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
          <div className="flex items-center justify-between gap-2 text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span className="min-w-0 truncate">近 24 小时查询总量</span>
            <div className="flex items-center gap-1.5 shrink-0">
              <Popconfirm
                title="确认重置近 24 小时查询总量统计？"
                actionText="确认重置"
                onConfirm={() => handleResetMetrics('kpi_total', '近 24 小时查询总量统计')}
              >
                <button
                  type="button"
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[11px] whitespace-nowrap shrink-0 text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 transition-colors"
                  title="重置查询总量统计"
                >
                  <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                  重置
                </button>
              </Popconfirm>
              <Activity className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon shrink-0" />
            </div>
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-900 dark:text-neutral-100 tabular-nums">
            {metrics?.total_queries.toLocaleString() ?? 0}
          </div>
          <div className="mt-2 text-xs text-neutral-500 dark:text-neutral-400 flex items-center justify-between">
            <span>实时 QPS</span>
            <span className="font-mono text-cyan-600 dark:text-cyan-400 font-semibold tabular-nums">
              {metrics?.qps.toFixed(1) ?? '0.0'} q/s
            </span>
          </div>
        </div>

        <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
          <div className="flex items-center justify-between gap-2 text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span className="min-w-0 truncate">近 24 小时已拦截威胁与广告</span>
            <div className="flex items-center gap-1.5 shrink-0">
              <Popconfirm
                title="确认重置近 24 小时拦截统计？"
                actionText="确认重置"
                onConfirm={() => handleResetMetrics('kpi_blocked', '近 24 小时拦截统计')}
              >
                <button
                  type="button"
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[11px] whitespace-nowrap shrink-0 text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 transition-colors"
                  title="重置拦截统计"
                >
                  <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                  重置
                </button>
              </Popconfirm>
              <ShieldCheck className="w-4 h-4 text-rose-500 dark:text-rose-400 align-icon shrink-0" />
            </div>
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-900 dark:text-neutral-100 tabular-nums">
            {metrics?.blocked_queries.toLocaleString() ?? 0}
          </div>
          <div className="mt-2 text-xs text-neutral-500 dark:text-neutral-400 flex items-center justify-between">
            <span>拦截比率</span>
            <span className="font-mono text-rose-600 dark:text-rose-400 font-semibold tabular-nums">
              {metrics?.block_rate.toFixed(1) ?? '0.0'}%
            </span>
          </div>
        </div>

        <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
          <div className="flex items-center justify-between gap-2 text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span className="min-w-0 truncate">近 24 小时智能内存缓存</span>
            <div className="flex items-center gap-1.5 shrink-0">
              <Popconfirm
                title="确认重置近 24 小时缓存命中统计？"
                actionText="确认重置"
                onConfirm={() => handleResetMetrics('kpi_cached', '近 24 小时缓存命中统计')}
              >
                <button
                  type="button"
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[11px] whitespace-nowrap shrink-0 text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 transition-colors"
                  title="重置缓存命中统计"
                >
                  <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                  重置
                </button>
              </Popconfirm>
              <Database className="w-4 h-4 text-amber-500 dark:text-amber-400 align-icon shrink-0" />
            </div>
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-900 dark:text-neutral-100 tabular-nums">
            {metrics?.cached_queries.toLocaleString() ?? 0}
          </div>
          <div className="mt-2 text-xs text-neutral-500 dark:text-neutral-400 flex items-center justify-between">
            <span>缓存命中率</span>
            <span className="font-mono text-amber-600 dark:text-amber-400 font-semibold tabular-nums">
              {metrics?.cache_hit_rate.toFixed(1) ?? '0.0'}%
            </span>
          </div>
        </div>

        <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
          <div className="flex items-center justify-between gap-2 text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span className="min-w-0 truncate">近 24 小时平均解析时延</span>
            <div className="flex items-center gap-1.5 shrink-0">
              <Popconfirm
                title="确认重置近 24 小时平均解析时延统计？"
                actionText="确认重置"
                onConfirm={() => handleResetMetrics('kpi_latency', '近 24 小时平均解析时延统计')}
              >
                <button
                  type="button"
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[11px] whitespace-nowrap shrink-0 text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 transition-colors"
                  title="重置平均时延统计"
                >
                  <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                  重置
                </button>
              </Popconfirm>
              <Clock className="w-4 h-4 text-emerald-500 dark:text-emerald-400 align-icon shrink-0" />
            </div>
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-900 dark:text-neutral-100 tabular-nums">
            {metrics?.avg_duration_ms ? `${metrics.avg_duration_ms.toFixed(2)} ms` : '< 1 ms'}
          </div>
          <div className="mt-2 text-xs text-neutral-500 dark:text-neutral-400 flex items-center justify-between">
            <span>防污染状态</span>
            <span className="font-mono text-emerald-600 dark:text-emerald-400 font-semibold">
              {status?.anti_pollution ? '已启用' : '未开启'}
            </span>
          </div>
        </div>
      </div>

      {/* DNS 缓存状态监控面板 (SmartDNS 原生级监控) */}
      <DnsCacheStatusTable refreshKey={cacheRefreshKey} onResetSuccess={fetchData} />

      {/* 24 小时动态折线图 */}
      <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2.5 pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
          <div className="min-w-0">
            <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">近 24 小时查询吞吐与拦截趋势</h4>
            <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">按小时统计总查询量与安全拦截量分布</p>
          </div>
          <div className="flex items-center justify-between sm:justify-end gap-2.5 shrink-0">
            <span className="text-xs font-mono text-neutral-400 whitespace-nowrap">时间轴 (24h)</span>
            <Popconfirm
              title="确认重置近 24 小时查询吞吐与拦截趋势数据？"
              actionText="确认重置"
              onConfirm={() => handleResetMetrics('hourly', '近 24 小时趋势统计')}
            >
              <button
                type="button"
                className="inline-flex items-center gap-1 min-h-[28px] px-2.5 text-xs whitespace-nowrap shrink-0 text-neutral-600 dark:text-neutral-300 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 dark:hover:border-neutral-600 rounded transition-colors"
              >
                <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                重置
              </button>
            </Popconfirm>
          </div>
        </div>
        <ActivityChart data={metrics?.hourly_history || []} />
      </div>

      {/* 下方榜单：Top 域名、Top 拦截、Top 客户端 */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Top 请求域名 */}
        {(() => {
          const list = (metrics?.top_domains || []).slice(0, MAX_TOP_LIMIT);
          const totalPages = Math.max(1, Math.ceil(list.length / cardPageSize));
          const pagedList = list.slice((domainsPage - 1) * cardPageSize, domainsPage * cardPageSize);
          const baseIdx = (domainsPage - 1) * cardPageSize;
          const maxCount = list[0] ? list[0][1] : 1;

          return (
            <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 whitespace-nowrap">高频查询域名</h4>
                    <span className="px-1.5 py-0.5 text-[11px] font-normal text-neutral-500 dark:text-neutral-400 bg-neutral-100 dark:bg-surface-2 rounded font-mono whitespace-nowrap shrink-0">
                      24h · {Math.min(list.length, MAX_TOP_LIMIT)}项
                    </span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <button
                      type="button"
                      onClick={() => onNavigate('querylog')}
                      className="text-xs text-cyan-600 dark:text-cyan-400 hover:underline font-medium whitespace-nowrap shrink-0"
                    >
                      查看日志
                    </button>
                    <Popconfirm
                      title="确认重置近 24 小时高频查询域名统计？"
                      actionText="确认重置"
                      onConfirm={() => handleResetMetrics('top_domains', '近 24 小时高频查询域名统计')}
                    >
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 min-h-[26px] px-2 text-xs whitespace-nowrap shrink-0 text-neutral-600 dark:text-neutral-300 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 dark:hover:border-neutral-600 rounded transition-colors"
                        title="重置高频查询域名统计"
                      >
                        <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                        重置
                      </button>
                    </Popconfirm>
                  </div>
                </div>
                <div className="space-y-3">
                  {pagedList.length > 0 ? (
                    pagedList.map(([domain, count], idx) => {
                      const pct = Math.round((count / maxCount) * 100);
                      return (
                        <div key={domain} className="group">
                          <div className="flex items-center justify-between gap-3 text-xs mb-1">
                            <span className="font-mono text-neutral-700 dark:text-neutral-300 truncate min-w-0 flex-1" title={domain}>
                              {baseIdx + idx + 1}. {domain}
                            </span>
                            <span className="text-neutral-500 font-mono tabular-nums shrink-0">{count}</span>
                          </div>
                          <div className="w-full bg-neutral-100 dark:bg-surface-3 h-1.5 rounded-full overflow-hidden">
                            <div
                              className="bg-cyan-500 h-full rounded-full transition-all duration-300"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                        </div>
                      );
                    })
                  ) : (
                    <p className="text-xs text-neutral-400 py-6 text-center">暂无查询统计数据</p>
                  )}
                </div>
              </div>
              {totalPages > 1 && (
                <div className="pt-3 mt-4 border-t border-neutral-100 dark:border-neutral-800/60 flex items-center justify-between text-xs text-neutral-500 dark:text-neutral-400 font-mono">
                  <span className="tabular-nums whitespace-nowrap">第 {domainsPage} / {totalPages} 页</span>
                  <div className="flex items-center gap-1">
                    <button
                      type="button"
                      disabled={domainsPage <= 1}
                      onClick={() => setDomainsPage((p) => Math.max(1, p - 1))}
                      className="min-w-[26px] min-h-[26px] flex items-center justify-center rounded border border-neutral-200 dark:border-neutral-800 bg-neutral-50 dark:bg-surface-2 hover:bg-neutral-100 dark:hover:bg-surface-3 disabled:opacity-30 transition-colors"
                      title="上一页"
                    >
                      <ChevronLeft className="w-3.5 h-3.5 align-icon" />
                    </button>
                    <button
                      type="button"
                      disabled={domainsPage >= totalPages}
                      onClick={() => setDomainsPage((p) => Math.min(totalPages, p + 1))}
                      className="min-w-[26px] min-h-[26px] flex items-center justify-center rounded border border-neutral-200 dark:border-neutral-800 bg-neutral-50 dark:bg-surface-2 hover:bg-neutral-100 dark:hover:bg-surface-3 disabled:opacity-30 transition-colors"
                      title="下一页"
                    >
                      <ChevronRight className="w-3.5 h-3.5 align-icon" />
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })()}

        {/* Top 拦截域名 */}
        {(() => {
          const list = (metrics?.top_blocked || []).slice(0, MAX_TOP_LIMIT);
          const totalPages = Math.max(1, Math.ceil(list.length / cardPageSize));
          const pagedList = list.slice((blockedPage - 1) * cardPageSize, blockedPage * cardPageSize);
          const baseIdx = (blockedPage - 1) * cardPageSize;
          const maxCount = list[0] ? list[0][1] : 1;

          return (
            <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 whitespace-nowrap">高频拦截目标</h4>
                    <span className="px-1.5 py-0.5 text-[11px] font-normal text-neutral-500 dark:text-neutral-400 bg-neutral-100 dark:bg-surface-2 rounded font-mono whitespace-nowrap shrink-0">
                      24h · {Math.min(list.length, MAX_TOP_LIMIT)}项
                    </span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <button
                      type="button"
                      onClick={() => onNavigate('filtering')}
                      className="text-xs text-rose-600 dark:text-rose-400 hover:underline font-medium whitespace-nowrap shrink-0"
                    >
                      规则库
                    </button>
                    <Popconfirm
                      title="确认重置近 24 小时高频拦截目标统计？"
                      actionText="确认重置"
                      onConfirm={() => handleResetMetrics('top_blocked', '近 24 小时高频拦截目标统计')}
                    >
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 min-h-[26px] px-2 text-xs whitespace-nowrap shrink-0 text-neutral-600 dark:text-neutral-300 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 dark:hover:border-neutral-600 rounded transition-colors"
                        title="重置高频拦截目标统计"
                      >
                        <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                        重置
                      </button>
                    </Popconfirm>
                  </div>
                </div>
                <div className="space-y-3">
                  {pagedList.length > 0 ? (
                    pagedList.map(([domain, count], idx) => {
                      const pct = Math.round((count / maxCount) * 100);
                      return (
                        <div key={domain} className="group">
                          <div className="flex items-center justify-between gap-3 text-xs mb-1">
                            <span className="font-mono text-neutral-700 dark:text-neutral-300 truncate min-w-0 flex-1" title={domain}>
                              {baseIdx + idx + 1}. {domain}
                            </span>
                            <span className="text-neutral-500 font-mono tabular-nums shrink-0">{count}</span>
                          </div>
                          <div className="w-full bg-neutral-100 dark:bg-surface-3 h-1.5 rounded-full overflow-hidden">
                            <div
                              className="bg-rose-500 h-full rounded-full transition-all duration-300"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                        </div>
                      );
                    })
                  ) : (
                    <p className="text-xs text-neutral-400 py-6 text-center">当前未命中拦截规则</p>
                  )}
                </div>
              </div>
              {totalPages > 1 && (
                <div className="pt-3 mt-4 border-t border-neutral-100 dark:border-neutral-800/60 flex items-center justify-between text-xs text-neutral-500 dark:text-neutral-400 font-mono">
                  <span className="tabular-nums whitespace-nowrap">第 {blockedPage} / {totalPages} 页</span>
                  <div className="flex items-center gap-1">
                    <button
                      type="button"
                      disabled={blockedPage <= 1}
                      onClick={() => setBlockedPage((p) => Math.max(1, p - 1))}
                      className="min-w-[26px] min-h-[26px] flex items-center justify-center rounded border border-neutral-200 dark:border-neutral-800 bg-neutral-50 dark:bg-surface-2 hover:bg-neutral-100 dark:hover:bg-surface-3 disabled:opacity-30 transition-colors"
                      title="上一页"
                    >
                      <ChevronLeft className="w-3.5 h-3.5 align-icon" />
                    </button>
                    <button
                      type="button"
                      disabled={blockedPage >= totalPages}
                      onClick={() => setBlockedPage((p) => Math.min(totalPages, p + 1))}
                      className="min-w-[26px] min-h-[26px] flex items-center justify-center rounded border border-neutral-200 dark:border-neutral-800 bg-neutral-50 dark:bg-surface-2 hover:bg-neutral-100 dark:hover:bg-surface-3 disabled:opacity-30 transition-colors"
                      title="下一页"
                    >
                      <ChevronRight className="w-3.5 h-3.5 align-icon" />
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })()}

        {/* Top 活跃客户端 */}
        {(() => {
          const list = (metrics?.top_clients || []).slice(0, MAX_TOP_LIMIT);
          const totalPages = Math.max(1, Math.ceil(list.length / cardPageSize));
          const pagedList = list.slice((clientsPage - 1) * cardPageSize, clientsPage * cardPageSize);
          const baseIdx = (clientsPage - 1) * cardPageSize;
          const maxCount = list[0] ? list[0][1] : 1;

          return (
            <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 whitespace-nowrap">活跃客户端 IP</h4>
                    <span className="px-1.5 py-0.5 text-[11px] font-normal text-neutral-500 dark:text-neutral-400 bg-neutral-100 dark:bg-surface-2 rounded font-mono whitespace-nowrap shrink-0">
                      24h · {Math.min(list.length, MAX_TOP_LIMIT)}项
                    </span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <Popconfirm
                      title="确认重置近 24 小时活跃客户端统计？"
                      actionText="确认重置"
                      onConfirm={() => handleResetMetrics('top_clients', '近 24 小时活跃客户端统计')}
                    >
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 min-h-[26px] px-2 text-xs whitespace-nowrap shrink-0 text-neutral-600 dark:text-neutral-300 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 dark:hover:border-neutral-600 rounded transition-colors"
                        title="重置活跃客户端统计"
                      >
                        <RotateCcw className="w-3 h-3 align-icon shrink-0" />
                        重置
                      </button>
                    </Popconfirm>
                  </div>
                </div>
                <div className="space-y-3">
                  {pagedList.length > 0 ? (
                    pagedList.map(([client, count], idx) => {
                      const pct = Math.round((count / maxCount) * 100);
                      const privateSource = isPrivateIp(client);
                      return (
                        <div key={client} className="group">
                          <div className="flex items-center justify-between gap-3 text-xs mb-1">
                            <span className="flex items-center gap-1.5 min-w-0 flex-1">
                              <span className="font-mono text-neutral-700 dark:text-neutral-300 truncate min-w-0" title={client}>
                                {baseIdx + idx + 1}. {client}
                              </span>
                              <span
                                className={`shrink-0 whitespace-nowrap px-1 py-0.5 text-[10px] rounded border ${
                                  privateSource
                                    ? 'bg-sky-50 dark:bg-sky-950/60 text-sky-700 dark:text-sky-400 border-sky-200 dark:border-sky-800/40'
                                    : 'bg-amber-50 dark:bg-amber-950/60 text-amber-700 dark:text-amber-400 border-amber-200 dark:border-amber-800/40'
                                }`}
                              >
                                {privateSource ? '内网' : '公网'}
                              </span>
                            </span>
                            <span className="text-neutral-500 font-mono tabular-nums shrink-0">{count}</span>
                          </div>
                          <div className="w-full bg-neutral-100 dark:bg-surface-3 h-1.5 rounded-full overflow-hidden">
                            <div
                              className="bg-emerald-500 h-full rounded-full transition-all duration-300"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                        </div>
                      );
                    })
                  ) : (
                    <p className="text-xs text-neutral-400 py-6 text-center">暂无客户端记录</p>
                  )}
                </div>
              </div>
              {totalPages > 1 && (
                <div className="pt-3 mt-4 border-t border-neutral-100 dark:border-neutral-800/60 flex items-center justify-between text-xs text-neutral-500 dark:text-neutral-400 font-mono">
                  <span className="tabular-nums whitespace-nowrap">第 {clientsPage} / {totalPages} 页</span>
                  <div className="flex items-center gap-1">
                    <button
                      type="button"
                      disabled={clientsPage <= 1}
                      onClick={() => setClientsPage((p) => Math.max(1, p - 1))}
                      className="min-w-[26px] min-h-[26px] flex items-center justify-center rounded border border-neutral-200 dark:border-neutral-800 bg-neutral-50 dark:bg-surface-2 hover:bg-neutral-100 dark:hover:bg-surface-3 disabled:opacity-30 transition-colors"
                      title="上一页"
                    >
                      <ChevronLeft className="w-3.5 h-3.5 align-icon" />
                    </button>
                    <button
                      type="button"
                      disabled={clientsPage >= totalPages}
                      onClick={() => setClientsPage((p) => Math.min(totalPages, p + 1))}
                      className="min-w-[26px] min-h-[26px] flex items-center justify-center rounded border border-neutral-200 dark:border-neutral-800 bg-neutral-50 dark:bg-surface-2 hover:bg-neutral-100 dark:hover:bg-surface-3 disabled:opacity-30 transition-colors"
                      title="下一页"
                    >
                      <ChevronRight className="w-3.5 h-3.5 align-icon" />
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })()}
        </div>

      {snackbarMessage && (
        <Snackbar message={snackbarMessage} onClose={() => setSnackbarMessage(null)} />
      )}
      </div>
  );
};
