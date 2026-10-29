import React, { useEffect, useState, useRef } from 'react';
import {
  Search,
  Pause,
  Play,
  Download,
  Trash2,
  Filter,
  CheckCircle2,
  ShieldAlert,
  Database,
  GitFork,
  Clock,
  Radio,
} from 'lucide-react';
import { api, QueryLogItem } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';
import { AlertDialog } from '../components/ui/AlertDialog';

export const QueryLogPage: React.FC = () => {
  const [logs, setLogs] = useState<QueryLogItem[]>([]);
  const [isLive, setIsLive] = useState(true);
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [selectedLog, setSelectedLog] = useState<QueryLogItem | null>(null);
  const [isClearAlertOpen, setIsClearAlertOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const wsRef = useRef<WebSocket | null>(null);

  // Load initial logs
  const fetchLogs = async () => {
    try {
      const res = await api.getQueryLogs({
        page: 1,
        page_size: 50,
        search: search || undefined,
        status: statusFilter,
      });
      setLogs(res.items);
    } catch (e) {
      console.error('Failed to load logs', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLogs();
  }, [search, statusFilter]);

  // WebSocket live streaming
  useEffect(() => {
    if (!isLive) {
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
      return;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/v1/ws/querylog`;
    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onmessage = (event) => {
      try {
        const item: QueryLogItem = JSON.parse(event.data);
        setLogs((prev) => [item, ...prev.slice(0, 199)]);
      } catch (e) {
        console.error('Failed to parse WS log', e);
      }
    };

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [isLive]);

  const handleClearLogs = async () => {
    try {
      await api.clearQueryLogs();
      setLogs([]);
    } catch (e) {
      console.error('Failed to clear logs', e);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'allowed':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-emerald-950/60 text-emerald-400 border border-emerald-800/40">
            <CheckCircle2 className="w-3 h-3 align-icon" />
            放行
          </span>
        );
      case 'blocked':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-rose-950/60 text-rose-400 border border-rose-800/40">
            <ShieldAlert className="w-3 h-3 align-icon" />
            已拦截
          </span>
        );
      case 'cached':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-amber-950/60 text-amber-400 border border-amber-800/40">
            <Database className="w-3 h-3 align-icon" />
            缓存
          </span>
        );
      case 'rewritten':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-indigo-950/60 text-indigo-400 border border-indigo-800/40">
            <GitFork className="w-3 h-3 align-icon" />
            重写
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded font-medium bg-neutral-800 text-neutral-400">
            {status}
          </span>
        );
    }
  };

  return (
    <div className="space-y-4 animate-in fade-in-50 duration-200">
      {/* 工具筛选栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-4 rounded-lg bg-surface-1 border border-neutral-800">
        <div className="flex flex-wrap items-center gap-3 flex-1">
          {/* 搜索框 */}
          <div className="relative min-w-[240px] max-w-sm flex-1">
            <Search className="w-4 h-4 text-neutral-500 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索域名关键字..."
              className="w-full min-h-[38px] pl-9 pr-3 text-xs bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-cyan-500/60 transition-colors"
            />
          </div>

          {/* 状态过滤 Select */}
          <div className="flex items-center gap-2">
            <span className="text-xs text-neutral-400">状态:</span>
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              className="min-h-[38px] px-3 text-xs bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-200 focus:outline-none focus:border-cyan-500/60"
            >
              <option value="all">全部记录</option>
              <option value="allowed">正常放行</option>
              <option value="blocked">已拦截 (Adblock)</option>
              <option value="cached">命中缓存</option>
              <option value="rewritten">本地重写</option>
            </select>
          </div>
        </div>

        {/* 右侧动作按钮集合 */}
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setIsLive(!isLive)}
            className={`inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs rounded-md border transition-colors ${
              isLive
                ? 'bg-emerald-950/50 border-emerald-800/60 text-emerald-400'
                : 'bg-surface-2 border-neutral-700 text-neutral-400 hover:text-neutral-200'
            }`}
          >
            {isLive ? <Radio className="w-3.5 h-3.5 animate-pulse align-icon" /> : <Play className="w-3.5 h-3.5 align-icon" />}
            {isLive ? '实时流运行中' : '已暂停实时流'}
          </button>

          <a
            href="/api/v1/querylog/export"
            download="nexdns-querylog.csv"
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs text-neutral-300 hover:text-neutral-100 bg-surface-2 border border-neutral-700 rounded-md transition-colors"
          >
            <Download className="w-3.5 h-3.5 align-icon" />
            导出 CSV
          </a>

          <button
            type="button"
            onClick={() => setIsClearAlertOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3 text-xs text-red-400 hover:text-red-300 hover:bg-red-950/30 bg-surface-2 border border-red-900/40 rounded-md transition-colors"
          >
            <Trash2 className="w-3.5 h-3.5 align-icon" />
            清空日志
          </button>
        </div>
      </div>

      {/* 查询日志主表格 */}
      <div className="rounded-lg bg-surface-1 border border-neutral-800 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-surface-2/60 border-b border-neutral-800 text-neutral-400 uppercase font-medium">
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
            <tbody className="divide-y divide-neutral-800/60 font-normal">
              {logs.length > 0 ? (
                logs.map((log) => {
                  const timeStr = log.timestamp.split('T')[1]?.substring(0, 8) || log.timestamp;
                  return (
                    <tr
                      key={log.id}
                      onClick={() => setSelectedLog(log)}
                      className="hover:bg-surface-2/60 cursor-pointer transition-colors duration-100"
                    >
                      <td className="py-3 px-4 font-mono text-neutral-400 tabular-nums whitespace-nowrap">
                        {timeStr}
                      </td>
                      <td className="py-3 px-4 whitespace-nowrap">
                        {getStatusBadge(log.status)}
                      </td>
                      <td className="py-3 px-4">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-neutral-200 font-medium">{log.qname}</span>
                          <span className="px-1 py-0.5 text-[10px] font-mono rounded bg-surface-3 text-neutral-400">
                            {log.qtype}
                          </span>
                        </div>
                      </td>
                      <td className="py-3 px-4 font-mono text-neutral-300 tabular-nums whitespace-nowrap">
                        {log.client_ip}
                      </td>
                      <td className="py-3 px-4 text-neutral-400 truncate max-w-[200px]">
                        {log.matched_rule ? (
                          <span className="text-rose-400 font-mono">{log.matched_rule}</span>
                        ) : log.routing_group ? (
                          <span className="text-cyan-400 font-mono">组: {log.routing_group}</span>
                        ) : (
                          '-'
                        )}
                      </td>
                      <td className="py-3 px-4 font-mono text-neutral-300 tabular-nums truncate max-w-[180px]">
                        {log.response_ip || log.rcode}
                      </td>
                      <td className="py-3 px-4 text-right font-mono tabular-nums text-neutral-300 whitespace-nowrap">
                        {log.duration_ms < 1 ? '< 1 ms' : `${log.duration_ms.toFixed(1)} ms`}
                      </td>
                    </tr>
                  );
                })
              ) : (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-neutral-500">
                    {loading ? '正在加载日志...' : '暂无匹配的查询日志'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* 单条日志深度诊断 Drawer */}
      <Drawer
        isOpen={!!selectedLog}
        onClose={() => setSelectedLog(null)}
        title="DNS 报文级深度诊断"
        subtitle={`请求 ID: ${selectedLog?.id}`}
      >
        {selectedLog && (
          <div className="space-y-5 text-xs">
            <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800 space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-neutral-400">查询域名:</span>
                <span className="font-mono text-neutral-100 font-semibold">{selectedLog.qname}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-400">记录类型:</span>
                <span className="font-mono text-neutral-100">{selectedLog.qtype}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-400">处理状态:</span>
                <div>{getStatusBadge(selectedLog.status)}</div>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-400">客户端来源:</span>
                <span className="font-mono text-neutral-100">{selectedLog.client_ip}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-neutral-400">全流程时延:</span>
                <span className="font-mono text-cyan-400 font-semibold tabular-nums">
                  {selectedLog.duration_ms.toFixed(2)} ms
                </span>
              </div>
            </div>

            {selectedLog.response_ip && (
              <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800 space-y-2">
                <span className="text-neutral-400 block font-semibold uppercase tracking-wider">
                  最终首选返回 IP (SmartDNS 测速置顶)
                </span>
                <div className="font-mono text-sm text-emerald-400 font-semibold bg-surface-1 p-2.5 rounded border border-neutral-800">
                  {selectedLog.response_ip}
                </div>
              </div>
            )}

            {selectedLog.matched_rule && (
              <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800 space-y-2">
                <span className="text-neutral-400 block font-semibold uppercase tracking-wider">
                  命中拦截规则 (AdGuard)
                </span>
                <div className="font-mono text-xs text-rose-400 bg-surface-1 p-2.5 rounded border border-neutral-800">
                  {selectedLog.matched_rule}
                </div>
              </div>
            )}

            {selectedLog.routing_group && (
              <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800 space-y-2">
                <span className="text-neutral-400 block font-semibold uppercase tracking-wider">
                  分流决策链路 (MosDNS)
                </span>
                <div className="font-mono text-xs text-cyan-400 bg-surface-1 p-2.5 rounded border border-neutral-800">
                  调度至上游组: {selectedLog.routing_group}
                </div>
              </div>
            )}
          </div>
        )}
      </Drawer>

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
