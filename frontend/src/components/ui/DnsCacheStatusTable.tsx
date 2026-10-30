import React, { useEffect, useState } from 'react';
import { api, CacheStatsSnapshot } from '../../lib/api';
import { Popconfirm } from './Popconfirm';

export interface DnsCacheStatusTableProps {
  className?: string;
}

export const DnsCacheStatusTable: React.FC<DnsCacheStatusTableProps> = ({ className = '' }) => {
  const [stats, setStats] = useState<CacheStatsSnapshot | null>(null);
  const [clearing, setClearing] = useState(false);

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
    } catch (e) {
      console.error('Failed to clear cache stats', e);
    } finally {
      setClearing(false);
    }
  };

  const rows = [
    { label: '昨日', data: stats?.yesterday },
    { label: '今日', data: stats?.today },
    { label: '累计', data: stats?.total },
  ];

  return (
    <div className={`p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4 ${className}`}>
      <div className="flex flex-col gap-3">
        <div className="text-sm font-normal text-neutral-800 dark:text-neutral-200">
          DNS缓存状态(更新频率: 1分钟)
        </div>

        <div>
          <Popconfirm
            title="确定清除所有 DNS 缓存累计统计数据吗？"
            actionText="确认清除"
            onConfirm={handleClear}
          >
            <button
              type="button"
              disabled={clearing}
              className="inline-flex items-center min-h-[30px] px-3.5 text-xs text-neutral-700 dark:text-neutral-300 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 dark:hover:border-neutral-600 rounded shadow-2xs transition-colors disabled:opacity-50"
            >
              {clearing ? '正在清除...' : '清除所有数据'}
            </button>
          </Popconfirm>
        </div>
      </div>

      <div className="overflow-x-auto rounded border border-neutral-200 dark:border-neutral-800">
        <table className="w-full text-xs text-left border-collapse">
          <thead>
            <tr className="bg-[#4a5568] dark:bg-neutral-800 text-white font-medium">
              <th className="py-2.5 px-4 w-28"></th>
              <th className="py-2.5 px-4 text-center font-normal">请求DNS次数</th>
              <th className="py-2.5 px-4 text-center font-normal">缓存命中次数</th>
              <th className="py-2.5 px-4 text-center font-normal">未命中次数</th>
              <th className="py-2.5 px-4 text-center font-normal">命中比例</th>
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
