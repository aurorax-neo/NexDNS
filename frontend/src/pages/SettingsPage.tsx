import React, { useEffect, useState } from 'react';
import {
  Plus,
  Trash2,
  Database,
  Sliders,
  Lock,
  Globe,
  Zap,
  CheckCircle2,
  XCircle,
  RefreshCw,
  Shield,
} from 'lucide-react';
import { api, AppSettings } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Popconfirm } from '../components/ui/Popconfirm';
import { Snackbar } from '../components/ui/Snackbar';
import { Switch } from '../components/ui/Switch';
import { CustomSelect } from '../components/ui/CustomSelect';

export const SettingsPage: React.FC = () => {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [initialLoaded, setInitialLoaded] = useState(false);
  const [autoSaveStatus, setAutoSaveStatus] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  // Password change state
  const [oldPassword, setOldPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [passwordMsg, setPasswordMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [changingPass, setChangingPass] = useState(false);

  // Proxy list and testing state
  const [proxyTestResults, setProxyTestResults] = useState<Record<string, { ok: boolean; latency_ms?: number; error?: string }>>({});
  const [testingProxyId, setTestingProxyId] = useState<string | null>(null);
  const [isAddProxyModalOpen, setIsAddProxyModalOpen] = useState(false);
  const [newProxyName, setNewProxyName] = useState('');
  const [newProxyAddr, setNewProxyAddr] = useState('');

  const loadData = async () => {
    try {
      const settingsRes = await api.getSettings();
      setSettings(settingsRes);
      setTimeout(() => setInitialLoaded(true), 200);
    } catch (e) {
      console.error('Failed to load settings data', e);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  // 修改自动保存机制 (带防抖与视觉反馈)
  useEffect(() => {
    if (!initialLoaded || !settings) return;
    setAutoSaveStatus('saving');
    const timer = setTimeout(async () => {
      try {
        await api.updateSettings(settings);
        setAutoSaveStatus('saved');
      } catch (e) {
        setAutoSaveStatus('idle');
        console.error('Auto save failed', e);
      }
    }, 450);
    return () => clearTimeout(timer);
  }, [settings, initialLoaded]);

  const handleTestSingleProxy = async (id: string, addr: string) => {
    if (!addr) return;
    setTestingProxyId(id);
    try {
      const res = await api.testProxy(addr);
      setProxyTestResults((prev) => ({ ...prev, [id]: res }));
    } catch (e) {
      setProxyTestResults((prev) => ({
        ...prev,
        [id]: { ok: false, error: '网络测试请求失败' },
      }));
    } finally {
      setTestingProxyId(null);
    }
  };

  const handleSetDefaultProxy = (id: string) => {
    if (!settings) return;
    const currentList = settings.proxy.list || [];
    const updatedList = currentList.map((p) => ({
      ...p,
      is_default: p.id === id,
    }));
    const defaultItem = updatedList.find((p) => p.is_default);
    setSettings({
      ...settings,
      proxy: {
        ...settings.proxy,
        socks5: defaultItem ? defaultItem.addr : settings.proxy.socks5,
        list: updatedList,
      },
    });
    setSnackbarMessage('已切换默认代理节点');
  };

  const handleDeleteProxy = (id: string) => {
    if (!settings) return;
    const currentList = settings.proxy.list || [];
    const target = currentList.find((p) => p.id === id);
    const updatedList = currentList.filter((p) => p.id !== id);
    if (target?.is_default && updatedList.length > 0) {
      updatedList[0].is_default = true;
    }
    setSettings({
      ...settings,
      proxy: {
        ...settings.proxy,
        list: updatedList,
      },
    });
    setSnackbarMessage('已移除代理节点');
  };

  const handleAddProxy = (e: React.FormEvent) => {
    e.preventDefault();
    if (!settings || !newProxyName || !newProxyAddr) return;
    const currentList = settings.proxy.list || [];
    const newId = `proxy_${Date.now()}`;
    const willBeDefault = currentList.length === 0;

    const updatedList = currentList.map((p) => ({
      ...p,
      is_default: willBeDefault ? false : p.is_default,
    }));

    updatedList.push({
      id: newId,
      name: newProxyName,
      addr: newProxyAddr,
      is_default: willBeDefault,
    });

    setSettings({
      ...settings,
      proxy: {
        ...settings.proxy,
        socks5: willBeDefault ? newProxyAddr : settings.proxy.socks5,
        list: updatedList,
      },
    });

    setIsAddProxyModalOpen(false);
    setNewProxyName('');
    setNewProxyAddr('');
    setSnackbarMessage('已添加新代理节点');
  };

  const handleChangePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newPassword !== confirmPassword) {
      setPasswordMsg({ text: '两次输入的新密码不一致', isError: true });
      return;
    }
    setChangingPass(true);
    setPasswordMsg(null);
    try {
      await api.changePassword(oldPassword, newPassword);
      setPasswordMsg({ text: '密码修改成功，请牢记新密码', isError: false });
      setOldPassword('');
      setNewPassword('');
      setConfirmPassword('');
    } catch (err: any) {
      setPasswordMsg({ text: err.message || '修改密码失败', isError: true });
    } finally {
      setChangingPass(false);
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部控制栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <Sliders className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            系统核心参数设置
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            可视化配置常规 DNS 与 DoH 监听、缓存时长策略、下游结果过滤、代理节点及管理员凭据。修改自动保存并热生效。
          </p>
        </div>

        {/* 自动保存状态指示器 */}
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-neutral-100 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 text-xs">
            {autoSaveStatus === 'saving' ? (
              <>
                <RefreshCw className="w-3.5 h-3.5 text-cyan-500 animate-spin align-icon" />
                <span className="text-cyan-600 dark:text-cyan-400 font-medium">正在自动保存...</span>
              </>
            ) : autoSaveStatus === 'saved' ? (
              <>
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 align-icon" />
                <span className="text-neutral-600 dark:text-neutral-300 font-medium">配置已自动保存热生效</span>
              </>
            ) : (
              <>
                <CheckCircle2 className="w-3.5 h-3.5 text-neutral-400 align-icon" />
                <span className="text-neutral-500">修改自动保存就绪</span>
              </>
            )}
          </div>
        </div>
      </div>

      {settings && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* 1. DNS 基础与协议栈设置 */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Sliders className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">DNS 基础监听与下游服务</h4>
            </div>
            <div className="space-y-3 text-xs">
              {/* 常规 DNS 53 服务开关 */}
              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    常规 DNS 监听服务 (UDP/TCP 53)
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    接收来自局域网设备与操作系统的标准端口 53 查询
                  </span>
                </div>
                <Switch
                  checked={settings.dns.enabled ?? true}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      dns: { ...settings.dns, enabled: v },
                    })
                  }
                />
              </div>

              {/* 下游 DoH 服务开关 */}
              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    下游 HTTP DoH 服务 (/dns-query)
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    支持下游客户端通过 HTTP 访问 /dns-query 发起加密解析
                  </span>
                </div>
                <Switch
                  checked={settings.dns.doh_enabled ?? true}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      dns: { ...settings.dns, doh_enabled: v },
                    })
                  }
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">DNS 监听端口</label>
                  <input
                    type="number"
                    value={settings.dns.port}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        dns: { ...settings.dns, port: parseInt(e.target.value) || 53 },
                      })
                    }
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                  />
                </div>
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">Web 控制台端口</label>
                  <input
                    type="number"
                    value={settings.web.port}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        web: { ...settings.web, port: parseInt(e.target.value) || 3000 },
                      })
                    }
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">监听模式</label>
                  <CustomSelect
                    options={[
                      { value: 'dual', label: 'IPv4 + IPv6' },
                      { value: 'ipv4', label: 'IPv4' },
                      { value: 'ipv6', label: 'IPv6' },
                    ]}
                    value={settings.dns.listen_mode || settings.dns.connect_ip_version || 'dual'}
                    onChange={(v) =>
                      setSettings({
                        ...settings,
                        dns: { ...settings.dns, listen_mode: v, connect_ip_version: v },
                      })
                    }
                    className="w-full"
                  />
                </div>
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">返回下游解析结果</label>
                  <CustomSelect
                    options={[
                      { value: 'dual', label: 'IPv4 + IPv6' },
                      { value: 'ipv4_only', label: '仅 IPv4' },
                      { value: 'ipv6_only', label: '仅 IPv6' },
                    ]}
                    value={settings.dns.response_mode || 'dual'}
                    onChange={(v) =>
                      setSettings({
                        ...settings,
                        dns: { ...settings.dns, response_mode: v },
                      })
                    }
                    className="w-full"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">阻断响应策略 (Blocking Mode)</label>
                  <CustomSelect
                    options={[
                      { value: 'zero_ip', label: 'ZeroIP (0.0.0.0 / ::)' },
                      { value: 'nxdomain', label: 'NXDOMAIN (域名不存在)' },
                      { value: 'refused', label: 'REFUSED (拒绝查询)' },
                    ]}
                    value={settings.dns.block_type}
                    onChange={(v) =>
                      setSettings({
                        ...settings,
                        dns: { ...settings.dns, block_type: v },
                      })
                    }
                    className="w-full"
                  />
                </div>
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">阻断响应 TTL (秒)</label>
                  <input
                    type="number"
                    value={settings.dns.block_ttl}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        dns: { ...settings.dns, block_ttl: parseInt(e.target.value) || 60 },
                      })
                    }
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                  />
                </div>
              </div>
            </div>
          </div>

          {/* 2. 智能缓存设置 (SmartCache) */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Database className="w-4 h-4 text-amber-600 dark:text-amber-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">智能内存缓存 (SmartCache)</h4>
            </div>
            <div className="space-y-3 text-xs">
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">最大缓存条数</label>
                  <input
                    type="number"
                    value={settings.cache.size}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        cache: { ...settings.cache, size: parseInt(e.target.value) || 10000 },
                      })
                    }
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                  />
                </div>
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">最小 TTL (秒)</label>
                  <input
                    type="number"
                    value={settings.cache.min_ttl || 60}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        cache: { ...settings.cache, min_ttl: parseInt(e.target.value) || 60 },
                      })
                    }
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                  />
                </div>
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">最大 TTL (秒)</label>
                  <input
                    type="number"
                    value={settings.cache.max_ttl || 3600}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        cache: { ...settings.cache, max_ttl: parseInt(e.target.value) || 3600 },
                      })
                    }
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                  />
                </div>
              </div>

              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    Serve-Expired (过期缓存秒返)
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    过期后先以 3s TTL 立即应答，后台静默向真实上游更新
                  </span>
                </div>
                <Switch
                  checked={settings.cache.serve_expired}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      cache: { ...settings.cache, serve_expired: v },
                    })
                  }
                />
              </div>

              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    Prefetch-Domain (高频域名自动预取)
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    在 TTL 剩余低于 20% 时由内核自动发起静默预取
                  </span>
                </div>
                <Switch
                  checked={settings.cache.prefetch}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      cache: { ...settings.cache, prefetch: v },
                    })
                  }
                />
              </div>
            </div>
          </div>

          {/* 3. 分流与安全防御设置 (Routing Pipeline) */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Shield className="w-4 h-4 text-rose-600 dark:text-rose-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">分流与安全防御</h4>
            </div>
            <div className="space-y-3 text-xs">
              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    Qtype 65 (HTTPS/SVCB) 快捷阻断
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    阻止苹果及 Chrome 绕过系统 DNS 分流策略
                  </span>
                </div>
                <Switch
                  checked={settings.routing.qtype_65_reject}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      routing: { ...settings.routing, qtype_65_reject: v },
                    })
                  }
                />
              </div>

              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    严格防 DNS 污染检测
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    大陆组若返回非中国 IP 则直接丢弃并触发 Fallback 回退
                  </span>
                </div>
                <Switch
                  checked={settings.anti_pollution}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      anti_pollution: v,
                      routing: { ...settings.routing, anti_pollution: v },
                    })
                  }
                />
              </div>

              <div>
                <label className="block text-neutral-600 dark:text-neutral-400 mb-1">Fallback 回退切换超时 (毫秒)</label>
                <input
                  type="number"
                  value={settings.routing.fallback_timeout_ms}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      routing: {
                        ...settings.routing,
                        fallback_timeout_ms: parseInt(e.target.value) || 1500,
                      },
                    })
                  }
                  className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono"
                />
              </div>
            </div>
          </div>

          {/* 4. SOCKS5 代理设置 (Proxy) */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Globe className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">代理服务与上层默认代理设置</h4>
            </div>
            <div className="space-y-3 text-xs">
              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    启用代理转发功能
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    启用后，上游服务器可自由选择直连、默认代理或指定专属代理节点转发请求
                  </span>
                </div>
                <Switch
                  checked={settings.proxy.enabled}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      proxy: { ...settings.proxy, enabled: v },
                    })
                  }
                />
              </div>

              {/* 上层直接选择全局默认代理 */}
              <div>
                <label className="block text-neutral-600 dark:text-neutral-400 mb-1">全局默认代理节点</label>
                <CustomSelect
                  options={(settings.proxy.list || []).map((p) => ({
                    value: p.id,
                    label: `${p.name} (${p.addr})`,
                  }))}
                  value={
                    (settings.proxy.list || []).find((p) => p.is_default)?.id ||
                    (settings.proxy.list || [])[0]?.id ||
                    ''
                  }
                  onChange={(id) => handleSetDefaultProxy(id)}
                  className="w-full"
                />
              </div>

              {/* 代理节点列表 */}
              <div className="space-y-2 pt-1">
                <div className="flex items-center justify-between">
                  <span className="font-medium text-neutral-700 dark:text-neutral-300">已配置代理节点</span>
                  <button
                    type="button"
                    onClick={() => setIsAddProxyModalOpen(true)}
                    className="inline-flex items-center gap-1.5 min-h-[32px] px-3 text-xs bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 rounded-md text-neutral-800 dark:text-neutral-200 transition-colors"
                  >
                    <Plus className="w-3.5 h-3.5 align-icon" />
                    添加代理节点
                  </button>
                </div>

                <div className="space-y-2">
                  {(settings.proxy.list || []).map((p) => {
                    const testRes = proxyTestResults[p.id];
                    const isTesting = testingProxyId === p.id;

                    return (
                      <div
                        key={p.id}
                        className="p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex flex-wrap items-center justify-between gap-3"
                      >
                        <div className="flex items-center gap-2.5 min-w-0 flex-1">
                          {p.is_default && (
                            <span className="px-2 py-0.5 text-[11px] font-medium rounded bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-400 border border-cyan-200 dark:border-cyan-800/40 shrink-0">
                              默认代理
                            </span>
                          )}
                          <div className="min-w-0">
                            <div className="font-medium text-neutral-900 dark:text-neutral-100 truncate">
                              {p.name}
                            </div>
                            <div className="font-mono text-[11px] text-neutral-500 truncate">
                              {p.addr}
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-2 shrink-0">
                          {testRes && (
                            <div className="flex items-center gap-1.5 font-mono text-[11px] tabular-nums">
                              {testRes.ok ? (
                                <span className="text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                                  <CheckCircle2 className="w-3.5 h-3.5 align-icon" />
                                  {testRes.latency_ms?.toFixed(1)} ms
                                </span>
                              ) : (
                                <span className="text-rose-600 dark:text-rose-400 flex items-center gap-1" title={testRes.error}>
                                  <XCircle className="w-3.5 h-3.5 align-icon" />
                                  连接失败
                                </span>
                              )}
                            </div>
                          )}

                          <button
                            type="button"
                            onClick={() => handleTestSingleProxy(p.id, p.addr)}
                            disabled={isTesting}
                            className="min-h-[32px] px-2.5 text-xs bg-white dark:bg-surface-3 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 rounded text-neutral-800 dark:text-neutral-200 disabled:opacity-50 transition-colors"
                          >
                            {isTesting ? '探测中...' : '测试连通性'}
                          </button>

                          {(settings.proxy.list || []).length > 1 && (
                            <Popconfirm
                              title="确认删除该代理节点？"
                              actionText="确认删除"
                              onConfirm={() => handleDeleteProxy(p.id)}
                            >
                              <button
                                type="button"
                                className="min-w-[32px] min-h-[32px] flex items-center justify-center text-neutral-400 hover:text-red-500"
                                title="删除节点"
                              >
                                <Trash2 className="w-3.5 h-3.5 align-icon" />
                              </button>
                            </Popconfirm>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          </div>

          {/* 5. 日志与留存设置 */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Zap className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">查询日志与留存策略</h4>
            </div>
            <div className="space-y-3 text-xs">
              <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
                <div>
                  <span className="font-medium text-neutral-800 dark:text-neutral-200 block">
                    记录 DNS 查询日志
                  </span>
                  <span className="text-[11px] text-neutral-500">
                    在 SQLite WAL 数据库中异步留存客户端查询流水
                  </span>
                </div>
                <Switch
                  checked={settings.logging.enabled}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      logging: { ...settings.logging, enabled: v },
                    })
                  }
                />
              </div>

              <div>
                <label className="block text-neutral-600 dark:text-neutral-400 mb-1">日志保留天数</label>
                <CustomSelect
                  options={[
                    { value: '1', label: '保留 24 小时' },
                    { value: '7', label: '保留 7 天 (推荐)' },
                    { value: '30', label: '保留 30 天' },
                    { value: '90', label: '保留 90 天' },
                  ]}
                  value={settings.logging.retention_days.toString()}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      logging: { ...settings.logging, retention_days: parseInt(v) || 7 },
                    })
                  }
                  className="w-full"
                />
              </div>
            </div>
          </div>

          {/* 6. 管理员账户安全 */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Lock className="w-4 h-4 text-indigo-600 dark:text-indigo-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">修改管理员密码</h4>
            </div>
            <form onSubmit={handleChangePassword} className="space-y-3 text-xs">
              <div>
                <label className="block text-neutral-600 dark:text-neutral-400 mb-1">当前原密码</label>
                <input
                  type="password"
                  value={oldPassword}
                  onChange={(e) => setOldPassword(e.target.value)}
                  required
                  placeholder="请输入当前密码"
                  className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">新密码</label>
                  <input
                    type="password"
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    required
                    placeholder="至少 6 位"
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100"
                  />
                </div>
                <div>
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">确认新密码</label>
                  <input
                    type="password"
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                    required
                    placeholder="再次输入"
                    className="w-full min-h-[38px] px-3 bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100"
                  />
                </div>
              </div>

              {passwordMsg && (
                <div
                  className={`p-2.5 rounded-md text-xs ${
                    passwordMsg.isError
                      ? 'bg-red-50 dark:bg-red-950/40 text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800'
                      : 'bg-emerald-50 dark:bg-emerald-950/40 text-emerald-600 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800'
                  }`}
                >
                  {passwordMsg.text}
                </div>
              )}

              <button
                type="submit"
                disabled={changingPass}
                className="min-h-[38px] px-4 bg-indigo-600 hover:bg-indigo-500 text-white font-medium rounded-md shadow-xs disabled:opacity-50"
              >
                {changingPass ? '修改中...' : '确认更新密码'}
              </button>
            </form>
          </div>
        </div>
      )}


      {/* 添加代理节点 Modal */}
      <Modal
        isOpen={isAddProxyModalOpen}
        onClose={() => setIsAddProxyModalOpen(false)}
        title="添加代理节点"
        size="sm"
      >
        <form onSubmit={handleAddProxy} className="space-y-4 text-xs">
          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              节点备注名称
            </label>
            <input
              type="text"
              value={newProxyName}
              onChange={(e) => setNewProxyName(e.target.value)}
              placeholder="例如: 本地 Clash 代理 或 香港高速专线"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              代理连接地址 (支持 SOCKS5 / HTTP)
            </label>
            <input
              type="text"
              value={newProxyAddr}
              onChange={(e) => setNewProxyAddr(e.target.value)}
              placeholder="例如: socks5://10.96.1.108:7890 或 127.0.0.1:7890"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div className="flex justify-end gap-2 pt-3 border-t border-neutral-200 dark:border-neutral-800">
            <button
              type="button"
              onClick={() => setIsAddProxyModalOpen(false)}
              className="min-h-[36px] px-4 text-neutral-600 dark:text-neutral-400"
            >
              取消
            </button>
            <button
              type="submit"
              className="min-h-[36px] px-5 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-md shadow-xs"
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
