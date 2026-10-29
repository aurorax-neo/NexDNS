import React, { useEffect, useState } from 'react';
import {
  Settings,
  Plus,
  Trash2,
  RotateCcw,
  Save,
  Shield,
  Database,
  Sliders,
  Lock,
  Globe,
  Zap,
  CheckCircle2,
  XCircle,
} from 'lucide-react';
import { api, AppSettings, DnsRewriteRecord } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Popconfirm } from '../components/ui/Popconfirm';
import { Snackbar } from '../components/ui/Snackbar';
import { Switch } from '../components/ui/Switch';
import { CustomSelect } from '../components/ui/CustomSelect';

export const SettingsPage: React.FC = () => {
  const [rewrites, setRewrites] = useState<DnsRewriteRecord[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [savingSettings, setSavingSettings] = useState(false);

  // Modal & Snackbar
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [domain, setDomain] = useState('');
  const [recordType, setRecordType] = useState('A');
  const [value, setValue] = useState('');
  const [flushSuccess, setFlushSuccess] = useState(false);
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
  const [newProxyIsDefault, setNewProxyIsDefault] = useState(false);

  const loadData = async () => {
    try {
      const [rewritesRes, settingsRes] = await Promise.all([
        api.getRewrites(),
        api.getSettings(),
      ]);
      setRewrites(rewritesRes);
      setSettings(settingsRes);
    } catch (e) {
      console.error('Failed to load settings data', e);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleSaveSettings = async () => {
    if (!settings) return;
    setSavingSettings(true);
    try {
      await api.updateSettings(settings);
      setSnackbarMessage('系统配置已保存并在内核中热生效！');
    } catch (e: any) {
      alert('保存失败: ' + e.message);
    } finally {
      setSavingSettings(false);
    }
  };

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
    const willBeDefault = newProxyIsDefault || currentList.length === 0;

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
    setNewProxyIsDefault(false);
    setSnackbarMessage('已添加新代理节点，请点击右上角“保存系统配置”使配置持久化');
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

  const handleAddRewrite = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!domain || !value) return;
    try {
      await api.addRewrite({
        domain,
        record_type: recordType,
        value,
      });
      setIsAddModalOpen(false);
      setDomain('');
      setValue('');
      const updated = await api.getRewrites();
      setRewrites(updated);
      setSnackbarMessage('已添加重写记录');
    } catch (e) {
      console.error('Failed to add rewrite', e);
    }
  };

  const handleDeleteRewrite = async (id: string) => {
    try {
      await api.deleteRewrite(id);
      const updated = await api.getRewrites();
      setRewrites(updated);
      setSnackbarMessage('已删除重写记录');
    } catch (e) {
      console.error('Failed to delete rewrite', e);
    }
  };

  const handleFlushCache = async () => {
    try {
      await api.flushCache();
      setFlushSuccess(true);
      setSnackbarMessage('DNS 内存缓存已清空');
      setTimeout(() => setFlushSuccess(false), 2500);
    } catch (e) {
      console.error('Failed to flush cache', e);
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部控制栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <Settings className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            系统核心参数精细化控制
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            可视化配置 DNS 协议、拦截响应模式、内存缓存策略、分流安全防御及 SOCKS5 代理。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleFlushCache}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-amber-700 dark:text-amber-300 hover:bg-amber-100 dark:hover:bg-amber-950/60 bg-amber-50 dark:bg-amber-950/40 border border-amber-200 dark:border-amber-800/60 rounded-md transition-colors shadow-xs"
          >
            <RotateCcw className="w-3.5 h-3.5 align-icon" />
            {flushSuccess ? '已清空缓存' : '清空内存缓存'}
          </button>
          <button
            type="button"
            onClick={handleSaveSettings}
            disabled={savingSettings || !settings}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-xs disabled:opacity-50"
          >
            <Save className="w-3.5 h-3.5 align-icon" />
            {savingSettings ? '保存中...' : '保存全部系统配置'}
          </button>
        </div>
      </div>

      {settings && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* 1. DNS 基础与协议栈设置 */}
          <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs space-y-4">
            <div className="flex items-center gap-2 pb-3 border-b border-neutral-200 dark:border-neutral-800">
              <Sliders className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">DNS 基础与协议栈</h4>
            </div>
            <div className="space-y-3 text-xs">
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

              <div>
                <label className="block text-neutral-600 dark:text-neutral-400 mb-1">IP 协议栈模式</label>
                <CustomSelect
                  options={[
                    { value: 'dual', label: '双栈模式 (IPv4 + IPv6)' },
                    { value: 'v4', label: '仅 IPv4 (停用 IPv6 优选)' },
                    { value: 'v6', label: '仅 IPv6' },
                  ]}
                  value={settings.dns.connect_ip_version}
                  onChange={(v) =>
                    setSettings({
                      ...settings,
                      dns: { ...settings.dns, connect_ip_version: v },
                    })
                  }
                  className="w-full"
                />
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
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">智能内存缓存与预取</h4>
            </div>
            <div className="space-y-3 text-xs">
              <div className="grid grid-cols-2 gap-3">
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
                  <label className="block text-neutral-600 dark:text-neutral-400 mb-1">最小 TTL (Min TTL)</label>
                  <input
                    type="number"
                    value={settings.cache.min_ttl}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        cache: { ...settings.cache, min_ttl: parseInt(e.target.value) || 60 },
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
              <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">代理服务与节点列表 (Proxy Nodes)</h4>
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
                          {p.is_default ? (
                            <span className="px-2 py-0.5 text-[11px] font-medium rounded bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-400 border border-cyan-200 dark:border-cyan-800/40 shrink-0">
                              默认代理
                            </span>
                          ) : (
                            <button
                              type="button"
                              onClick={() => handleSetDefaultProxy(p.id)}
                              className="px-2 py-0.5 text-[11px] text-neutral-500 hover:text-neutral-800 dark:hover:text-neutral-200 rounded border border-dashed border-neutral-300 dark:border-neutral-700 shrink-0"
                            >
                              设为默认
                            </button>
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
                  placeholder="默认: admin1234"
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

      {/* 7. DNS 重写记录列表 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden mt-6">
        <div className="px-5 py-4 border-b border-neutral-200 dark:border-neutral-800 flex items-center justify-between">
          <div>
            <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">自定义 DNS 重写 (Hosts 映射)</h4>
            <p className="text-xs text-neutral-500 mt-0.5">优先应答指定的本地 A/AAAA/CNAME 映射记录</p>
          </div>
          <button
            type="button"
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[36px] px-3.5 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md shadow-xs"
          >
            <Plus className="w-3.5 h-3.5 align-icon" />
            添加重写记录
          </button>
        </div>
        <div className="divide-y divide-neutral-200 dark:divide-neutral-800/60">
          {rewrites.length > 0 ? (
            rewrites.map((r) => (
              <div
                key={r.id}
                className="p-4 flex items-center justify-between hover:bg-neutral-50 dark:hover:bg-surface-2/40 transition-colors"
              >
                <div className="flex items-center gap-4">
                  <span className="px-2 py-0.5 text-xs font-mono font-bold rounded bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-400 border border-cyan-200 dark:border-cyan-800/40">
                    {r.record_type}
                  </span>
                  <div>
                    <div className="text-sm font-mono font-medium text-neutral-900 dark:text-neutral-100">{r.domain}</div>
                    <div className="text-xs font-mono text-neutral-500 dark:text-neutral-400 mt-0.5">&rarr; {r.value}</div>
                  </div>
                </div>

                <div>
                  <Popconfirm
                    title="确认删除该 DNS 重写规则？"
                    actionText="确认删除"
                    onConfirm={() => handleDeleteRewrite(r.id)}
                  >
                    <button
                      type="button"
                      className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-500 dark:text-neutral-400 hover:text-red-500 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2"
                      title="删除记录"
                    >
                      <Trash2 className="w-4 h-4 align-icon" />
                    </button>
                  </Popconfirm>
                </div>
              </div>
            ))
          ) : (
            <p className="text-xs text-neutral-400 py-10 text-center">暂未配置自定义重写规则</p>
          )}
        </div>
      </div>

      {/* 添加重写 Modal (400px - 小档) */}
      <Modal
        isOpen={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        title="添加自定义 DNS 重写"
        size="sm"
      >
        <form onSubmit={handleAddRewrite} className="space-y-4 text-xs">
          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">域名</label>
            <input
              type="text"
              value={domain}
              onChange={(e) => setDomain(e.target.value)}
              placeholder="例如: nas.home 或 gitlab.local"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">记录类型</label>
            <CustomSelect
              options={[
                { value: 'A', label: 'A 记录 (IPv4)' },
                { value: 'AAAA', label: 'AAAA 记录 (IPv6)' },
                { value: 'CNAME', label: 'CNAME 别名' },
              ]}
              value={recordType}
              onChange={setRecordType}
              className="w-full"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">目标 IP 或 目标域名</label>
            <input
              type="text"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder="例如: 192.168.1.100 或 example.com"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div className="flex justify-end gap-2 pt-3 border-t border-neutral-200 dark:border-neutral-800">
            <button
              type="button"
              onClick={() => setIsAddModalOpen(false)}
              className="min-h-[38px] px-4 text-neutral-600 dark:text-neutral-400"
            >
              取消
            </button>
            <button
              type="submit"
              className="min-h-[38px] px-5 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-md shadow-xs"
            >
              保存重写
            </button>
          </div>
        </form>
      </Modal>

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

          <div className="flex items-center justify-between p-3 rounded-md bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800">
            <div>
              <span className="font-medium text-neutral-800 dark:text-neutral-200 block">设为默认代理</span>
              <span className="text-[11px] text-neutral-500">未指定特定代理的上游节点将默认使用此节点</span>
            </div>
            <Switch
              checked={newProxyIsDefault}
              onChange={setNewProxyIsDefault}
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
