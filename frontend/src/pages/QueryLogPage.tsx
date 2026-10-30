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
  User,
  ExternalLink,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { api, QueryLogItem } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';
import { AlertDialog } from '../components/ui/AlertDialog';
import { CustomSelect } from '../components/ui/CustomSelect';
import { Snackbar } from '../components/ui/Snackbar';

export interface QueryLogPageProps {
  onNavigate?: (tab: string) => void;
}

export const QueryLogPage: React.FC<QueryLogPageProps> = ({ onNavigate }) => {
  const [logs, setLogs] = useState<QueryLogItem[]>([]);
  const [loggingEnabled, setLoggingEnabled] = useState<boolean>(true);
  const [loadingSettings, setLoadingSettings] = useState<boolean>(true);
  const [isLive, setIsLive] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [selectedLog, setSelectedLog] = useState<QueryLogItem | null>(null);
  const [isClearAlertOpen, setIsClearAlertOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);
  const [total, setTotal] = useState(0);
  const [actionSnackbar, setActionSnackbar] = useState<string | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);

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
      });
      setLogs(res.items);
      setTotal(res.total);
    } catch (e) {
      console.error('Failed to load logs', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!loggingEnabled) return;
    fetchLogs();
  }, [search, statusFilter, page, pageSize, loggingEnabled]);

  const handleFilterChange = (val: string) => {
    setStatusFilter(val);
    setPage(1);
  };

  // HTTPS SSE (Server-Sent Events) live streaming (默认关闭，用户手动开启后连接)
  useEffect(() => {
    if (!isLive || !loggingEnabled) {
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
      return;
    }

    const es = new EventSource('/api/v1/sse/querylog');
    eventSourceRef.current = es;

    es.onmessage = (event) => {
      try {
        const item: QueryLogItem = JSON.parse(event.data);
        if (page === 1 && !search && statusFilter === 'all') {
          setLogs((prev) => [item, ...prev.slice(0, pageSize - 1)]);
          setTotal((prev) => prev + 1);
        }
      } catch (e) {
        console.error('Failed to parse SSE log', e);
      }
    };

    return () => {
      es.close();
      eventSourceRef.current = null;
    };
  }, [isLive, loggingEnabled, page, pageSize, search, statusFilter]);

  const handleClearLogs = async () => {
    try {
      await api.clearQueryLogs();
      setLogs([]);
    } catch (e) {
      console.error('Failed to clear logs', e);
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
    <div className="space-y-4 animate-in fade-in-50 duration-200">
      {/* 工具筛选栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-4 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex flex-wrap items-center gap-3 flex-1">
          {/* 搜索框 */}
          <div className="relative min-w-[220px] max-w-sm flex-1">
            <Search className="w-4 h-4 text-neutral-400 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
            <input
              type="text"
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setPage(1);
              }}
              placeholder="搜索域名..."
              className="w-full min-h-[38px] pl-9 pr-3 text-xs bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 rounded-md text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500 transition-colors"
            />
          </div>

          {/* 状态过滤 CustomSelect (100% 拒绝原生 Select) */}
          <CustomSelect
            options={statusOptions}
            value={statusFilter}
            onChange={handleFilterChange}
          />
        </div>

        {/* 右侧动作按钮集合 */}
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={async () => {
              setIsRefreshing(true);
              await fetchLogs();
              setTimeout(() => setIsRefreshing(false), 500);
            }}
            disabled={isRefreshing}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-colors active:scale-95 disabled:opacity-60"
            title="手动刷新日志"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${isRefreshing ? 'animate-spin text-cyan-600 dark:text-cyan-400' : ''}`} />
            刷新
          </button>

          <button
            type="button"
            onClick={() => setIsLive(!isLive)}
            className={`inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs rounded-md border transition-colors ${
              isLive
                ? 'bg-emerald-50 dark:bg-emerald-950/50 border-emerald-200 dark:border-emerald-800/60 text-emerald-700 dark:text-emerald-400'
                : 'bg-neutral-100 dark:bg-surface-2 border-neutral-300 dark:border-neutral-700 text-neutral-600 dark:text-neutral-400'
            }`}
          >
            {isLive ? <Radio className="w-3.5 h-3.5 animate-pulse align-icon" /> : <Play className="w-3.5 h-3.5 align-icon" />}
            {isLive ? '实时流' : '已暂停'}
          </button>

          <a
            href="/api/v1/querylog/export"
            download="nexdns-querylog.csv"
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md transition-colors"
          >
            <Download className="w-3.5 h-3.5 align-icon" />
            导出 CSV
          </a>

          <button
            type="button"
            onClick={() => setIsClearAlertOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 bg-neutral-100 dark:bg-surface-2 border border-red-200 dark:border-red-900/40 rounded-md transition-colors"
          >
            <Trash2 className="w-3.5 h-3.5 align-icon" />
            清空日志
          </button>
        </div>
      </div>

      {/* 查询日志主表格 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-neutral-50 dark:bg-surface-2/60 border-b border-neutral-200 dark:border-neutral-800 text-neutral-500 dark:text-neutral-400 uppercase font-medium">
              <tr>
                <th className="py-3 px-4">请求时间</th>
                <th className="py-3 px-4">状态</th>
                <th className="py-3 px-4">查询域名与类型</th>
                <th className="py-3 px-4">客户端 IP</th>
                <th className="py-3 px-4">分流 / 匹配规则</th>
                <th className="py-3 px-4">响应结果</th>
                <th className="py-3 px-4 text-right">解析时延</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200 dark:divide-neutral-800/60 font-normal">
              {logs.length > 0 ? (
                logs.map((log) => {
                  const timeStr = log.timestamp.split('T')[1]?.substring(0, 8) || log.timestamp;
                  return (
                    <tr
                      key={log.id}
                      onClick={() => setSelectedLog(log)}
                      className="hover:bg-neutral-50 dark:hover:bg-surface-2/60 cursor-pointer transition-colors duration-100"
                    >
                      <td className="py-3 px-4 font-mono text-neutral-500 dark:text-neutral-400 tabular-nums whitespace-nowrap">
                        {timeStr}
                      </td>
                      <td className="py-3 px-4 whitespace-nowrap">
                        {getStatusBadge(log.status)}
                      </td>
                      <td className="py-3 px-4">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-neutral-800 dark:text-neutral-200 font-medium">{log.qname}</span>
                          <span className="px-1 py-0.5 text-[10px] font-mono rounded bg-neutral-100 dark:bg-surface-3 text-neutral-500 dark:text-neutral-400">
                            {log.qtype}
                          </span>
                        </div>
                      </td>
                      <td className="py-3 px-4 font-mono text-neutral-600 dark:text-neutral-300 tabular-nums whitespace-nowrap">
                        {log.client_ip}
                      </td>
                      <td className="py-3 px-4 text-neutral-500 dark:text-neutral-400 truncate max-w-[200px]">
                        {log.matched_rule ? (
                          <span className="text-rose-600 dark:text-rose-400 font-mono">{log.matched_rule}</span>
                        ) : log.routing_group ? (
                          <span className="text-cyan-600 dark:text-cyan-400 font-mono">组: {log.routing_group}</span>
                        ) : (
                          '-'
                        )}
                      </td>
                      <td className="py-3 px-4 font-mono text-neutral-700 dark:text-neutral-300 tabular-nums truncate max-w-[180px]">
                        {log.response_ip ? (
                          log.response_ip
                        ) : log.rcode === 'No Error' ? (
                          <span className="text-neutral-400">无记录 (NODATA)</span>
                        ) : (
                          <span className="text-rose-500 font-medium">{log.rcode}</span>
                        )}
                      </td>
                      <td className="py-3 px-4 text-right font-mono tabular-nums text-neutral-700 dark:text-neutral-300 whitespace-nowrap">
                        {log.duration_ms < 1 ? '< 1 ms' : `${log.duration_ms.toFixed(1)} ms`}
                      </td>
                    </tr>
                  );
                })
              ) : (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-neutral-400">
                    {loading ? '正在加载日志...' : '暂无匹配的查询日志'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        {/* 分页控制栏 */}
        <div className="px-4 py-3 border-t border-neutral-200 dark:border-neutral-800 flex flex-wrap items-center justify-between gap-3 text-xs text-neutral-500 dark:text-neutral-400">
          <div className="flex items-center gap-2 font-mono">
            <span>共 {total.toLocaleString()} 条日志流水</span>
            <span className="text-neutral-300 dark:text-neutral-700">|</span>
            <span>每页</span>
            <CustomSelect
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

          <div className="flex items-center gap-2">
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

      {/* 单条日志深度诊断 Drawer (带就地快捷动作) */}
      <Drawer
        isOpen={!!selectedLog}
        onClose={() => setSelectedLog(null)}
        title="DNS 报文级深度诊断"
        subtitle={`请求 ID: ${selectedLog?.id}`}
      >
        {selectedLog && (
          <div className="space-y-5 text-xs">
            {/* 核心快捷就地操作栏 */}
            <div className="p-3.5 rounded-lg bg-neutral-100 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-2">
              <span className="text-[11px] font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider block">
                对该域名的快捷动作
              </span>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => handleQuickRule(selectedLog.qname, 'block')}
                  className="inline-flex items-center gap-1.5 min-h-[34px] px-3 text-xs font-medium bg-rose-600 hover:bg-rose-500 text-white rounded-md transition-colors"
                >
                  <Shield className="w-3.5 h-3.5 align-icon" />
                  拦截此域名 (Blacklist)
                </button>
                <button
                  type="button"
                  onClick={() => handleQuickRule(selectedLog.qname, 'whitelist')}
                  className="inline-flex items-center gap-1.5 min-h-[34px] px-3 text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-white rounded-md transition-colors"
                >
                  <Check className="w-3.5 h-3.5 align-icon" />
                  放行白名单 (Whitelist)
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setSearch(selectedLog.qname);
                    setSelectedLog(null);
                  }}
                  className="inline-flex items-center gap-1.5 min-h-[34px] px-3 text-xs text-neutral-700 dark:text-neutral-300 hover:bg-neutral-200 dark:hover:bg-surface-3 bg-white dark:bg-surface-1 border border-neutral-300 dark:border-neutral-700 rounded-md transition-colors"
                >
                  <Search className="w-3.5 h-3.5 align-icon" />
                  筛选此域名
                </button>
              </div>
            </div>

            {/* 报文基础信息 */}
            <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-neutral-500 dark:text-neutral-400">查询域名:</span>
                <span className="font-mono text-neutral-900 dark:text-neutral-100 font-semibold">{selectedLog.qname}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-500 dark:text-neutral-400">记录类型:</span>
                <span className="font-mono text-neutral-900 dark:text-neutral-100">{selectedLog.qtype}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-500 dark:text-neutral-400">处理状态:</span>
                <div>{getStatusBadge(selectedLog.status)}</div>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-500 dark:text-neutral-400">客户端来源:</span>
                <span className="font-mono text-neutral-900 dark:text-neutral-100">{selectedLog.client_ip}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-500 dark:text-neutral-400">全流程时延:</span>
                <span className="font-mono text-cyan-600 dark:text-cyan-400 font-semibold tabular-nums">
                  {selectedLog.duration_ms.toFixed(2)} ms
                </span>
              </div>
            </div>

            {selectedLog.response_ip && (
              <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-2">
                <span className="text-neutral-500 dark:text-neutral-400 block font-semibold uppercase tracking-wider">
                  首选返回 IP (TCP 测速优选)
                </span>
                <div className="font-mono text-sm text-emerald-600 dark:text-emerald-400 font-semibold bg-white dark:bg-surface-1 p-2.5 rounded border border-neutral-200 dark:border-neutral-800">
                  {selectedLog.response_ip}
                </div>
              </div>
            )}

            {selectedLog.matched_rule && (
              <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-2">
                <span className="text-neutral-500 dark:text-neutral-400 block font-semibold uppercase tracking-wider">
                  命中拦截规则
                </span>
                <div className="font-mono text-xs text-rose-600 dark:text-rose-400 bg-white dark:bg-surface-1 p-2.5 rounded border border-neutral-200 dark:border-neutral-800">
                  {selectedLog.matched_rule}
                </div>
              </div>
            )}

            {selectedLog.routing_group && (
              <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-2">
                <span className="text-neutral-500 dark:text-neutral-400 block font-semibold uppercase tracking-wider">
                  分流决策链路
                </span>
                <div className="font-mono text-xs text-cyan-600 dark:text-cyan-400 bg-white dark:bg-surface-1 p-2.5 rounded border border-neutral-200 dark:border-neutral-800">
                  调度至上游组: {selectedLog.routing_group}
                </div>
              </div>
            )}
          </div>
        )}
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
