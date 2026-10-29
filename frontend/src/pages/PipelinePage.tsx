import React, { useEffect, useState } from 'react';
import {
  GitFork,
  ArrowRight,
  CheckCircle2,
  Shield,
  Database,
  Compass,
  Zap,
  AlertTriangle,
  SlidersHorizontal,
  Layers,
  Info,
} from 'lucide-react';
import { api, PipelineNode } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';

export const PipelinePage: React.FC = () => {
  const [nodes, setNodes] = useState<PipelineNode[]>([]);
  const [selectedNode, setSelectedNode] = useState<PipelineNode | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const res = await api.getPipeline();
        setNodes(res);
      } catch (e) {
        console.error('Failed to load pipeline', e);
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

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
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-surface-1 border border-neutral-800">
        <div>
          <h3 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
            <SlidersHorizontal className="w-4 h-4 text-cyan-400 align-icon" />
            流水线插件化分流拓扑 (Pipeline Flow)
          </h3>
          <p className="text-xs text-neutral-400 mt-1 max-w-2xl leading-relaxed">
            基于 Rust 原生单内核的单次查询流水线，在内存中零拷贝流转。点击任一流水线插件节点可查看其执行条件、匹配规则与上下游关系。
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="px-2.5 py-1 text-xs rounded border border-neutral-700 bg-surface-2 text-neutral-300">
            插件总数: <span className="tabular-nums font-mono text-cyan-400 font-semibold">{nodes.length}</span>
          </span>
        </div>
      </div>

      {/* 拓扑流转主链卡片 */}
      <div className="p-6 rounded-lg bg-surface-1 border border-neutral-800">
        <div className="space-y-4">
          {nodes.map((node, index) => {
            const isLast = index === nodes.length - 1;
            return (
              <div key={node.id} className="relative">
                <div
                  onClick={() => setSelectedNode(node)}
                  className="flex items-center justify-between p-4 rounded-lg bg-surface-2 border border-neutral-800/80 hover:border-neutral-700 hover:bg-surface-3/50 cursor-pointer transition-all duration-150 group"
                >
                  <div className="flex items-center gap-4">
                    <div className="p-2.5 rounded-md bg-surface-1 border border-neutral-800 shrink-0">
                      {getNodeIcon(node.id)}
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-neutral-100 group-hover:text-cyan-400 transition-colors">
                          {node.name}
                        </span>
                        <span className="px-1.5 py-0.5 text-[10px] font-mono uppercase rounded bg-neutral-800 text-neutral-400 border border-neutral-700/50">
                          {node.stage}
                        </span>
                      </div>
                      <p className="text-xs text-neutral-400 mt-1">{node.description}</p>
                    </div>
                  </div>

                  <div className="flex items-center gap-3">
                    <span
                      className={`px-2 py-0.5 text-xs rounded font-medium ${
                        node.enabled
                          ? 'bg-emerald-950/60 text-emerald-400 border border-emerald-800/40'
                          : 'bg-neutral-800 text-neutral-400'
                      }`}
                    >
                      {node.enabled ? '活跃执行' : '已停用'}
                    </span>
                    <span className="text-xs text-neutral-500 group-hover:text-neutral-300 transition-colors">
                      详情 &rarr;
                    </span>
                  </div>
                </div>

                {!isLast && (
                  <div className="flex justify-center my-1.5">
                    <div className="w-0.5 h-4 bg-neutral-800 group-hover:bg-neutral-700" />
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
            <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800">
              <h4 className="text-xs font-semibold text-neutral-400 uppercase tracking-wider mb-2">
                插件职责与行为描述
              </h4>
              <p className="text-sm text-neutral-200 leading-relaxed">{selectedNode.description}</p>
            </div>

            {selectedNode.id === 'router' && (
              <div className="space-y-4">
                <h4 className="text-xs font-semibold text-neutral-400 uppercase tracking-wider">
                  MosDNS 分流规则流转拓扑
                </h4>
                <div className="space-y-2 text-xs">
                  <div className="p-3 rounded bg-surface-2 border border-neutral-800 flex justify-between items-center">
                    <span className="font-mono text-neutral-300">qname $domain_direct</span>
                    <span className="text-cyan-400 font-semibold">&rarr; 转发大陆组 (mainland)</span>
                  </div>
                  <div className="p-3 rounded bg-surface-2 border border-neutral-800 flex justify-between items-center">
                    <span className="font-mono text-neutral-300">qname $domain_proxy</span>
                    <span className="text-rose-400 font-semibold">&rarr; 转发 GFW / 境外组 (gfw)</span>
                  </div>
                  <div className="p-3 rounded bg-surface-2 border border-neutral-800 flex justify-between items-center">
                    <span className="font-mono text-neutral-300">其他域名</span>
                    <span className="text-amber-400 font-semibold">&rarr; 进入 Fallback 回退序列</span>
                  </div>
                </div>
              </div>
            )}

            {selectedNode.id === 'anti_pollution' && (
              <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800 space-y-3">
                <h4 className="text-xs font-semibold text-neutral-400 uppercase tracking-wider">
                  防污染判断逻辑
                </h4>
                <p className="text-xs text-neutral-300 leading-relaxed">
                  当未指定分流规则的域名向大陆上游发起解析，若返回的 IP <strong className="text-rose-400">不在中国 IPv4/IPv6 CIDR 地址库中</strong>，则判定为 DNS 劫持污染。
                </p>
                <p className="text-xs text-neutral-400 leading-relaxed">
                  执行动作：<code className="text-amber-400 bg-surface-3 px-1 py-0.5 rounded">DropResp</code> 丢弃该虚假解析，流水线无缝回退至 GFW 纯净代理组。
                </p>
              </div>
            )}

            {selectedNode.id === 'speed_check' && (
              <div className="p-4 rounded-lg bg-surface-2 border border-neutral-800 space-y-3">
                <h4 className="text-xs font-semibold text-neutral-400 uppercase tracking-wider">
                  TCP 端口建连测速 (SmartDNS)
                </h4>
                <p className="text-xs text-neutral-300 leading-relaxed">
                  当上游响应中包含多个 A 或 AAAA 记录时，内核自动启动非阻塞异步 TCP SYN 握手探测（优先 80 与 443 端口）。
                </p>
                <div className="grid grid-cols-2 gap-3 mt-3">
                  <div className="p-2.5 rounded bg-surface-1 border border-neutral-800 text-center">
                    <div className="text-xs text-neutral-400">探测超时</div>
                    <div className="text-sm font-semibold font-mono text-neutral-100 mt-1">400 ms</div>
                  </div>
                  <div className="p-2.5 rounded bg-surface-1 border border-neutral-800 text-center">
                    <div className="text-xs text-neutral-400">测速评分缓存</div>
                    <div className="text-sm font-semibold font-mono text-neutral-100 mt-1">5 分钟</div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </Drawer>
    </div>
  );
};
