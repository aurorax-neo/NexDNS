import React, { useState } from 'react';
import {
  Search,
  Radio,
  Cpu,
  ArrowRight,
  AlertCircle,
  CheckCircle2,
  ShieldAlert,
  Database,
  Layers,
  ExternalLink,
} from 'lucide-react';
import { api, ToolQueryResponse } from '../../lib/api';
import { CustomSelect } from './CustomSelect';

export interface QueryToolProps {
  className?: string;
  defaultDomain?: string;
}

export const QueryTool: React.FC<QueryToolProps> = ({
  className = '',
  defaultDomain = '',
}) => {
  const [domain, setDomain] = useState(defaultDomain);
  const [qtype, setQtype] = useState('A');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<ToolQueryResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleQuery = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const trimmed = domain.trim();
    if (!trimmed) return;

    setLoading(true);
    setError(null);
    try {
      const data = await api.toolQuery(trimmed, qtype);
      setResult(data);
    } catch (err: any) {
      setError(err.message || '查询失败');
      setResult(null);
    } finally {
      setLoading(false);
    }
  };

  const qtypeOptions = [
    { value: 'A', label: 'A' },
    { value: 'AAAA', label: 'AAAA' },
    { value: 'CNAME', label: 'CNAME' },
    { value: 'MX', label: 'MX' },
    { value: 'TXT', label: 'TXT' },
  ];

  const getOutcomeColor = (outcome: string) => {
    switch (outcome) {
      case 'RESOLVED':
        return 'text-emerald-500 dark:text-emerald-400';
      case 'BLOCKED':
        return 'text-rose-500 dark:text-rose-400';
      case 'CACHED':
        return 'text-amber-500 dark:text-amber-400';
      case 'REWRITTEN':
        return 'text-cyan-500 dark:text-cyan-400';
      default:
        return 'text-rose-600 dark:text-rose-500';
    }
  };

  const getOutcomeText = (outcome: string) => {
    switch (outcome) {
      case 'RESOLVED':
        return '正常解析 (RESOLVED)';
      case 'BLOCKED':
        return '安全拦截 (BLOCKED)';
      case 'CACHED':
        return '缓存命中 (CACHED)';
      case 'REWRITTEN':
        return '本地重写 (REWRITTEN)';
      default:
        return '解析异常 (ERROR)';
    }
  };

  return (
    <div className={`p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4 ${className}`}>
      {/* 标题栏 */}
      <div>
        <div className="flex items-center gap-2">
          <Search className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
          <h3 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">
            DNS 查询诊断工具
          </h3>
        </div>
        <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">
          测试指定域名的全流程真实解析、规则拦截与分流链路
        </p>
      </div>

      {/* 输入与操作栏 */}
      <form onSubmit={handleQuery} className="flex flex-col sm:flex-row items-stretch sm:items-center gap-2.5">
        <div className="relative flex-1 min-w-0">
          <input
            type="text"
            value={domain}
            onChange={(e) => setDomain(e.target.value)}
            placeholder="输入待测试的域名，如 example.com、github.com 或 qq.com"
            className="w-full min-h-[40px] px-3.5 text-xs font-mono rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 dark:placeholder-neutral-500 focus:outline-none focus:border-cyan-500 dark:focus:border-cyan-400 transition-colors shadow-2xs"
          />
        </div>

        <div className="shrink-0 w-24 sm:w-28 min-w-0">
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
          className="inline-flex items-center justify-center gap-2 min-h-[40px] px-5 text-xs font-medium rounded-md bg-neutral-900 hover:bg-neutral-800 dark:bg-surface-3 dark:hover:bg-neutral-700 text-white border border-neutral-800 dark:border-neutral-600 shadow-xs transition-all active:scale-95 disabled:opacity-40 shrink-0"
        >
          <Search className={`w-3.5 h-3.5 align-icon ${loading ? 'animate-spin' : ''}`} />
          查询
        </button>
      </form>

      {/* 错误提示 */}
      {error && (
        <div className="p-3 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-600 dark:text-rose-400 flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0 align-icon" />
          <span>{error}</span>
        </div>
      )}

      {/* 诊断结果卡片 */}
      {result && (
        <div className="rounded-lg bg-neutral-50 dark:bg-[#121211] border border-neutral-200 dark:border-neutral-800/80 overflow-hidden shadow-xs animate-in fade-in-50 duration-200">
          {/* 顶栏微标 */}
          <div className="px-4 py-2.5 bg-neutral-100/80 dark:bg-surface-2 border-b border-neutral-200 dark:border-neutral-800/80 flex items-center justify-between text-xs">
            <div className="flex items-center gap-2 font-mono text-neutral-700 dark:text-neutral-300">
              <Cpu className="w-3.5 h-3.5 text-cyan-500 align-icon" />
              <span className="font-semibold">NexDNS 原生单内核引擎</span>
            </div>
            <div className="font-mono text-[11px] text-neutral-500 dark:text-neutral-400 tabular-nums">
              耗时: <span className="font-semibold text-cyan-600 dark:text-cyan-400">{result.duration_ms.toFixed(1)} ms</span>
            </div>
          </div>

          {/* 双栏诊断内容区 */}
          <div className="grid grid-cols-1 md:grid-cols-12 divide-y md:divide-y-0 md:divide-x divide-neutral-200 dark:divide-neutral-800/80">
            {/* 左侧: 解析判定结果 */}
            <div className="p-5 md:col-span-5 space-y-3 flex flex-col justify-between">
              <div>
                <div className="flex items-center gap-1.5 text-[11px] font-semibold tracking-wider text-neutral-400 dark:text-neutral-500 uppercase">
                  <Radio className="w-3 h-3 text-cyan-500 align-icon" />
                  <span>解析判定结果</span>
                </div>

                <div className={`mt-2 text-2xl font-extrabold tracking-tight font-mono ${getOutcomeColor(result.outcome)}`}>
                  {getOutcomeText(result.outcome)}
                </div>

                <div className="mt-1 font-mono text-xs text-neutral-600 dark:text-neutral-300">
                  <span className="font-semibold">{result.rcode.toUpperCase()}</span>
                  <span className="mx-1.5 text-neutral-400">/</span>
                  <span>共 {result.answers_count} 条应答记录</span>
                </div>
              </div>

              <div className="pt-2 border-t border-neutral-200/60 dark:border-neutral-800/60 text-[11px] font-mono text-neutral-500 dark:text-neutral-400 break-all">
                决策链路: {result.summary}
              </div>
            </div>

            {/* 右侧: DNS 应答记录列表 */}
            <div className="p-5 md:col-span-7 space-y-3">
              <div className="text-[11px] font-semibold tracking-wider text-neutral-400 dark:text-neutral-500 uppercase">
                DNS 应答解析记录 (Answers)
              </div>

              <div className="space-y-2">
                {result.answers.length > 0 ? (
                  result.answers.map((ans, idx) => (
                    <div
                      key={idx}
                      className="flex items-center justify-between gap-3 p-2 rounded bg-white dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800/70 font-mono text-xs"
                    >
                      <div className="flex items-center gap-3 truncate min-w-0">
                        <span className="text-[11px] font-semibold text-neutral-400 dark:text-neutral-500 tabular-nums shrink-0">
                          {String(idx + 1).padStart(2, '0')}
                        </span>
                        <span className="font-bold text-neutral-900 dark:text-neutral-100 truncate">
                          {ans.data}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 shrink-0 text-[11px] text-neutral-400">
                        <span className="px-1.5 py-0.5 rounded bg-neutral-100 dark:bg-surface-3 text-neutral-600 dark:text-neutral-400">
                          {ans.record_type}
                        </span>
                        <span className="tabular-nums">TTL {ans.ttl}s</span>
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="py-6 text-center text-xs text-neutral-400 font-mono">
                    无记录返回 (NODATA)
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
