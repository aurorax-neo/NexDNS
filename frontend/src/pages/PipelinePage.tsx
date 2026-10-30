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
      case 'qtype_drop':
        return <AlertTriangle className="w-5 h-5 text-amber-400 align-icon" />;
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
      <div className="p-6 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="space-y-4">
          {nodes.map((node, index) => {
            const isLast = index === nodes.length - 1;
            const canToggle = node.id !== 'inbound' && node.id !== 'rewrite';
            return (
              <div key={node.id} className="relative">
                <div
                  className={`flex items-center justify-between p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border transition-all duration-150 ${
                    node.enabled
                      ? 'border-neutral-200 dark:border-neutral-800/80 hover:border-neutral-300 dark:hover:border-neutral-700'
                      : 'opacity-60 border-dashed border-neutral-300 dark:border-neutral-800'
                  }`}
                >
                  <div
                    className="flex items-center gap-4 cursor-pointer flex-1"
                    onClick={() => setSelectedNode(node)}
                  >
                    <div className="p-2.5 rounded-md bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shrink-0 shadow-2xs">
                      {getNodeIcon(node.id)}
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 hover:text-cyan-600 dark:hover:text-cyan-400 transition-colors">
                          {node.name}
                        </span>
                        <span className="px-1.5 py-0.5 text-[10px] font-mono uppercase rounded bg-neutral-200 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400 border border-neutral-300 dark:border-neutral-700/50">
                          {node.stage}
                        </span>
                      </div>
                      <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1">{node.description}</p>
                    </div>
                  </div>

                  <div className="flex items-center gap-4 pl-4 border-l border-neutral-200 dark:border-neutral-800">
                    <button
                      type="button"
                      onClick={() => setSelectedNode(node)}
                      className="text-xs text-neutral-500 hover:text-neutral-900 dark:hover:text-neutral-200 transition-colors"
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

            {selectedNode.id === 'router' && (
              <div className="space-y-4">
                <h4 className="text-xs font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider">
                  NexDNS 原生分流策略与调度拓扑
                </h4>
                <div className="space-y-2 text-xs">
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">大陆直连规则匹配 (Direct Match)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">命中 direct.txt 及大陆直连分组的域名</div>
                    </div>
                    <span className="text-cyan-600 dark:text-cyan-400 font-semibold">&rarr; 转发大陆高速直连组 (mainland)</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">境外代理规则匹配 (Proxy Match)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">命中 proxy.txt 及出海代理分组的域名</div>
                    </div>
                    <span className="text-rose-600 dark:text-rose-400 font-semibold">&rarr; 转发 GFW / 境外代理组 (gfw)</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">局域网私有域名 (Private Match)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">命中 private.txt 及局域网本地域名</div>
                    </div>
                    <span className="text-purple-600 dark:text-purple-400 font-semibold">&rarr; 本地内网优先解析 (private)</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <div>
                      <div className="font-semibold text-neutral-900 dark:text-neutral-100">未命中规则域名 (Fallback Routing)</div>
                      <div className="text-[11px] text-neutral-500 mt-0.5">未分配特定策略的普通域名</div>
                    </div>
                    <span className="text-amber-600 dark:text-amber-400 font-semibold">&rarr; 优先直连，非中国 IP 触发回退</span>
                  </div>
                </div>
              </div>
            )}

            {selectedNode.id === 'anti_pollution' && (
              <div className="p-4 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 space-y-3">
                <h4 className="text-xs font-semibold text-neutral-500 dark:text-neutral-400 uppercase tracking-wider">
                  防污染判断逻辑
                </h4>
                <p className="text-xs text-neutral-700 dark:text-neutral-300 leading-relaxed">
                  当未指定分流规则的域名向大陆上游发起解析，若返回的 IP <strong className="text-rose-600 dark:text-rose-400">不在中国 IPv4/IPv6 CIDR 地址库中</strong>，则判定为 DNS 劫持污染。
                </p>
                <p className="text-xs text-neutral-500 dark:text-neutral-400 leading-relaxed">
                  执行动作：<code className="text-amber-600 dark:text-amber-400 bg-neutral-200 dark:bg-surface-3 px-1 py-0.5 rounded">DropResp</code> 丢弃该虚假解析，流水线无缝回退至 GFW 纯净代理组。
                </p>
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
