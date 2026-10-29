import React, { useEffect, useState } from 'react';
import {
  Activity,
  ShieldCheck,
  Database,
  Zap,
  Clock,
  Server,
  ArrowUpRight,
  RefreshCw,
} from 'lucide-react';
import { api, MetricsSnapshot, StatusResponse } from '../lib/api';

export interface OverviewPageProps {
  onNavigate: (tab: string) => void;
}

export const OverviewPage: React.FC<OverviewPageProps> = ({ onNavigate }) => {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [metrics, setMetrics] = useState<MetricsSnapshot | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchData = async () => {
    try {
      const [s, m] = await Promise.all([api.getStatus(), api.getMetrics()]);
      setStatus(s);
      setMetrics(m);
    } catch (e) {
      console.error('Failed to fetch overview data', e);
    } finally {
      setLoading(false);
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
      {/* 顶部服务状态横幅 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-4 rounded-lg bg-surface-1 border border-neutral-800">
        <div className="flex items-center gap-3">
          <div className="relative flex items-center justify-center w-3 h-3">
            <span className="absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75 animate-ping" />
            <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-neutral-100">NexDNS 统一内核运行中</span>
              <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-emerald-950/60 text-emerald-400 border border-emerald-800/40">
                v{status?.version || '0.1.0'}
              </span>
            </div>
            <p className="text-xs text-neutral-400 mt-0.5">
              UDP/TCP 端口: <span className="tabular-nums font-mono text-neutral-300">{status?.dns_port || 53}</span>
              <span className="mx-2 text-neutral-700">|</span>
              Web 端口: <span className="tabular-nums font-mono text-neutral-300">{status?.web_port || 3000}</span>
              <span className="mx-2 text-neutral-700">|</span>
              运行时长: <span className="tabular-nums text-neutral-300">{status ? formatUptime(status.uptime_seconds) : '-'}</span>
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={fetchData}
            className="inline-flex items-center gap-1.5 min-h-[36px] px-3 text-xs text-neutral-300 hover:text-neutral-100 bg-surface-2 border border-neutral-700 rounded-md transition-colors"
          >
            <RefreshCw className="w-3.5 h-3.5 align-icon" />
            刷新状态
          </button>
          <button
            type="button"
            onClick={() => onNavigate('pipeline')}
            className="inline-flex items-center gap-1.5 min-h-[36px] px-3 text-xs text-cyan-300 hover:text-cyan-200 bg-cyan-950/40 border border-cyan-800/60 rounded-md transition-colors"
          >
            查看流水线
            <ArrowUpRight className="w-3.5 h-3.5 align-icon" />
          </button>
        </div>
      </div>

      {/* 核心指标 KPI 卡片矩阵 */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between text-neutral-400 text-xs font-medium">
            <span>今日查询总量</span>
            <Activity className="w-4 h-4 text-cyan-400 align-icon" />
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-100 tabular-nums">
            {metrics?.total_queries.toLocaleString() ?? 0}
          </div>
          <div className="mt-2 text-xs text-neutral-400 flex items-center justify-between">
            <span>实时 QPS</span>
            <span className="font-mono text-cyan-400 font-semibold tabular-nums">
              {metrics?.qps.toFixed(1) ?? '0.0'} q/s
            </span>
          </div>
        </div>

        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between text-neutral-400 text-xs font-medium">
            <span>已拦截威胁与广告</span>
            <ShieldCheck className="w-4 h-4 text-rose-400 align-icon" />
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-100 tabular-nums">
            {metrics?.blocked_queries.toLocaleString() ?? 0}
          </div>
          <div className="mt-2 text-xs text-neutral-400 flex items-center justify-between">
            <span>拦截比率</span>
            <span className="font-mono text-rose-400 font-semibold tabular-nums">
              {metrics?.block_rate.toFixed(1) ?? '0.0'}%
            </span>
          </div>
        </div>

        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between text-neutral-400 text-xs font-medium">
            <span>SmartDNS 智能缓存</span>
            <Database className="w-4 h-4 text-amber-400 align-icon" />
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-100 tabular-nums">
            {metrics?.cached_queries.toLocaleString() ?? 0}
          </div>
          <div className="mt-2 text-xs text-neutral-400 flex items-center justify-between">
            <span>缓存命中率</span>
            <span className="font-mono text-amber-400 font-semibold tabular-nums">
              {metrics?.cache_hit_rate.toFixed(1) ?? '0.0'}%
            </span>
          </div>
        </div>

        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between text-neutral-400 text-xs font-medium">
            <span>平均解析时延</span>
            <Clock className="w-4 h-4 text-emerald-400 align-icon" />
          </div>
          <div className="mt-3 text-2xl font-semibold text-neutral-100 tabular-nums">
            {metrics?.avg_duration_ms ? `${metrics.avg_duration_ms.toFixed(2)} ms` : '< 1 ms'}
          </div>
          <div className="mt-2 text-xs text-neutral-400 flex items-center justify-between">
            <span>防污染状态</span>
            <span className="font-mono text-emerald-400 font-semibold">
              {status?.anti_pollution ? '已启用' : '未开启'}
            </span>
          </div>
        </div>
      </div>

      {/* 下方榜单：Top 域名、Top 拦截、Top 客户端 */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Top 请求域名 */}
        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between pb-3 border-b border-neutral-800 mb-4">
            <h4 className="text-sm font-semibold text-neutral-200">高频查询域名</h4>
            <button
              type="button"
              onClick={() => onNavigate('querylog')}
              className="text-xs text-cyan-400 hover:text-cyan-300 font-medium"
            >
              查看日志
            </button>
          </div>
          <div className="space-y-3">
            {metrics?.top_domains && metrics.top_domains.length > 0 ? (
              metrics.top_domains.map(([domain, count], idx) => {
                const maxCount = metrics.top_domains[0][1] || 1;
                const pct = Math.round((count / maxCount) * 100);
                return (
                  <div key={domain} className="group">
                    <div className="flex items-center justify-between text-xs mb-1">
                      <span className="font-mono text-neutral-300 truncate max-w-[190px]">
                        {idx + 1}. {domain}
                      </span>
                      <span className="text-neutral-400 font-mono tabular-nums">{count}</span>
                    </div>
                    <div className="w-full bg-surface-3 h-1.5 rounded-full overflow-hidden">
                      <div
                        className="bg-cyan-500 h-full rounded-full transition-all duration-300"
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                  </div>
                );
              })
            ) : (
              <p className="text-xs text-neutral-500 py-6 text-center">暂无查询统计数据</p>
            )}
          </div>
        </div>

        {/* Top 拦截域名 */}
        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between pb-3 border-b border-neutral-800 mb-4">
            <h4 className="text-sm font-semibold text-neutral-200">高频拦截目标</h4>
            <button
              type="button"
              onClick={() => onNavigate('filtering')}
              className="text-xs text-rose-400 hover:text-rose-300 font-medium"
            >
              规则库
            </button>
          </div>
          <div className="space-y-3">
            {metrics?.top_blocked && metrics.top_blocked.length > 0 ? (
              metrics.top_blocked.map(([domain, count], idx) => {
                const maxCount = metrics.top_blocked[0][1] || 1;
                const pct = Math.round((count / maxCount) * 100);
                return (
                  <div key={domain} className="group">
                    <div className="flex items-center justify-between text-xs mb-1">
                      <span className="font-mono text-neutral-300 truncate max-w-[190px]">
                        {idx + 1}. {domain}
                      </span>
                      <span className="text-neutral-400 font-mono tabular-nums">{count}</span>
                    </div>
                    <div className="w-full bg-surface-3 h-1.5 rounded-full overflow-hidden">
                      <div
                        className="bg-rose-500 h-full rounded-full transition-all duration-300"
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                  </div>
                );
              })
            ) : (
              <p className="text-xs text-neutral-500 py-6 text-center">当前未命中拦截规则</p>
            )}
          </div>
        </div>

        {/* Top 活跃客户端 */}
        <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
          <div className="flex items-center justify-between pb-3 border-b border-neutral-800 mb-4">
            <h4 className="text-sm font-semibold text-neutral-200">活跃客户端 IP</h4>
            <span className="text-xs text-neutral-500">内网来源</span>
          </div>
          <div className="space-y-3">
            {metrics?.top_clients && metrics.top_clients.length > 0 ? (
              metrics.top_clients.map(([client, count], idx) => {
                const maxCount = metrics.top_clients[0][1] || 1;
                const pct = Math.round((count / maxCount) * 100);
                return (
                  <div key={client} className="group">
                    <div className="flex items-center justify-between text-xs mb-1">
                      <span className="font-mono text-neutral-300 truncate max-w-[190px]">
                        {idx + 1}. {client}
                      </span>
                      <span className="text-neutral-400 font-mono tabular-nums">{count}</span>
                    </div>
                    <div className="w-full bg-surface-3 h-1.5 rounded-full overflow-hidden">
                      <div
                        className="bg-emerald-500 h-full rounded-full transition-all duration-300"
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                  </div>
                );
              })
            ) : (
              <p className="text-xs text-neutral-500 py-6 text-center">暂无客户端记录</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
