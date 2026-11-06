import React, { useEffect, useState, useRef } from 'react';
import {
  Search,
  Play,
  RefreshCw,
  Download,
  Trash2,
  CheckCircle2,
  ShieldAlert,
  Database,
  GitFork,
  Radio,
  Shield,
  Check,
  ChevronLeft,
  ChevronRight,
  HardDrive,
  X,
  Cpu,
} from 'lucide-react';
import { api, DiskStatus, QueryLogItem } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';
import { AlertDialog } from '../components/ui/AlertDialog';
import { CustomSelect } from '../components/ui/CustomSelect';
import { Snackbar } from '../components/ui/Snackbar';
import { formatShortDateTime, formatDateTime } from '../lib/time';
import { QueryTool } from '../components/ui/QueryTool';

export interface QueryLogPageProps {
  onNavigate?: (tab: string) => void;
}

export const QueryLogPage: React.FC<QueryLogPageProps> = ({ onNavigate }) => {
  const [logs, setLogs] = useState<QueryLogItem[]>([]);
  const [diskStatus, setDiskStatus] = useState<DiskStatus | null>(null);
  const [loggingEnabled, setLoggingEnabled] = useState<boolean>(true);
  const [loadingSettings, setLoadingSettings] = useState<boolean>(true);
  const [isLive, setIsLive] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [search, setSearch] = useState('');
  const [clientFilter, setClientFilter] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [selectedLog, setSelectedLog] = useState<QueryLogItem | null>(null);
  const [externalQuery, setExternalQuery] = useState<{ domain: string; qtype?: string; trigger: number } | null>(null);
  const [isClearAlertOpen, setIsClearAlertOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);
  const [total, setTotal] = useState(0);
  const [actionSnackbar, setActionSnackbar] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const abortControllerRef = useRef<AbortController | null>(null);
  const bufferRef = useRef<QueryLogItem[]>([]);
  const batchTimeoutRef = useRef<number | null>(null);
  // SSE 连接存活期间读取最新筛选条件，但筛选变化不重建连接
  const liveFiltersRef = useRef({ page, pageSize, search, statusFilter, clientFilter });
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    liveFiltersRef.current = { page, pageSize, search, statusFilter, clientFilter };
  }, [page, pageSize, search, statusFilter, clientFilter]);

  useEffect(() => {
    api.getSettings().then((s) => {
      setLoggingEnabled(s.logging.enabled);
      setLoadingSettings(false);
    }).catch(() => {
      setLoadingSettings(false);
    });
  }, []);

  // Load initial logs
  const fetchLogs = async () => {
    if (!loggingEnabled) return;
    try {
      const res = await api.getQueryLogs({
        page,
        page_size: pageSize,
        search: search || undefined,
        status: statusFilter,
        client: clientFilter || undefined,
      });
      setLogs(res.items);
      setTotal(res.total);
      if (res.disk) {
        setDiskStatus(res.disk);
      }
    } catch (e) {
      console.error('Failed to load logs', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!loggingEnabled) return;
    fetchLogs();
  }, [search, statusFilter, clientFilter, page, pageSize, loggingEnabled]);

  const handleFilterChange = (val: string) => {
    setStatusFilter(val);
    setPage(1);
  };

  // HTTPS SSE (Server-Sent Events) live streaming (默认关闭，用户手动开启后连接)
  // 连接生命周期只受开关与日志开关控制，筛选条件通过 ref 读取，避免重连竞态导致无法关闭
  useEffect(() => {
    if (!isLive || !loggingEnabled) {
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
        abortControllerRef.current = null;
      }
      if (batchTimeoutRef.current) {
        window.clearTimeout(batchTimeoutRef.current);
        batchTimeoutRef.current = null;
      }
      bufferRef.current = [];
      return;
    }

    const abortCtrl = new AbortController();
    abortControllerRef.current = abortCtrl;
    const token = localStorage.getItem('nexdns_token');
    const headers: Record<string, string> = {
      'Accept': 'text/event-stream',
    };
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

    const flushBuffer = () => {
      const buffered = bufferRef.current;
      if (buffered.length === 0) return;
      bufferRef.current = [];

      const { page: curPage, pageSize: curPageSize } = liveFiltersRef.current;
      const count = buffered.length;
      setTotal((prev) => prev + count);
      if (curPage === 1) {
        setLogs((prev) => {
          // 保持时间倒序: 最新到来的日志置于顶部
          const next = [...buffered.slice().reverse(), ...prev];
          return next.slice(0, curPageSize);
        });
      }
    };

    (async () => {
      let reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
      try {
        const response = await fetch('/api/v1/sse/querylog', {
          headers,
          signal: abortCtrl.signal,
        });
        if (!response.ok || !response.body) {
          if (!abortCtrl.signal.aborted && mountedRef.current) {
            setIsLive(false);
          }
          return;
        }
        reader = response.body.getReader();
        const decoder = new TextDecoder();
        let streamBuffer = '';

        while (!abortCtrl.signal.aborted) {
          const { done, value } = await reader.read();
          if (done) break;
          streamBuffer += decoder.decode(value, { stream: true });
          const blocks = streamBuffer.split('\n\n');
          streamBuffer = blocks.pop() || '';

          for (const block of blocks) {
            for (const line of block.split('\n')) {
              if (line.startsWith('data:')) {
                const dataStr = line.slice(5).trim();
                if (!dataStr) continue;
                try {
                  const item: QueryLogItem = JSON.parse(dataStr);
                  const { search: curSearch, statusFilter: curStatus, clientFilter: curClient } = liveFiltersRef.current;
                  const matchesStatus = curStatus === 'all' || item.status === curStatus;
                  const matchesClient = !curClient || item.client_ip === curClient;
                  const searchTrimmed = curSearch.toLowerCase().trim();
                  const matchesSearch =
                    !searchTrimmed ||
                    item.qname.toLowerCase().includes(searchTrimmed) ||
                    item.client_ip.includes(searchTrimmed);
                  if (matchesStatus && matchesClient && matchesSearch) {
                    bufferRef.current.push(item);
                    if (!batchTimeoutRef.current) {
                      batchTimeoutRef.current = window.setTimeout(() => {
                        batchTimeoutRef.current = null;
                        flushBuffer();
                      }, 80); // 80ms 缓冲防闪烁
                    }
                  }
                } catch (e) {
                  console.error('Failed to parse SSE log', e);
                }
              }
            }
          }
        }
      } catch (err: any) {
        if (err?.name !== 'AbortError') {
          console.error('SSE live stream error', err);
        }
      } finally {
        // 非用户主动关闭（服务端结束/网络断开/鉴权失效）时同步 UI，避免显示"实时流"但实际已断开
        if (!abortCtrl.signal.aborted && mountedRef.current) {
          setIsLive(false);
        }
        if (reader) {
          try {
            await reader.cancel();
          } catch {
            // 流可能已关闭，忽略
          }
        }
      }
    })();

    return () => {
      abortCtrl.abort();
      if (abortControllerRef.current === abortCtrl) {
        abortControllerRef.current = null;
      }
      if (batchTimeoutRef.current) {
        window.clearTimeout(batchTimeoutRef.current);
        batchTimeoutRef.current = null;
      }
      bufferRef.current = [];
    };
  }, [isLive, loggingEnabled]);

  const handleClearLogs = async () => {
    try {
      await api.clearQueryLogs();
      setLogs([]);
      setTotal(0);
      setSelectedLog(null);
      await fetchLogs();
    } catch (e) {
      console.error('Failed to clear logs', e);
    }
  };

  const handleExportCsv = async () => {
    setIsExporting(true);
    try {
      const token = localStorage.getItem('nexdns_token');
      const headers: Record<string, string> = {};
      if (token) headers['Authorization'] = `Bearer ${token}`;
      const res = await fetch('/api/v1/querylog/export', { headers });
      if (!res.ok) throw new Error('导出失败');
      const blob = await res.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `nexdns-querylog-${new Date().toISOString().slice(0, 10)}.csv`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      window.URL.revokeObjectURL(url);
      setActionSnackbar('已成功导出查询日志 CSV');
    } catch (e) {
      console.error('Export CSV error', e);
      setActionSnackbar('导出 CSV 失败，请检查网络或登录状态');
    } finally {
      setIsExporting(false);
    }
  };

  const handleQuickRule = async (domain: string, action: 'block' | 'whitelist') => {
    try {
      await api.quickAddRule(domain, action);
      setActionSnackbar(action === 'block' ? `已将 ${domain} 加入拦截黑名单` : `已将 ${domain} 加入放行白名单`);
      await fetchLogs();
    } catch (e) {
      console.error('Failed to add quick rule', e);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'allowed':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800/40">
            <CheckCircle2 className="w-3 h-3 align-icon" />
            放行
          </span>
        );
      case 'blocked':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-400 border border-rose-200 dark:border-rose-800/40">
            <ShieldAlert className="w-3 h-3 align-icon" />
            已拦截
          </span>
        );
      case 'cached':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-amber-50 dark:bg-amber-950/60 text-amber-700 dark:text-amber-400 border border-amber-200 dark:border-amber-800/40">
            <Database className="w-3 h-3 align-icon" />
            缓存
          </span>
        );
      case 'rewritten':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-400 border border-indigo-200 dark:border-indigo-800/40">
            <GitFork className="w-3 h-3 align-icon" />
            重写
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-neutral-100 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400">
            {status}
          </span>
        );
    }
  };

  const statusOptions = [
    { value: 'all', label: '全部状态' },
    { value: 'allowed', label: '正常放行' },
    { value: 'blocked', label: '已拦截' },
    { value: 'cached', label: '命中缓存' },
    { value: 'rewritten', label: '本地重写' },
  ];

  const renderInspectorContent = (log: QueryLogItem) => (
    <div className="space-y-3 text-xs">
      {/* 快捷操作按钮组 */}
      <div className="grid grid-cols-2 gap-1.5">
        <button
          type="button"
          onClick={() => handleQuickRule(log.qname, 'block')}
          className="inline-flex items-center justify-center gap-1.5 min-h-[32px] px-2.5 text-xs font-medium bg-rose-600 hover:bg-rose-500 text-white rounded-md transition-colors"
        >
          <Shield className="w-3.5 h-3.5 align-icon" />
          拦截此域名
        </button>
        <button
          type="button"
          onClick={() => handleQuickRule(log.qname, 'whitelist')}
          className="inline-flex items-center justify-center gap-1.5 min-h-[32px] px-2.5 text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-white rounded-md transition-colors"
        >
          <Check className="w-3.5 h-3.5 align-icon" />
          放行白名单
        </button>
        <button
          type="button"
          onClick={() => setExternalQuery({ domain: log.qname, qtype: log.qtype || 'A', trigger: Date.now() })}
          className="inline-flex items-center justify-center gap-1.5 min-h-[32px] px-2.5 text-xs font-medium text-cyan-700 dark:text-cyan-300 bg-cyan-50 dark:bg-cyan-950/50 hover:bg-cyan-100 dark:hover:bg-cyan-900/60 border border-cyan-200 dark:border-cyan-800/60 rounded-md transition-colors"
        >
          <Cpu className="w-3.5 h-3.5 align-icon" />
          立即拨测
        </button>
        <button
          type="button"
          onClick={() => {
            setSearch(log.qname);
            setPage(1);
            setSelectedLog(null);
          }}
          className="inline-flex items-center justify-center gap-1.5 min-h-[32px] px-2.5 text-xs text-neutral-700 dark:text-neutral-300 hover:bg-neutral-100 dark:hover:bg-surface-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-colors"
        >
          <Search className="w-3.5 h-3.5 align-icon" />
          筛选此域名
        </button>
      </div>

      {/* 核心报文详情 */}
      <div className="p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-2">
        <div className="flex justify-between items-center gap-2">
          <span className="text-neutral-500 dark:text-neutral-400 shrink-0">请求时间</span>
          <span className="font-mono text-neutral-900 dark:text-neutral-100 tabular-nums">
            {formatDateTime(log.timestamp)}
          </span>
        </div>
        <div className="flex justify-between items-start gap-2">
          <span className="text-neutral-500 dark:text-neutral-400 shrink-0">查询域名</span>
          <span className="font-mono text-neutral-900 dark:text-neutral-100 font-semibold break-all text-right">
            {log.qname}
          </span>
        </div>
        <div className="flex justify-between items-center gap-2">
          <span className="text-neutral-500 dark:text-neutral-400">记录类型 / 响应码</span>
          <span className="font-mono text-neutral-800 dark:text-neutral-200">
            {log.qtype} · {log.rcode}
          </span>
        </div>
        <div className="flex justify-between items-center gap-2">
          <span className="text-neutral-500 dark:text-neutral-400">处理状态</span>
          <div>{getStatusBadge(log.status)}</div>
        </div>
        <div className="flex justify-between items-center gap-2">
          <span className="text-neutral-500 dark:text-neutral-400">客户端 IP</span>
          <button
            type="button"
            onClick={() => {
              setClientFilter(log.client_ip === clientFilter ? '' : log.client_ip);
              setPage(1);
            }}
            className="font-mono text-cyan-600 dark:text-cyan-400 hover:underline"
            title="点击按此客户端 IP 过滤"
          >
            {log.client_ip}
          </button>
        </div>
        <div className="flex justify-between items-center gap-2">
          <span className="text-neutral-500 dark:text-neutral-400">全流程耗时</span>
          <span className="font-mono text-cyan-600 dark:text-cyan-400 font-semibold tabular-nums">
            {log.duration_ms.toFixed(2)} ms
          </span>
        </div>
        <div className="flex justify-between items-center gap-2">
          <span className="text-neutral-500 dark:text-neutral-400">调度上游组</span>
          <span className="font-mono text-neutral-800 dark:text-neutral-200">
            {log.routing_group || '本地直接响应'}
          </span>
        </div>
        <div className="flex justify-between items-start gap-2">
          <span className="text-neutral-500 dark:text-neutral-400 shrink-0">命中规则</span>
          <span className="font-mono text-rose-600 dark:text-rose-400 break-all text-right">
            {log.matched_rule || '-'}
          </span>
        </div>
        <div className="pt-2 border-t border-neutral-200 dark:border-neutral-800 space-y-1">
          <span className="text-neutral-500 dark:text-neutral-400 block">返回解析结果 (Response)</span>
          <div className="font-mono text-xs text-emerald-600 dark:text-emerald-400 font-semibold bg-white dark:bg-surface-1 p-2 rounded border border-neutral-200 dark:border-neutral-800 break-all">
            {log.response_ip || (log.rcode === 'No Error' ? '无记录 (NODATA)' : log.rcode)}
          </div>
        </div>
      </div>
    </div>
  );

  if (!loggingEnabled && !loadingSettings) {
    return (
      <div className="p-12 text-center rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-5 max-w-lg mx-auto my-16 animate-in fade-in-50">
        <div className="w-14 h-14 mx-auto rounded-full bg-amber-50 dark:bg-amber-950/40 text-amber-600 dark:text-amber-400 flex items-center justify-center">
          <ShieldAlert className="w-7 h-7 align-icon" />
        </div>
        <div className="space-y-2">
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100">
            DNS 查询日志记录已关闭
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 leading-relaxed">
            系统已在配置中停用日志留存功能，当前不会留存或展示任何 DNS 请求报文。如需监控实时流量与解析诊断，请前往系统设置开启【记录 DNS 查询日志】。
          </p>
        </div>
        {onNavigate && (
          <button
            type="button"
            onClick={() => onNavigate('settings')}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-5 text-xs font-medium rounded-md bg-cyan-600 hover:bg-cyan-500 text-white shadow-xs transition-colors"
          >
            前往系统设置开启
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-2.5 animate-in fade-in-50 duration-200">
      {/* 紧凑单行 DNS 实时拨测工具条 */}
      <QueryTool externalQuery={externalQuery} />

      {/* 紧凑一体化筛选、状态快速切换与磁盘水位工具栏 */}
      <div className="flex flex-wrap items-center justify-between gap-2.5 p-2.5 px-3 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex flex-wrap items-center gap-2 flex-1 min-w-[260px]">
          {/* 搜索框 */}
          <div className="relative w-full sm:w-64">
            <Search className="w-3.5 h-3.5 text-neutral-400 absolute left-2.5 top-1/2 -translate-y-1/2 align-icon" />
            <input
              type="text"
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setPage(1);
              }}
              placeholder="搜索域名 / 客户端 IP / 规则..."
              className="w-full min-h-[34px] pl-8 pr-7 text-xs bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 rounded-md text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500 transition-colors"
            />
            {search && (
              <button
                type="button"
                onClick={() => {
                  setSearch('');
                  setPage(1);
                }}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-neutral-400 hover:text-neutral-600"
              >
                <X className="w-3.5 h-3.5 align-icon" />
              </button>
            )}
          </div>

          {/* 状态分段快捷过滤按钮组 */}
          <div className="inline-flex items-center p-0.5 rounded-md bg-neutral-100 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 text-xs">
            {statusOptions.map((opt) => {
              const active = statusFilter === opt.value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => handleFilterChange(opt.value)}
                  className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
                    active
                      ? 'bg-white dark:bg-surface-1 text-cyan-600 dark:text-cyan-400 shadow-2xs'
                      : 'text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-200'
                  }`}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>

          {clientFilter && (
            <span className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-cyan-50 dark:bg-cyan-950/60 border border-cyan-200 dark:border-cyan-800/50 text-xs font-mono text-cyan-700 dark:text-cyan-300">
              客户端: {clientFilter}
              <button
                type="button"
                onClick={() => {
                  setClientFilter('');
                  setPage(1);
                }}
                className="hover:text-cyan-900 dark:hover:text-white"
              >
                <X className="w-3 h-3 align-icon" />
              </button>
            </span>
          )}
        </div>

        {/* 右侧磁盘水位指标与动作按钮集合 */}
        <div className="flex flex-wrap items-center gap-1.5">
          {diskStatus && (
            <div
              className="hidden md:inline-flex items-center gap-1.5 min-h-[34px] px-2.5 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 text-[11px] font-mono text-neutral-600 dark:text-neutral-300 tabular-nums"
              title="后台每 15 秒自动巡检磁盘水位并在超限时淘汰旧日志释放空间"
            >
              <HardDrive className={`w-3.5 h-3.5 align-icon ${diskStatus.disk_used_percent >= 85 ? 'text-rose-500' : 'text-cyan-500'}`} />
              <span>日志库 {(diskStatus.db_size_bytes / (1024 * 1024)).toFixed(1)}MB</span>
              <span className="text-neutral-300 dark:text-neutral-700">|</span>
              <span>磁盘 {diskStatus.disk_used_percent.toFixed(0)}% (余 {(diskStatus.disk_avail_bytes / (1024 * 1024 * 1024)).toFixed(1)}GB)</span>
            </div>
          )}

          <button
            type="button"
            onClick={async () => {
              setIsRefreshing(true);
              await fetchLogs();
              setTimeout(() => setIsRefreshing(false), 350);
            }}
            disabled={isRefreshing}
            className="inline-flex items-center justify-center gap-1 min-h-[34px] px-2.5 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-colors active:scale-95 disabled:opacity-60"
            title="手动刷新日志"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${isRefreshing ? 'animate-spin text-cyan-600 dark:text-cyan-400' : ''}`} />
            刷新
          </button>

          <button
            type="button"
            onClick={() => setIsLive((prev) => !prev)}
            className={`inline-flex items-center justify-center gap-1 min-h-[34px] px-2.5 text-xs rounded-md border transition-colors ${
              isLive
                ? 'bg-emerald-50 dark:bg-emerald-950/50 border-emerald-200 dark:border-emerald-800/60 text-emerald-700 dark:text-emerald-400'
                : 'bg-neutral-100 dark:bg-surface-2 border-neutral-300 dark:border-neutral-700 text-neutral-600 dark:text-neutral-400'
            }`}
          >
            {isLive ? <Radio className="w-3.5 h-3.5 animate-pulse align-icon" /> : <Play className="w-3.5 h-3.5 align-icon" />}
            {isLive ? '实时流' : '实时跟踪'}
          </button>

          <button
            type="button"
            onClick={handleExportCsv}
            disabled={isExporting}
            className="inline-flex items-center justify-center gap-1 min-h-[34px] px-2.5 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-colors disabled:opacity-50"
          >
            <Download className="w-3.5 h-3.5 align-icon" />
            {isExporting ? '导出中...' : '导出 CSV'}
          </button>

          <button
            type="button"
            onClick={() => setIsClearAlertOpen(true)}
            className="inline-flex items-center justify-center gap-1 min-h-[34px] px-2.5 text-xs text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 bg-neutral-100 dark:bg-surface-2 border border-red-200 dark:border-red-900/40 rounded-md transition-colors"
          >
            <Trash2 className="w-3.5 h-3.5 align-icon" />
            清空
          </button>
        </div>
      </div>

      {/* 全宽查询日志主表格 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="overflow-x-auto no-scrollbar">
          <table className="w-full min-w-[860px] lg:min-w-0 text-left text-xs table-fixed">
            <thead className="bg-neutral-50 dark:bg-surface-2/60 border-b border-neutral-200 dark:border-neutral-800 text-neutral-500 dark:text-neutral-400 uppercase font-medium">
              <tr>
                <th className="py-2.5 px-3.5 w-[110px]">请求时间</th>
                <th className="py-2.5 px-2.5 w-[80px]">状态</th>
                <th className="py-2.5 px-3.5">查询域名与类型</th>
                <th className="py-2.5 px-2.5 w-[18%]">客户端 IP</th>
                <th className="py-2.5 px-2.5 w-[14%]">分流 / 匹配规则</th>
                <th className="py-2.5 px-2.5 w-[20%]">响应结果</th>
                <th className="py-2.5 px-2.5 w-[80px] text-right">时延</th>
                <th className="py-2.5 px-3.5 w-[124px] text-right">就地操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200 dark:divide-neutral-800/60 font-normal bg-white dark:bg-surface-1">
              {logs.length > 0 ? (
                logs.map((log) => {
                  const timeDisplay = formatShortDateTime(log.timestamp);
                  const fullTimeDisplay = formatDateTime(log.timestamp);
                  const cleanQname = (log.qname || '').replace(/[\r\n\t]+/g, ' ');
                  const isSelected = selectedLog?.id === log.id;
                  return (
                    <tr
                      key={log.id}
                      onClick={() => setSelectedLog(log)}
                      className={`cursor-pointer select-none ${
                        isSelected
                          ? 'bg-cyan-50/60 dark:bg-cyan-950/25'
                          : 'hover:bg-neutral-100/60 dark:hover:bg-white/[0.04]'
                      }`}
                    >
                      <td
                        className="py-2 px-3 font-mono text-[11px] text-neutral-500 dark:text-neutral-400 tabular-nums whitespace-nowrap"
                        title={fullTimeDisplay}
                      >
                        {timeDisplay}
                      </td>
                      <td className="py-2 px-2.5 whitespace-nowrap">
                        {getStatusBadge(log.status)}
                      </td>
                      <td className="py-2 px-3 overflow-hidden">
                        <div className="flex items-center gap-1.5 min-w-0">
                          <span
                            className="font-mono text-neutral-800 dark:text-neutral-200 font-medium truncate block max-w-full"
                            title={cleanQname}
                          >
                            {cleanQname}
                          </span>
                          <span className="px-1 py-0.5 text-[10px] font-mono rounded bg-neutral-100 dark:bg-surface-3 text-neutral-500 dark:text-neutral-400 shrink-0">
                            {log.qtype}
                          </span>
                        </div>
                      </td>
                      <td className="py-2 px-2.5 font-mono text-[11px] text-neutral-600 dark:text-neutral-300 tabular-nums overflow-hidden">
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            setClientFilter(clientFilter === log.client_ip ? '' : log.client_ip);
                            setPage(1);
                          }}
                          className="truncate block max-w-full hover:text-cyan-600 dark:hover:text-cyan-400 hover:underline text-left"
                          title={`点击按客户端 ${log.client_ip} 筛选`}
                        >
                          {log.client_ip}
                        </button>
                      </td>
                      <td className="py-2 px-2.5 text-neutral-500 dark:text-neutral-400 overflow-hidden">
                        {log.matched_rule ? (
                          <span className="text-rose-600 dark:text-rose-400 font-mono text-[11px] truncate block max-w-full" title={log.matched_rule}>
                            {log.matched_rule}
                          </span>
                        ) : log.routing_group ? (
                          <span className="text-cyan-600 dark:text-cyan-400 font-mono text-[11px] truncate block max-w-full" title={`组: ${log.routing_group}`}>
                            组: {log.routing_group}
                          </span>
                        ) : (
                          <span className="text-neutral-400 text-[11px]">本地直出</span>
                        )}
                      </td>
                      <td className="py-2 px-2.5 font-mono text-[11px] text-neutral-700 dark:text-neutral-300 tabular-nums overflow-hidden">
                        {log.response_ip ? (
                          <span className="truncate block max-w-full" title={log.response_ip}>
                            {log.response_ip}
                          </span>
                        ) : log.rcode === 'No Error' ? (
                          <span className="text-neutral-400 truncate block max-w-full">无记录 (NODATA)</span>
                        ) : (
                          <span className="text-rose-500 font-medium truncate block max-w-full" title={log.rcode}>
                            {log.rcode}
                          </span>
                        )}
                      </td>
                      <td className="py-2 px-2.5 text-right font-mono text-[11px] tabular-nums text-neutral-700 dark:text-neutral-300 whitespace-nowrap">
                        {log.duration_ms < 1 ? '< 1 ms' : `${log.duration_ms.toFixed(1)} ms`}
                      </td>
                      <td
                        className="py-2 px-3 text-right whitespace-nowrap"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <div className="inline-flex items-center justify-end gap-1">
                          <button
                            type="button"
                            onClick={() => handleQuickRule(log.qname, 'block')}
                            className="px-1.5 py-0.5 text-[11px] rounded border border-rose-200 dark:border-rose-900/50 bg-rose-50/70 dark:bg-rose-950/40 text-rose-600 dark:text-rose-400 hover:bg-rose-100 dark:hover:bg-rose-900/60 transition-colors"
                            title="加入拦截黑名单"
                          >
                            拦截
                          </button>
                          <button
                            type="button"
                            onClick={() => handleQuickRule(log.qname, 'whitelist')}
                            className="px-1.5 py-0.5 text-[11px] rounded border border-emerald-200 dark:border-emerald-900/50 bg-emerald-50/70 dark:bg-emerald-950/40 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-100 dark:hover:bg-emerald-900/60 transition-colors"
                            title="加入放行白名单"
                          >
                            放行
                          </button>
                          <button
                            type="button"
                            onClick={() => setExternalQuery({ domain: log.qname, qtype: log.qtype || 'A', trigger: Date.now() })}
                            className="px-1.5 py-0.5 text-[11px] rounded border border-neutral-200 dark:border-neutral-700 bg-neutral-50 dark:bg-surface-2 text-neutral-600 dark:text-neutral-300 hover:border-cyan-400 hover:text-cyan-600 dark:hover:text-cyan-400 transition-colors"
                            title="立即对此域名发起拨测"
                          >
                            拨测
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })
              ) : (
                <tr>
                  <td colSpan={8} className="py-10 text-center text-neutral-400">
                    {loading ? '正在加载日志...' : '暂无匹配的查询日志'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        {/* 分页控制栏 */}
        <div className="px-3.5 py-2.5 border-t border-neutral-200 dark:border-neutral-800 flex flex-col sm:flex-row items-center justify-between gap-2 text-xs text-neutral-500 dark:text-neutral-400">
          <div className="flex items-center gap-2 font-mono w-full sm:w-auto justify-between sm:justify-start">
            <span>共 {total.toLocaleString()} 条日志流水</span>
            <span className="text-neutral-300 dark:text-neutral-700">|</span>
            <span>每页</span>
            <CustomSelect
              direction="up"
              options={[
                { value: '20', label: '20 条/页' },
                { value: '50', label: '50 条/页' },
                { value: '100', label: '100 条/页' },
              ]}
              value={String(pageSize)}
              onChange={(v) => {
                setPageSize(Number(v));
                setPage(1);
              }}
            />
          </div>

          <div className="flex items-center justify-between sm:justify-end w-full sm:w-auto gap-3">
            <span className="font-mono text-xs tabular-nums">
              第 {page} / {Math.max(1, Math.ceil(total / pageSize))} 页
            </span>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={page <= 1}
                className="min-h-[32px] px-2.5 flex items-center justify-center rounded border border-neutral-300 dark:border-neutral-700 bg-white dark:bg-surface-2 text-neutral-700 dark:text-neutral-300 hover:border-neutral-400 disabled:opacity-40 transition-colors"
                title="上一页"
              >
                <ChevronLeft className="w-3.5 h-3.5 align-icon" />
              </button>
              <button
                type="button"
                onClick={() => setPage((p) => Math.min(Math.ceil(total / pageSize), p + 1))}
                disabled={page >= Math.ceil(total / pageSize)}
                className="min-h-[32px] px-2.5 flex items-center justify-center rounded border border-neutral-300 dark:border-neutral-700 bg-white dark:bg-surface-2 text-neutral-700 dark:text-neutral-300 hover:border-neutral-400 disabled:opacity-40 transition-colors"
                title="下一页"
              >
                <ChevronRight className="w-3.5 h-3.5 align-icon" />
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* 点击日志行按需滑出详情 Drawer */}
      <Drawer
        isOpen={!!selectedLog}
        onClose={() => setSelectedLog(null)}
        title="DNS 报文级深度诊断"
        subtitle={`请求 ID: ${selectedLog?.id}`}
      >
        {selectedLog && renderInspectorContent(selectedLog)}
      </Drawer>

      {/* 快捷动作反馈 Snackbar */}
      {actionSnackbar && (
        <Snackbar
          message={actionSnackbar}
          onClose={() => setActionSnackbar(null)}
        />
      )}

      {/* 清空日志高危二次确认 AlertDialog */}
      <AlertDialog
        isOpen={isClearAlertOpen}
        onClose={() => setIsClearAlertOpen(false)}
        onConfirm={handleClearLogs}
        title="确认清空全部查询历史日志？"
        description="此操作将永久清空内嵌 SQLite 数据库中的所有 DNS 查询日志与请求时延记录，不可恢复。"
        confirmText="确认清空全部日志"
      />
    </div>
  );
};
