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
  const [ruleFiles, setRuleFiles] = useState<RuleFileInfo[]>([]);
  const [selectedNode, setSelectedNode] = useState<PipelineNode | null>(null);
  const [updatingRules, setUpdatingRules] = useState(false);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const [pipelineRes, rulesRes] = await Promise.all([
          api.getPipeline(),
          api.getRoutingRules(),
        ]);
        setNodes(pipelineRes);
        setRuleFiles(rulesRes);
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
    } catch (e) {
      console.error('Failed to toggle pipeline node', e);
    }
  };

  const handleUpdateAllRules = async () => {
    setUpdatingRules(true);
    try {
      await api.updateRoutingRules();
      const updated = await api.getRoutingRules();
      setRuleFiles(updated);
      setSnackbarMessage('全量分流规则文件已成功同步最新版本！');
    } catch (e) {
      console.error('Failed to update routing rules', e);
    } finally {
      setUpdatingRules(false);
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
            const canToggle = node.id !== 'inbound' && node.id !== 'rewrite' && node.id !== 'router';
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

      {/* 分流规则文件实时同步管理 (MosDNS 核心规则) */}
      <div className="p-6 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
        <div className="flex items-center justify-between pb-3 border-b border-neutral-200 dark:border-neutral-800">
          <div>
            <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
              <FileCode className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
              分流与防污染规则集 (Routing Rulesets)
            </h4>
            <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">
              管理直连、代理、私网域名表以及中国 IPv4/IPv6 CIDR 地址段
            </p>
          </div>
          <button
            type="button"
            onClick={handleUpdateAllRules}
            disabled={updatingRules}
            className="inline-flex items-center gap-1.5 min-h-[36px] px-3.5 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-xs disabled:opacity-50"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${updatingRules ? 'animate-spin' : ''}`} />
            {updatingRules ? '正在在线同步...' : '一键全量更新规则集'}
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {ruleFiles.map((rf) => (
            <div
              key={rf.file_name}
              className="p-3.5 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800/80 space-y-2"
            >
              <div className="flex items-center justify-between">
                <span className="font-medium text-xs text-neutral-900 dark:text-neutral-100 truncate">
                  {rf.name}
                </span>
                <span className="font-mono text-[11px] px-1.5 py-0.5 rounded bg-neutral-200 dark:bg-surface-3 text-neutral-700 dark:text-neutral-300">
                  {rf.count.toLocaleString()} 条
                </span>
              </div>
              <div className="text-[11px] text-neutral-500 font-mono flex items-center justify-between">
                <span>体积: {(rf.size_bytes / 1024).toFixed(1)} KB</span>
                <span>更新: {rf.updated_at.split('T')[0] || '-'}</span>
              </div>
            </div>
          ))}
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
                  分流规则流转拓扑
                </h4>
                <div className="space-y-2 text-xs">
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <span className="font-mono text-neutral-700 dark:text-neutral-300">qname $domain_direct</span>
                    <span className="text-cyan-600 dark:text-cyan-400 font-semibold">&rarr; 转发大陆组 (mainland)</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <span className="font-mono text-neutral-700 dark:text-neutral-300">qname $domain_proxy</span>
                    <span className="text-rose-600 dark:text-rose-400 font-semibold">&rarr; 转发 GFW / 境外组 (gfw)</span>
                  </div>
                  <div className="p-3 rounded bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex justify-between items-center">
                    <span className="font-mono text-neutral-700 dark:text-neutral-300">其他域名</span>
                    <span className="text-amber-600 dark:text-amber-400 font-semibold">&rarr; 进入 Fallback 回退序列</span>
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
