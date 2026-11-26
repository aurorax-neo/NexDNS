import React, { useEffect, useState } from 'react';
import {
  GitFork,
  CheckCircle2,
  Shield,
  Database,
  Compass,
  Zap,
  AlertTriangle,
  SlidersHorizontal,
  Layers,
  Info,
  RefreshCw,
  FileCode,
} from 'lucide-react';
import { api, PipelineNode, RuleFileInfo } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';
import { Switch } from '../components/ui/Switch';
import { Snackbar } from '../components/ui/Snackbar';

export const PipelinePage: React.FC = () => {
  const [nodes, setNodes] = useState<PipelineNode[]>([]);
  const [selectedNode, setSelectedNode] = useState<PipelineNode | null>(null);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const pipelineRes = await api.getPipeline();
        setNodes(pipelineRes);
      } catch (e) {
        console.error('Failed to load pipeline', e);
      }
    };
    load();
  }, []);

  const handleToggleStage = async (nodeId: string, currentEnabled: boolean) => {
    const nextState = !currentEnabled;
    try {
      await api.togglePipelineStage(nodeId, nextState);
      setNodes((prev) =>
        prev.map((n) => (n.id === nodeId ? { ...n, enabled: nextState } : n))
      );
      setSnackbarMessage(`已${nextState ? '启用' : '停用'}节点`);
    } catch (e: any) {
      console.error('Failed to toggle pipeline node', e);
      setSnackbarMessage(e?.message || '切换节点失败，请检查上游组配置');
    }
  };

  const getNodeIcon = (id: string) => {
    switch (id) {
      case 'inbound':
        return <Layers className="w-5 h-5 text-cyan-400 align-icon" />;
      case 'rewrite':
        return <GitFork className="w-5 h-5 text-indigo-400 align-icon" />;
      case 'adblock':
        return <Shield className="w-5 h-5 text-rose-400 align-icon" />;
      case 'cache':
        return <Database className="w-5 h-5 text-amber-400 align-icon" />;
      case 'router':
        return <Compass className="w-5 h-5 text-emerald-400 align-icon" />;
      case 'anti_pollution':
        return <CheckCircle2 className="w-5 h-5 text-teal-400 align-icon" />;
      case 'speed_check':
        return <Zap className="w-5 h-5 text-yellow-400 align-icon" />;
      default:
        return <Info className="w-5 h-5 text-neutral-400 align-icon" />;
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 头部说明 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <SlidersHorizontal className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            流水线分流拓扑与节点热重载
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-2xl leading-relaxed">
            查询报文在内存中按顺序依次经过以下节点。各节点支持就地独立启用/停用，变更即时在单内核内存中生效。
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="px-2.5 py-1 text-xs rounded border border-neutral-200 dark:border-neutral-700 bg-neutral-100 dark:bg-surface-2 text-neutral-700 dark:text-neutral-300">
            总节点数: <span className="tabular-nums font-mono text-cyan-600 dark:text-cyan-400 font-semibold">{nodes.length}</span>
          </span>
        </div>
      </div>

      {/* 拓扑流转主链卡片 */}
      <div className="p-4 sm:p-6 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="space-y-4">
          {nodes.map((node, index) => {
            const isLast = index === nodes.length - 1;
            const canToggle = node.id !== 'inbound' && node.id !== 'rewrite';
            return (
              <div key={node.id} className="relative">
                <div
                  className={`flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border transition-all duration-150 ${
                    node.enabled
                      ? 'border-neutral-200 dark:border-neutral-800/80 hover:border-neutral-300 dark:hover:border-neutral-700'
                      : 'opacity-60 border-dashed border-neutral-300 dark:border-neutral-800'
                  }`}
                >
                  <div
                    className="flex items-start sm:items-center gap-3.5 sm:gap-4 cursor-pointer flex-1 min-w-0"
                    onClick={() => setSelectedNode(node)}
                  >
                    <div className="p-2.5 rounded-md bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shrink-0 shadow-2xs">
                      {getNodeIcon(node.id)}
                    </div>
                    <div className="min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 hover:text-cyan-600 dark:hover:text-cyan-400 transition-colors">
                          {node.name}
                        </span>
                        <span className="px-1.5 py-0.5 text-[10px] font-mono uppercase rounded bg-neutral-200 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400 border border-neutral-300 dark:border-neutral-700/50 shrink-0">
                          {node.stage}
                        </span>
                      </div>
                      <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1">{node.description}</p>
                    </div>
                  </div>

                  <div className="flex items-center justify-between sm:justify-end gap-4 pt-2.5 sm:pt-0 sm:pl-4 border-t sm:border-t-0 sm:border-l border-neutral-200 dark:border-neutral-800 shrink-0">
                    <button
                      type="button"
                      onClick={() => setSelectedNode(node)}
                      className="text-xs whitespace-nowrap shrink-0 text-neutral-500 hover:text-neutral-900 dark:hover:text-neutral-200 transition-colors"
                    >
                      规则配置 &rarr;
                    </button>
                    {canToggle ? (
                      <Switch
                        checked={node.enabled}
                        onChange={() => handleToggleStage(node.id, node.enabled)}
                        ariaLabel={`切换 ${node.name}`}
                      />
                    ) : (
                      <span className="text-[11px] font-mono text-neutral-400 px-2 py-1 bg-neutral-200 dark:bg-neutral-800 rounded">
                        核心常驻
                      </span>
                    )}
                  </div>
                </div>

                {!isLast && (
                  <div className="flex justify-center my-1.5">
                    <div className="w-0.5 h-4 bg-neutral-200 dark:bg-neutral-800" />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* 节点深度配置 Drawer */}
      <Drawer
        isOpen={!!selectedNode}
        onClose={() => setSelectedNode(null)}
        title={selectedNode?.name || '节点详情'}
        subtitle={`阶段: ${selectedNode?.stage} | ID: ${selectedNode?.id}`}
      >
        {selectedNode && (
          <div className="space-y-6">
            <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
              <h4 className="text-xs font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider mb-2">
                插件职责与行为描述
              </h4>
              <p className="text-sm text-neutral-800 dark:text-neutral-200 leading-relaxed">{selectedNode.description}</p>
            </div>

            {selectedNode.id === 'cache' && (
              <div className="space-y-4">
                <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-3">
                  <h4 className="text-xs font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider">
                    高性能智能缓存说明
                  </h4>
                  <div className="text-xs text-neutral-700 dark:text-neutral-300 space-y-2 leading-relaxed">
                    <p>• <strong>标准 TTL 生命周期</strong>: 按照上游权威解析真实 TTL 缓存（默认 60s~1800s），未过期秒返，过期自动回源拉取最新解析，杜绝高频无效回源。</p>
                    <p>• <strong>Prefetch-Domain (安全静默预取)</strong>: 高频访问热点域名在 TTL 剩余低于 20% 时由内核自动复用完整分流 + 防污染 + 下游地址族过滤流水线静默预取刷新，绝不污染干净缓存。</p>
                  </div>
                </div>

                <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex items-center justify-between">
                  <div>
                    <div className="text-xs font-semibold text-neutral-900 dark:text-neutral-100">立即清空运行时缓存</div>
                    <div className="text-[11px] text-neutral-500 mt-0.5">立即注销当前单内核内存中所有的 DNS 解析缓存条目</div>
                  </div>
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        await api.flushCache();
                        setSnackbarMessage('已成功清空当前单内核全部内存 DNS 缓存！');
                      } catch (e: any) {
                        setSnackbarMessage(e?.message || '清空缓存失败');
                      }
                    }}
                    className="min-h-[34px] px-3.5 text-xs font-medium rounded-md bg-amber-500 hover:bg-amber-400 text-white transition-colors shadow-2xs"
                  >
                    清空内存缓存
                  </button>
                </div>
              </div>
            )}

            {selectedNode.id === 'router' && (
              <div className="space-y-4">
                <h4 className="text-xs font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider">
                  一体化智能分流与防污染调度拓扑
                </h4>
                <div className="space-y-2 text-xs">
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center gap-3">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">大陆直连规则匹配 (Direct Match)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">最长特异度命中 custom_direct / direct.txt 的国内域名</div>
                    </div>
                    <span className="text-cyan-600 dark:text-cyan-400 font-semibold shrink-0">&rarr; 大陆直连组 (mainland)</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center gap-3">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">境外防污染规则匹配 (Proxy Match)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">最长特异度命中 custom_proxy / proxy.txt（含 google.cn 等 .cn 境外域），代理瞬断仅回退海外直连加密组，严禁回退大陆</div>
                    </div>
                    <span className="text-rose-600 dark:text-rose-400 font-semibold shrink-0">&rarr; GFW 组 &rarr; 海外备用组</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center gap-3">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">局域网私有域名 (Private Match)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">命中 private.txt 及 RFC 保留域，本地权威拦截</div>
                    </div>
                    <span className="text-purple-600 dark:text-purple-400 font-semibold shrink-0">&rarr; 本地权威响应</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center gap-3">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">未命中域名 (非中国 IP 自动抛弃防污染回退)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">先探测 mainland：含中国 IP 则采纳；非中国 IP 立即彻底丢弃（绝不缓存污染 IP）并切入 gfw &rarr; overseas 纯净加密链路</div>
                    </div>
                    <span className="text-amber-600 dark:text-amber-400 font-semibold shrink-0">&rarr; 防污染硬隔离回退</span>
                  </div>
                </div>
              </div>
            )}

            {selectedNode.id === 'speed_check' && (
              <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-3">
                <h4 className="text-xs font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider">
                  TCP 端口建连测速优选
                </h4>
                <p className="text-xs text-neutral-700 dark:text-neutral-300 leading-relaxed">
                  当上游响应中包含多个 A 或 AAAA 记录时，内核自动启动非阻塞异步 TCP SYN 握手探测（优先 80 与 443 端口）。
                </p>
                <div className="grid grid-cols-2 gap-3 mt-3">
                  <div className="p-2.5 rounded bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 text-center">
                    <div className="text-xs text-neutral-500 dark:text-neutral-400">探测超时</div>
                    <div className="text-sm font-semibold font-mono text-neutral-900 dark:text-neutral-100 mt-1">400 ms</div>
                  </div>
                  <div className="p-2.5 rounded bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 text-center">
                    <div className="text-xs text-neutral-500 dark:text-neutral-400">测速评分缓存</div>
                    <div className="text-sm font-semibold font-mono text-neutral-900 dark:text-neutral-100 mt-1">5 分钟</div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </Drawer>

      {snackbarMessage && (
        <Snackbar message={snackbarMessage} onClose={() => setSnackbarMessage(null)} />
      )}
    </div>
  );
};
