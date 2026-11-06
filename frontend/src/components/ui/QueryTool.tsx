import React, { useEffect, useState } from 'react';
import {
  Search,
  Radio,
  Cpu,
  AlertCircle,
  X,
} from 'lucide-react';
import { api, ToolQueryResponse } from '../../lib/api';
import { CustomSelect } from './CustomSelect';

export interface QueryToolProps {
  className?: string;
  defaultDomain?: string;
  externalQuery?: { domain: string; qtype?: string; trigger: number } | null;
}

export const QueryTool: React.FC<QueryToolProps> = ({
  className = '',
  defaultDomain = '',
  externalQuery = null,
}) => {
  const [domain, setDomain] = useState(defaultDomain);
  const [qtype, setQtype] = useState('A');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<ToolQueryResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const runQuery = async (targetDomain: string, targetQtype: string) => {
    const trimmed = targetDomain.trim();
    if (!trimmed) return;

    setLoading(true);
    setError(null);
    try {
      const data = await api.toolQuery(trimmed, targetQtype);
      setResult(data);
    } catch (err: any) {
      setError(err.message || '查询失败');
      setResult(null);
    } finally {
      setLoading(false);
    }
  };

  const handleQuery = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    await runQuery(domain, qtype);
  };

  useEffect(() => {
    if (externalQuery && externalQuery.domain) {
      const nextType = externalQuery.qtype || 'A';
      setDomain(externalQuery.domain);
      setQtype(nextType);
      runQuery(externalQuery.domain, nextType);
    }
  }, [externalQuery]);

  const qtypeOptions = [
    { value: 'A', label: 'A' },
    { value: 'AAAA', label: 'AAAA' },
    { value: 'CNAME', label: 'CNAME' },
    { value: 'MX', label: 'MX' },
    { value: 'TXT', label: 'TXT' },
  ];

  const getOutcomeBadgeClass = (outcome: string) => {
    switch (outcome) {
      case 'RESOLVED':
        return 'bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border-emerald-200 dark:border-emerald-800/50';
      case 'BLOCKED':
        return 'bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-400 border-rose-200 dark:border-rose-800/50';
      case 'CACHED':
        return 'bg-amber-50 dark:bg-amber-950/60 text-amber-700 dark:text-amber-400 border-amber-200 dark:border-amber-800/50';
      case 'REWRITTEN':
        return 'bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-400 border-indigo-200 dark:border-indigo-800/50';
      default:
        return 'bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-400 border-rose-200 dark:border-rose-800/50';
    }
  };

  const getOutcomeText = (outcome: string) => {
    switch (outcome) {
      case 'RESOLVED':
        return '正常解析';
      case 'BLOCKED':
        return '安全拦截';
      case 'CACHED':
        return '缓存命中';
      case 'REWRITTEN':
        return '本地重写';
      default:
        return '解析异常';
    }
  };

  return (
    <div className={`p-3 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-2.5 ${className}`}>
      <form onSubmit={handleQuery} className="flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
        <div className="flex items-center gap-1.5 px-2.5 py-1.5 rounded bg-cyan-50/80 dark:bg-cyan-950/40 border border-cyan-200/70 dark:border-cyan-900/50 text-cyan-700 dark:text-cyan-300 text-xs font-semibold shrink-0">
          <Cpu className="w-3.5 h-3.5 align-icon" />
          <span>DNS 实时拨测</span>
        </div>

        <div className="relative flex-1 min-w-0">
          <input
            type="text"
            value={domain}
            onChange={(e) => setDomain(e.target.value)}
            placeholder="输入待拨测域名，如 example.com、github.com、qq.com（支持点击下方日志行一键拨测）"
            className="w-full min-h-[34px] px-3 text-xs font-mono rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 dark:placeholder-neutral-500 focus:outline-none focus:border-cyan-500 transition-colors"
          />
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <div className="w-24">
            <CustomSelect
              options={qtypeOptions}
              value={qtype}
              onChange={(v) => setQtype(v)}
              className="w-full"
            />
          </div>

          <button
            type="submit"
            disabled={loading || !domain.trim()}
            className="inline-flex items-center justify-center gap-1.5 min-h-[34px] px-4 text-xs font-medium rounded-md bg-cyan-600 hover:bg-cyan-500 text-white shadow-xs transition-all active:scale-95 disabled:opacity-40 shrink-0"
          >
            <Search className={`w-3.5 h-3.5 align-icon ${loading ? 'animate-spin' : ''}`} />
            {loading ? '拨测中' : '立即拨测'}
          </button>

          {(result || error) && (
            <button
              type="button"
              onClick={() => {
                setResult(null);
                setError(null);
              }}
              className="inline-flex items-center justify-center min-h-[34px] px-2 rounded-md text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200 bg-neutral-100 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-700"
              title="收起拨测结果"
            >
              <X className="w-3.5 h-3.5 align-icon" />
            </button>
          )}
        </div>
      </form>

      {error && (
        <div className="px-3 py-2 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-600 dark:text-rose-400 flex items-center gap-2">
          <AlertCircle className="w-3.5 h-3.5 shrink-0 align-icon" />
          <span>{error}</span>
        </div>
      )}

      {result && (
        <div className="px-3 py-2 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex flex-wrap items-center justify-between gap-2 text-xs animate-in fade-in-50 duration-150">
          <div className="flex flex-wrap items-center gap-2">
            <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded border font-medium ${getOutcomeBadgeClass(result.outcome)}`}>
              <Radio className="w-3 h-3 align-icon" />
              {getOutcomeText(result.outcome)}
            </span>
            <span className="font-mono font-semibold text-neutral-800 dark:text-neutral-200">
              {result.domain} ({result.qtype})
            </span>
            <span className="px-1.5 py-0.5 rounded bg-neutral-200/70 dark:bg-surface-3 font-mono text-[11px] text-neutral-600 dark:text-neutral-300">
              {result.rcode}
            </span>
            <span className="font-mono text-[11px] text-cyan-600 dark:text-cyan-400 font-semibold tabular-nums">
              {result.duration_ms.toFixed(1)} ms
            </span>
            <span className="text-[11px] font-mono text-neutral-500 dark:text-neutral-400">
              [{result.summary}]
            </span>
          </div>

          <div className="flex flex-wrap items-center gap-1.5">
            {result.answers.length > 0 ? (
              result.answers.map((ans, idx) => (
                <span
                  key={idx}
                  className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-700 font-mono text-[11px]"
                >
                  <span className="text-neutral-400">{ans.record_type}</span>
                  <span className="font-semibold text-neutral-900 dark:text-neutral-100">{ans.data}</span>
                  <span className="text-neutral-400">({ans.ttl}s)</span>
                </span>
              ))
            ) : (
              <span className="font-mono text-[11px] text-neutral-400">无记录返回 (NODATA)</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
};
