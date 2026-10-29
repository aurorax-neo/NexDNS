import React, { useEffect, useState } from 'react';
import {
  Server,
  Zap,
  CheckCircle2,
  XCircle,
  RefreshCw,
  Globe,
  Shield,
  Plus,
  Trash2,
  Play,
  Edit3,
  GripVertical,
} from 'lucide-react';
import { api, UpstreamTestItem, UpstreamsResponse, ProxyItem } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Popconfirm } from '../components/ui/Popconfirm';
import { Snackbar } from '../components/ui/Snackbar';
import { CustomSelect } from '../components/ui/CustomSelect';

export const UpstreamsPage: React.FC = () => {
  const [upstreams, setUpstreams] = useState<UpstreamsResponse | null>(null);
  const [proxyList, setProxyList] = useState<ProxyItem[]>([]);
  const [testResults, setTestResults] = useState<UpstreamTestItem[]>([]);
  const [testing, setTesting] = useState(false);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [targetGroup, setTargetGroup] = useState<string>('mainland');
  const [newAddr, setNewAddr] = useState('');
  const [newHostName, setNewHostName] = useState('');
  const [newProxyMode, setNewProxyMode] = useState('default');

  // Single test state
  const [testingSingleAddr, setTestingSingleAddr] = useState<string | null>(null);

  // Edit upstream state
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingOldGroup, setEditingOldGroup] = useState<string>('mainland');
  const [editingOldAddr, setEditingOldAddr] = useState<string>('');
  const [editTargetGroup, setEditTargetGroup] = useState<string>('mainland');
  const [editAddr, setEditAddr] = useState<string>('');
  const [editHostName, setEditHostName] = useState<string>('');
  const [editProxyMode, setEditProxyMode] = useState<string>('default');
  const [savingEdit, setSavingEdit] = useState(false);

  // Drag and drop state
  const [draggedGroup, setDraggedGroup] = useState<string | null>(null);
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);

  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  useEffect(() => {
    api.getUpstreams().then(setUpstreams).catch(console.error);
    api.getSettings().then((s) => {
      if (s.proxy?.list) {
        setProxyList(s.proxy.list);
      }
    }).catch(console.error);
  }, []);

  const handleBenchmark = async () => {
    setTesting(true);
    try {
      const res = await api.testUpstreams();
      setTestResults(res);
    } catch (e) {
      console.error('Failed to test upstreams', e);
    } finally {
      setTesting(false);
    }
  };

  const handleTestSingle = async (group: string, addr: string) => {
    setTestingSingleAddr(addr);
    try {
      const res = await api.testSingleUpstream(group, addr);
      setTestResults((prev) => {
        const existing = prev.filter((r) => r.addr !== addr);
        return [...existing, { addr, group, latency_ms: res.latency_ms, ok: res.ok }];
      });
      if (res.ok) {
        setSnackbarMessage(`单节点测速成功！时延: ${res.latency_ms?.toFixed(1)} ms`);
      } else {
        setSnackbarMessage(`节点连接失败: ${res.error || '不可达'}`);
      }
    } catch (e: any) {
      setSnackbarMessage(`测试请求失败: ${e.message}`);
    } finally {
      setTestingSingleAddr(null);
    }
  };

  const handleOpenEditModal = (group: string, rawAddr: string) => {
    const { uri, hostName, proxy } = parseUpstreamEntry(rawAddr);
    setEditingOldGroup(group);
    setEditingOldAddr(rawAddr);
    setEditTargetGroup(group);
    setEditAddr(uri);
    setEditHostName(hostName || '');
    setEditProxyMode(proxy || 'default');
    setIsEditModalOpen(true);
  };

  const handleEditSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editAddr.trim()) return;
    setSavingEdit(true);
    try {
      let finalAddr = editAddr.trim();
      const params: string[] = [];
      if (editHostName.trim()) {
        params.push(`sni=${editHostName.trim()}`);
      }
      if (editProxyMode !== 'default') {
        if (editProxyMode !== 'none' || editTargetGroup !== 'mainland') {
          params.push(`proxy=${editProxyMode}`);
        }
      } else if (editTargetGroup !== 'mainland') {
        params.push('proxy=default');
      }
      if (params.length > 0) {
        finalAddr += `#${params.join('&')}`;
      }

      await api.editUpstream({
        old_group: editingOldGroup,
        old_addr: editingOldAddr,
        new_group: editTargetGroup,
        new_addr: finalAddr,
      });
      setIsEditModalOpen(false);
      const updated = await api.getUpstreams();
      setUpstreams(updated);
      setSnackbarMessage('上游节点信息已成功修改并热重载生效！');
    } catch (e: any) {
      console.error('Failed to edit upstream', e);
    } finally {
      setSavingEdit(false);
    }
  };

  const handleDragStart = (e: React.DragEvent, groupKey: string, index: number) => {
    setDraggedGroup(groupKey);
    setDraggedIndex(index);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', `${groupKey}:${index}`);
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  };

  const handleDrop = async (groupKey: 'mainland' | 'gfw' | 'overseas', targetIndex: number) => {
    if (draggedGroup !== groupKey || draggedIndex === null || draggedIndex === targetIndex || !upstreams) {
      setDraggedGroup(null);
      setDraggedIndex(null);
      return;
    }

    const currentList = [...upstreams[groupKey]];
    const [moved] = currentList.splice(draggedIndex, 1);
    currentList.splice(targetIndex, 0, moved);

    const updatedUpstreams = {
      ...upstreams,
      [groupKey]: currentList,
    };
    setUpstreams(updatedUpstreams);
    setDraggedGroup(null);
    setDraggedIndex(null);

    try {
      await api.reorderUpstreams(groupKey, currentList);
      setSnackbarMessage(`已更新 ${groupKey} 组节点优先级排序！`);
    } catch (e) {
      console.error('Failed to reorder upstreams', e);
    }
  };

  const handleAddUpstream = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newAddr.trim()) return;
    try {
      let finalAddr = newAddr.trim();
      const params: string[] = [];
      if (newHostName.trim()) {
        params.push(`sni=${newHostName.trim()}`);
      }
      if (newProxyMode !== 'default') {
        if (newProxyMode !== 'none' || targetGroup !== 'mainland') {
          params.push(`proxy=${newProxyMode}`);
        }
      } else if (targetGroup !== 'mainland') {
        params.push('proxy=default');
      }
      if (params.length > 0) {
        finalAddr += `#${params.join('&')}`;
      }
      await api.addUpstream(targetGroup, finalAddr);
      setIsAddModalOpen(false);
      setNewAddr('');
      setNewHostName('');
      setNewProxyMode('default');
      const updated = await api.getUpstreams();
      setUpstreams(updated);
      setSnackbarMessage('已添加上游服务器');
    } catch (e) {
      console.error('Failed to add upstream', e);
    }
  };

  const handleRemoveUpstream = async (group: string, addr: string) => {
    try {
      await api.removeUpstream(group, addr);
      const updated = await api.getUpstreams();
      setUpstreams(updated);
      setSnackbarMessage('已删除上游节点');
    } catch (e) {
      console.error('Failed to remove upstream', e);
    }
  };

  const getLatencyScore = (addr: string) => {
    const match = testResults.find((r) => r.addr === addr);
    if (!match) return null;
    return match;
  };

  const parseUpstreamEntry = (raw: string) => {
    let cleanStr = raw.trim();
    let hostName: string | null = null;
    let proxy: string | null = null;

    // Support NexDNS native URI fragment metadata: #sni=...&proxy=...
    if (cleanStr.includes('#')) {
      const [base, frag] = cleanStr.split('#', 2);
      cleanStr = base;
      if (frag.includes('=')) {
        for (const pair of frag.split('&')) {
          const [k, v] = pair.split('=', 2);
          if (k === 'sni' || k === 'host') hostName = v.trim();
          if (k === 'proxy') proxy = v.trim();
        }
      } else if (frag.trim()) {
        hostName = frag.trim();
      }
    }

    // Backward compatibility for CLI flags
    const parts = cleanStr.split(/\s+/);
    let uri = parts[0] || '';

    for (let i = 1; i < parts.length; i++) {
      if ((parts[i] === '-host-name' || parts[i] === '-sni') && i + 1 < parts.length) {
        if (!hostName) hostName = parts[i + 1];
        i++;
      } else if (parts[i] === '-proxy' && i + 1 < parts.length) {
        if (!proxy) proxy = parts[i + 1];
        i++;
      }
    }

    const isDoh = uri.startsWith('https://');
    const isTcp = uri.startsWith('tcp://');
    const proto = isDoh ? 'DoH' : isTcp ? 'TCP' : 'UDP';

    return { uri, hostName, proxy, proto };
  };

  const renderGroup = (title: string, groupKey: 'mainland' | 'gfw' | 'overseas', icon: React.ReactNode, desc: string) => {
    const servers = upstreams ? upstreams[groupKey] : [];
    return (
      <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
        <div className="flex items-center justify-between pb-3 border-b border-neutral-200 dark:border-neutral-800">
          <div className="flex items-center gap-2.5">
            {icon}
            <div>
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">{title}</h4>
              <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">{desc}</p>
            </div>
          </div>
          <span className="text-xs font-mono text-neutral-600 dark:text-neutral-400 bg-neutral-100 dark:bg-surface-2 px-2 py-0.5 rounded border border-neutral-200 dark:border-neutral-800">
            {servers.length} 个节点
          </span>
        </div>

        <div className="space-y-2.5">
          {servers.map((addr) => {
            const score = getLatencyScore(addr);
            const { uri, hostName, proxy, proto } = parseUpstreamEntry(addr);
            const isDoh = proto === 'DoH';
            const isTcp = proto === 'TCP';
            const effectiveProxy = proxy || (groupKey === 'mainland' ? 'none' : 'default');

            return (
              <div
                key={addr}
                className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800/80 hover:border-neutral-300 dark:hover:border-neutral-700 transition-colors"
              >
                <div className="flex items-center gap-2 min-w-0 pr-3 flex-wrap">
                  <span
                    className={`px-1.5 py-0.5 text-[10px] font-mono font-bold rounded ${
                      isDoh
                        ? 'bg-purple-50 dark:bg-purple-950/60 text-purple-700 dark:text-purple-400 border border-purple-200 dark:border-purple-800/40'
                        : isTcp
                        ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-700 dark:text-blue-400 border border-blue-200 dark:border-blue-800/40'
                        : 'bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800/40'
                    }`}
                  >
                    {proto}
                  </span>
                  <span className="font-mono text-xs text-neutral-800 dark:text-neutral-200 truncate">
                    {uri}
                  </span>
                  {hostName && (
                    <span className="px-1.5 py-0.5 text-[10px] font-mono rounded bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-400 border border-cyan-200 dark:border-cyan-800/40 shrink-0">
                      SNI: {hostName}
                    </span>
                  )}
                  {effectiveProxy === 'none' ? (
                    <span className="px-1.5 py-0.5 text-[10px] rounded bg-neutral-200/70 dark:bg-surface-3 text-neutral-600 dark:text-neutral-400 shrink-0">
                      直连
                    </span>
                  ) : effectiveProxy === 'default' ? (
                    <span className="px-1.5 py-0.5 text-[10px] rounded bg-blue-50 dark:bg-blue-950/60 text-blue-700 dark:text-blue-400 border border-blue-200 dark:border-blue-800/40 shrink-0">
                      默认代理
                    </span>
                  ) : (
                    <span className="px-1.5 py-0.5 text-[10px] rounded bg-purple-50 dark:bg-purple-950/60 text-purple-700 dark:text-purple-400 border border-purple-200 dark:border-purple-800/40 shrink-0">
                      代理: {proxyList.find((p) => p.id === effectiveProxy)?.name || effectiveProxy}
                    </span>
                  )}
                </div>

                <div className="flex items-center gap-3 shrink-0">
                  {score && (
                    <div className="flex items-center gap-1.5 font-mono text-xs tabular-nums">
                      {score.ok ? (
                        <>
                          <span
                            className={`font-semibold ${
                              score.latency_ms! < 10
                                ? 'text-emerald-600 dark:text-emerald-400'
                                : score.latency_ms! < 150
                                ? 'text-amber-600 dark:text-amber-400'
                                : 'text-neutral-400'
                            }`}
                          >
                            {score.latency_ms! < 0.1 ? '< 0.1 ms' : `${score.latency_ms!.toFixed(1)} ms`}
                          </span>
                          <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 align-icon" />
                        </>
                      ) : (
                        <>
                          <span className="text-rose-600 dark:text-rose-400">不可达</span>
                          <XCircle className="w-3.5 h-3.5 text-rose-500 align-icon" />
                        </>
                      )}
                    </div>
                  )}

                  <Popconfirm
                    title="确认移除该上游服务器？"
                    actionText="确认移除"
                    onConfirm={() => handleRemoveUpstream(groupKey, addr)}
                  >
                    <button
                      type="button"
                      className="text-neutral-400 hover:text-red-500 transition-colors"
                      title="删除节点"
                    >
                      <Trash2 className="w-3.5 h-3.5 align-icon" />
                    </button>
                  </Popconfirm>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 头部控制栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <Server className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            多协议上游集群与测速优选
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-2xl leading-relaxed">
            按分组调度解析。支持 UDP 53、TCP 53 及加密 DoH。内置非阻塞 TCP 80/443 建连测速，优先向客户端返回最快可用 IP。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-neutral-800 dark:text-neutral-200 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 rounded-md transition-colors"
          >
            <Plus className="w-3.5 h-3.5 align-icon" />
            添加上游节点
          </button>
          <button
            type="button"
            onClick={handleBenchmark}
            disabled={testing}
            className="inline-flex items-center gap-2 min-h-[38px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-xs disabled:opacity-50"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${testing ? 'animate-spin' : ''}`} />
            {testing ? '正在并发测试...' : '测试上游连通性与时延'}
          </button>
        </div>
      </div>

      {/* 三大上游组卡片 */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {renderGroup(
          '大陆直连组 (mainland)',
          'mainland',
          <Zap className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />,
          '直连解析，支持阿里 DNS、DNSPub 等节点'
        )}
        {renderGroup(
          'GFW / 境外组 (gfw)',
          'gfw',
          <Shield className="w-4 h-4 text-rose-500 dark:text-rose-400 align-icon" />,
          '抗污染回退组，走纯净加密 DoH 解析'
        )}
        {renderGroup(
          '海外备用组 (overseas)',
          'overseas',
          <Globe className="w-4 h-4 text-emerald-600 dark:text-emerald-400 align-icon" />,
          'Cloudflare 全球冗余应急解析'
        )}
      </div>

      {/* 添加上游 Modal */}
      <Modal
        isOpen={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        title="添加上游 DNS 节点"
        size="md"
      >
        <form onSubmit={handleAddUpstream} className="space-y-4 text-xs">
          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              所属分组
            </label>
            <CustomSelect
              options={[
                { value: 'mainland', label: '大陆直连组 (mainland)' },
                { value: 'gfw', label: 'GFW / 境外代理组 (gfw)' },
                { value: 'overseas', label: '海外备用组 (overseas)' },
              ]}
              value={targetGroup}
              onChange={setTargetGroup}
              className="w-full"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              服务器地址 (支持 udp://、tcp://、https://)
            </label>
            <input
              type="text"
              value={newAddr}
              onChange={(e) => setNewAddr(e.target.value)}
              placeholder="例如: https://101.226.4.6/dns-query 或 udp://223.5.5.5:53"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              自定义 SNI / 主机名 (可选，直连 DoH 节点)
            </label>
            <input
              type="text"
              value={newHostName}
              onChange={(e) => setNewHostName(e.target.value)}
              placeholder="例如: doh.360.cn (将自动组装为原生 URI: #sni=doh.360.cn)"
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              代理策略 (Proxy Route)
            </label>
            <CustomSelect
              options={[
                { value: 'none', label: '直连 (不使用代理)' },
                { value: 'default', label: '系统默认代理 (推荐)' },
                ...proxyList.map((p) => ({
                  value: p.id,
                  label: `${p.name} (${p.addr})`,
                })),
              ]}
              value={newProxyMode}
              onChange={setNewProxyMode}
              className="w-full"
            />
          </div>

          <div className="flex justify-end gap-2 pt-3 border-t border-neutral-200 dark:border-neutral-800">
            <button
              type="button"
              onClick={() => setIsAddModalOpen(false)}
              className="min-h-[36px] px-4 text-neutral-600 dark:text-neutral-400"
            >
              取消
            </button>
            <button
              type="submit"
              className="min-h-[36px] px-4 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-md shadow-xs"
            >
              确认添加
            </button>
          </div>
        </form>
      </Modal>

      {snackbarMessage && (
        <Snackbar message={snackbarMessage} onClose={() => setSnackbarMessage(null)} />
      )}
    </div>
  );
};
