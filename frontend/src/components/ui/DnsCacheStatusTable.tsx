import React, { useEffect, useState } from 'react';
import { api, CacheStatsSnapshot } from '../../lib/api';
import { Popconfirm } from './Popconfirm';

export interface DnsCacheStatusTableProps {
  className?: string;
}

export const DnsCacheStatusTable: React.FC<DnsCacheStatusTableProps> = ({ className = '' }) => {
  const [stats, setStats] = useState<CacheStatsSnapshot | null>(null);
  const [clearing, setClearing] = useState(false);
  const [flushing, setFlushing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const fetchStats = async () => {
    try {
      const data = await api.getCacheStats();
      setStats(data);
    } catch (e) {
      console.error('Failed to fetch cache stats', e);
    }
  };

  useEffect(() => {
    fetchStats();
    const timer = setInterval(fetchStats, 5000);
    return () => clearInterval(timer);
  }, []);

  const handleClear = async () => {
    setClearing(true);
    try {
      await api.clearCacheStats();
      await fetchStats();
      setNotice('已重置缓存统计计数！');
      setTimeout(() => setNotice(null), 3000);
    } catch (e) {
      console.error('Failed to clear cache stats', e);
    } finally {
      setClearing(false);
    }
  };

  const handleFlushCache = async () => {
    setFlushing(true);
    try {
      await api.flushCache();
      setNotice('已清空当前全部 DNS 内存缓存条目！');
      setTimeout(() => setNotice(null), 3000);
    } catch (e) {
      console.error('Failed to flush cache', e);
    } finally {
      setFlushing(false);
    }
  };

  const rows = [
    { label: '昨日', data: stats?.yesterday },
    { label: '今日', data: stats?.today },
    { label: '累计', data: stats?.total },
  ];

  return (
    <div className={`p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4 ${className}`}>
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div className="text-sm font-normal text-neutral-800 dark:text-neutral-200">
          DNS缓存状态(更新频率: 1分钟)
        </div>

        <div className="flex items-center gap-2">
          {notice && (
            <span className="text-xs text-emerald-600 dark:text-emerald-400 font-mono animate-in fade-in">
              {notice}
            </span>
          )}
          <Popconfirm
            title="确认立即清空内存中的所有 DNS 缓存条目？"
            actionText="确认清空缓存"
            onConfirm={handleFlushCache}
          >
            <button
              type="button"
              disabled={flushing}
              className="inline-flex items-center min-h-[30px] px-3.5 text-xs text-amber-700 dark:text-amber-300 bg-amber-50 hover:bg-amber-100 dark:bg-amber-950/40 dark:hover:bg-amber-900/60 border border-amber-200 dark:border-amber-800/60 rounded shadow-2xs transition-colors disabled:opacity-50"
            >
              {flushing ? '正在清空...' : '清空内存缓存'}
            </button>
          </Popconfirm>

          <Popconfirm
            title="确定清除所有 DNS 缓存累计统计计数吗？"
            actionText="确认重置计数"
            onConfirm={handleClear}
          >
            <button
              type="button"
              disabled={clearing}
              className="inline-flex items-center min-h-[30px] px-3.5 text-xs text-neutral-700 dark:text-neutral-300 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 dark:hover:border-neutral-600 rounded shadow-2xs transition-colors disabled:opacity-50"
            >
              {clearing ? '正在重置...' : '重置统计计数'}
            </button>
          </Popconfirm>
        </div>
      </div>

      <div className="overflow-x-auto rounded border border-neutral-200 dark:border-neutral-800">
        <table className="w-full text-xs text-left border-collapse">
          <thead>
            <tr className="bg-[#4a5568] dark:bg-neutral-800 text-white font-medium">
              <th className="py-2.5 px-4 w-28"></th>
              <th
                className="py-2.5 px-4 text-center font-normal cursor-help"
                title="仅统计实际进入缓存路径的查询；私有域名拒绝、规则拦截与本地重写不计入"
              >
                缓存查询次数
              </th>
              <th className="py-2.5 px-4 text-center font-normal">缓存命中次数</th>
              <th className="py-2.5 px-4 text-center font-normal">未命中次数</th>
              <th
                className="py-2.5 px-4 text-center font-normal cursor-help"
                title="缓存命中次数 ÷ 缓存查询次数"
              >
                命中比例
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-neutral-200 dark:divide-neutral-800/80 bg-white dark:bg-surface-1">
            {rows.map(({ label, data }) => (
              <tr key={label} className="hover:bg-neutral-50/70 dark:hover:bg-surface-2/60 transition-colors">
                <td className="py-3 px-4 font-normal text-neutral-700 dark:text-neutral-300 whitespace-nowrap text-center">
                  {label}
                </td>
                <td className="py-3 px-4 text-center font-mono tabular-nums text-neutral-900 dark:text-neutral-100">
                  {data ? data.requests.toLocaleString() : '0'}
                </td>
                <td className="py-3 px-4 text-center font-mono tabular-nums text-neutral-900 dark:text-neutral-100">
                  {data ? data.hits.toLocaleString() : '0'}
                </td>
                <td className="py-3 px-4 text-center font-mono tabular-nums text-neutral-900 dark:text-neutral-100">
                  {data ? data.misses.toLocaleString() : '0'}
                </td>
                <td className="py-3 px-4 text-center font-mono tabular-nums font-medium text-emerald-600 dark:text-emerald-400">
                  {data ? `${data.hit_rate.toFixed(2)}%` : '0.00%'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
};
