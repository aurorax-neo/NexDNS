import React, { useEffect, useState } from 'react';
import {
  Activity,
  ShieldCheck,
  Database,
  Clock,
  ArrowUpRight,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { api, MetricsSnapshot, StatusResponse } from '../lib/api';
import { ActivityChart } from '../components/ui/ActivityChart';

export interface OverviewPageProps {
  onNavigate: (tab: string) => void;
}

export const OverviewPage: React.FC<OverviewPageProps> = ({ onNavigate }) => {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [metrics, setMetrics] = useState<MetricsSnapshot | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [domainsPage, setDomainsPage] = useState(1);
  const [blockedPage, setBlockedPage] = useState(1);
  const [clientsPage, setClientsPage] = useState(1);
  const cardPageSize = 5;

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
    await fetchData();
    setTimeout(() => setIsRefreshing(false), 700);
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
      <div className="flex flex-wrap items-center justify-between gap-4 p-4 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex items-center gap-3">
          <div className="relative flex items-center justify-center w-3 h-3">
            <span className="absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75 animate-ping" />
            <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">服务正常运行中</span>
              <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800/40">
                v{status?.version || '0.1.0'}
              </span>
            </div>
            <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">
              DNS 端口: <span className="tabular-nums font-mono font-medium text-neutral-700 dark:text-neutral-300">{status?.dns_port || 53}</span>
              <span className="mx-2 text-neutral-300 dark:text-neutral-700">|</span>
              Web 端口: <span className="tabular-nums font-mono font-medium text-neutral-700 dark:text-neutral-300">{status?.web_port || 3000}</span>
              <span className="mx-2 text-neutral-300 dark:text-neutral-700">|</span>
              运行时长: <span className="tabular-nums text-neutral-700 dark:text-neutral-300">{status ? formatUptime(status.uptime_seconds) : '-'}</span>
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleManualRefresh}
            disabled={isRefreshing}
            className="inline-flex items-center gap-1.5 min-h-[36px] px-3.5 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-all active:scale-95 disabled:opacity-75"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${isRefreshing ? 'animate-spin' : ''}`} />
            刷新
          </button>
        </div>
      </div>

      {/* 核心指标 KPI 卡片矩阵 */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
          <div className="flex items-center justify-between text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span>今日查询总量</span>
            <Activity className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
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
          <div className="flex items-center justify-between text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span>已拦截威胁与广告</span>
            <ShieldCheck className="w-4 h-4 text-rose-500 dark:text-rose-400 align-icon" />
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
          <div className="flex items-center justify-between text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span>智能内存缓存</span>
            <Database className="w-4 h-4 text-amber-500 dark:text-amber-400 align-icon" />
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
          <div className="flex items-center justify-between text-neutral-500 dark:text-neutral-400 text-xs font-medium">
            <span>平均解析时延</span>
            <Clock className="w-4 h-4 text-emerald-500 dark:text-emerald-400 align-icon" />
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

      {/* 24 小时动态折线图 */}
      <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex items-center justify-between pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
          <div>
            <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">近 24 小时查询吞吐与拦截趋势</h4>
            <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">按小时统计总查询量与安全拦截量分布</p>
          </div>
          <span className="text-xs font-mono text-neutral-400">时间轴 (24h)</span>
        </div>
        <ActivityChart data={metrics?.hourly_history || []} />
      </div>

      {/* 下方榜单：Top 域名、Top 拦截、Top 客户端 */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Top 请求域名 */}
        {(() => {
          const list = metrics?.top_domains || [];
          const totalPages = Math.max(1, Math.ceil(list.length / cardPageSize));
          const pagedList = list.slice((domainsPage - 1) * cardPageSize, domainsPage * cardPageSize);
          const baseIdx = (domainsPage - 1) * cardPageSize;
          const maxCount = list[0] ? list[0][1] : 1;

          return (
            <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
                  <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">高频查询域名</h4>
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => onNavigate('querylog')}
                      className="text-xs text-cyan-600 dark:text-cyan-400 hover:underline font-medium mr-1"
                    >
                      查看日志
                    </button>
                    {totalPages > 1 && (
                      <div className="flex items-center gap-1 text-xs text-neutral-500 font-mono">
                        <button
                          type="button"
                          disabled={domainsPage <= 1}
                          onClick={() => setDomainsPage((p) => Math.max(1, p - 1))}
                          className="p-1 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 disabled:opacity-30 transition-colors"
                        >
                          <ChevronLeft className="w-3.5 h-3.5 align-icon" />
                        </button>
                        <span className="tabular-nums">{domainsPage}/{totalPages}</span>
                        <button
                          type="button"
                          disabled={domainsPage >= totalPages}
                          onClick={() => setDomainsPage((p) => Math.min(totalPages, p + 1))}
                          className="p-1 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 disabled:opacity-30 transition-colors"
                        >
                          <ChevronRight className="w-3.5 h-3.5 align-icon" />
                        </button>
                      </div>
                    )}
                  </div>
                </div>
                <div className="space-y-3">
                  {pagedList.length > 0 ? (
                    pagedList.map(([domain, count], idx) => {
                      const pct = Math.round((count / maxCount) * 100);
                      return (
                        <div key={domain} className="group">
                          <div className="flex items-center justify-between text-xs mb-1">
                            <span className="font-mono text-neutral-700 dark:text-neutral-300 truncate max-w-[190px]">
                              {baseIdx + idx + 1}. {domain}
                            </span>
                            <span className="text-neutral-500 font-mono tabular-nums">{count}</span>
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
            </div>
          );
        })()}

        {/* Top 拦截域名 */}
        {(() => {
          const list = metrics?.top_blocked || [];
          const totalPages = Math.max(1, Math.ceil(list.length / cardPageSize));
          const pagedList = list.slice((blockedPage - 1) * cardPageSize, blockedPage * cardPageSize);
          const baseIdx = (blockedPage - 1) * cardPageSize;
          const maxCount = list[0] ? list[0][1] : 1;

          return (
            <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
                  <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">高频拦截目标</h4>
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => onNavigate('filtering')}
                      className="text-xs text-rose-600 dark:text-rose-400 hover:underline font-medium mr-1"
                    >
                      规则库
                    </button>
                    {totalPages > 1 && (
                      <div className="flex items-center gap-1 text-xs text-neutral-500 font-mono">
                        <button
                          type="button"
                          disabled={blockedPage <= 1}
                          onClick={() => setBlockedPage((p) => Math.max(1, p - 1))}
                          className="p-1 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 disabled:opacity-30 transition-colors"
                        >
                          <ChevronLeft className="w-3.5 h-3.5 align-icon" />
                        </button>
                        <span className="tabular-nums">{blockedPage}/{totalPages}</span>
                        <button
                          type="button"
                          disabled={blockedPage >= totalPages}
                          onClick={() => setBlockedPage((p) => Math.min(totalPages, p + 1))}
                          className="p-1 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 disabled:opacity-30 transition-colors"
                        >
                          <ChevronRight className="w-3.5 h-3.5 align-icon" />
                        </button>
                      </div>
                    )}
                  </div>
                </div>
                <div className="space-y-3">
                  {pagedList.length > 0 ? (
                    pagedList.map(([domain, count], idx) => {
                      const pct = Math.round((count / maxCount) * 100);
                      return (
                        <div key={domain} className="group">
                          <div className="flex items-center justify-between text-xs mb-1">
                            <span className="font-mono text-neutral-700 dark:text-neutral-300 truncate max-w-[190px]">
                              {baseIdx + idx + 1}. {domain}
                            </span>
                            <span className="text-neutral-500 font-mono tabular-nums">{count}</span>
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
            </div>
          );
        })()}

        {/* Top 活跃客户端 */}
        {(() => {
          const list = metrics?.top_clients || [];
          const totalPages = Math.max(1, Math.ceil(list.length / cardPageSize));
          const pagedList = list.slice((clientsPage - 1) * cardPageSize, clientsPage * cardPageSize);
          const baseIdx = (clientsPage - 1) * cardPageSize;
          const maxCount = list[0] ? list[0][1] : 1;

          return (
            <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between pb-3 border-b border-neutral-200 dark:border-neutral-800 mb-4">
                  <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">活跃客户端 IP</h4>
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-neutral-400 mr-1">内网来源</span>
                    {totalPages > 1 && (
                      <div className="flex items-center gap-1 text-xs text-neutral-500 font-mono">
                        <button
                          type="button"
                          disabled={clientsPage <= 1}
                          onClick={() => setClientsPage((p) => Math.max(1, p - 1))}
                          className="p-1 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 disabled:opacity-30 transition-colors"
                        >
                          <ChevronLeft className="w-3.5 h-3.5 align-icon" />
                        </button>
                        <span className="tabular-nums">{clientsPage}/{totalPages}</span>
                        <button
                          type="button"
                          disabled={clientsPage >= totalPages}
                          onClick={() => setClientsPage((p) => Math.min(totalPages, p + 1))}
                          className="p-1 rounded hover:bg-neutral-100 dark:hover:bg-surface-2 disabled:opacity-30 transition-colors"
                        >
                          <ChevronRight className="w-3.5 h-3.5 align-icon" />
                        </button>
                      </div>
                    )}
                  </div>
                </div>
                <div className="space-y-3">
                  {pagedList.length > 0 ? (
                    pagedList.map(([client, count], idx) => {
                      const pct = Math.round((count / maxCount) * 100);
                      return (
                        <div key={client} className="group">
                          <div className="flex items-center justify-between text-xs mb-1">
                            <span className="font-mono text-neutral-700 dark:text-neutral-300 truncate max-w-[190px]">
                              {baseIdx + idx + 1}. {client}
                            </span>
                            <span className="text-neutral-500 font-mono tabular-nums">{count}</span>
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
            </div>
          );
        })()}
        </div>
      </div>
  );
};
