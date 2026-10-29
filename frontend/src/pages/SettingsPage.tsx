import React, { useEffect, useState } from 'react';
import {
  Settings,
  Plus,
  Trash2,
  RotateCcw,
  FileCode,
  Save,
} from 'lucide-react';
import { api, DnsRewriteRecord } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Drawer } from '../components/ui/Drawer';
import { Popconfirm } from '../components/ui/Popconfirm';
import { Snackbar } from '../components/ui/Snackbar';

export const SettingsPage: React.FC = () => {
  const [rewrites, setRewrites] = useState<DnsRewriteRecord[]>([]);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [domain, setDomain] = useState('');
  const [recordType, setRecordType] = useState('A');
  const [value, setValue] = useState('');
  const [flushSuccess, setFlushSuccess] = useState(false);

  // Raw config editor
  const [isConfigDrawerOpen, setIsConfigDrawerOpen] = useState(false);
  const [rawConfig, setRawConfig] = useState('');
  const [savingConfig, setSavingConfig] = useState(false);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  const fetchRewrites = async () => {
    try {
      const res = await api.getRewrites();
      setRewrites(res);
    } catch (e) {
      console.error('Failed to load rewrites', e);
    }
  };

  useEffect(() => {
    fetchRewrites();
  }, []);

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
      await fetchRewrites();
      setSnackbarMessage('已添加重写记录');
    } catch (e) {
      console.error('Failed to add rewrite', e);
    }
  };

  const handleDeleteRewrite = async (id: string) => {
    try {
      await api.deleteRewrite(id);
      await fetchRewrites();
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

  const handleOpenConfigDrawer = async () => {
    try {
      const res = await api.getRawConfig();
      setRawConfig(res.content);
      setIsConfigDrawerOpen(true);
    } catch (e) {
      console.error('Failed to load raw config', e);
    }
  };

  const handleSaveConfig = async () => {
    setSavingConfig(true);
    try {
      await api.updateRawConfig(rawConfig);
      setSnackbarMessage('config.yaml 配置文件已保存并应用！');
      setIsConfigDrawerOpen(false);
    } catch (e: any) {
      alert('配置格式错误: ' + e.message);
    } finally {
      setSavingConfig(false);
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 模块说明 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <Settings className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            系统设置与自定义 DNS 重写
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            配置本地自定义域名解析（支持 A、AAAA、CNAME 重写）、内存缓存控制以及 YAML 原生配置文件。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleOpenConfigDrawer}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-neutral-800 dark:text-neutral-200 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 rounded-md transition-colors shadow-xs"
          >
            <FileCode className="w-3.5 h-3.5 align-icon" />
            在线查看/编辑 config.yaml
          </button>
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
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-xs"
          >
            <Plus className="w-4 h-4 align-icon" />
            添加重写记录
          </button>
        </div>
      </div>

      {/* DNS 重写记录列表 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="px-5 py-4 border-b border-neutral-200 dark:border-neutral-800 flex items-center justify-between">
          <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">自定义 DNS 重写表 (Hosts)</h4>
          <span className="text-xs text-neutral-500 dark:text-neutral-400 font-mono">{rewrites.length} 条记录</span>
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

      {/* config.yaml 原生配置文件查看与编辑 Drawer */}
      <Drawer
        isOpen={isConfigDrawerOpen}
        onClose={() => setIsConfigDrawerOpen(false)}
        title="原生配置文件 (data/config.yaml)"
        subtitle="支持直接在线编辑核心配置参数并持久化"
        width="w-[640px]"
        footer={
          <div className="flex items-center justify-between w-full">
            <span className="text-xs text-neutral-500">保存前会自动验证 YAML 语法</span>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setIsConfigDrawerOpen(false)}
                className="min-h-[36px] px-4 text-xs text-neutral-600 dark:text-neutral-400"
              >
                取消
              </button>
              <button
                type="button"
                disabled={savingConfig}
                onClick={handleSaveConfig}
                className="inline-flex items-center gap-1.5 min-h-[36px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md shadow-xs disabled:opacity-50"
              >
                <Save className="w-3.5 h-3.5 align-icon" />
                {savingConfig ? '保存中...' : '保存配置文件'}
              </button>
            </div>
          </div>
        }
      >
        <div className="space-y-3">
          <textarea
            rows={20}
            value={rawConfig}
            onChange={(e) => setRawConfig(e.target.value)}
            className="w-full p-3 font-mono text-xs bg-neutral-900 text-neutral-100 border border-neutral-800 rounded-md focus:outline-none focus:border-cyan-500 leading-relaxed resize-y"
          />
        </div>
      </Drawer>

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
            <select
              value={recordType}
              onChange={(e) => setRecordType(e.target.value)}
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="A">A 记录 (IPv4)</option>
              <option value="AAAA">AAAA 记录 (IPv6)</option>
              <option value="CNAME">CNAME 别名</option>
            </select>
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

      {snackbarMessage && (
        <Snackbar message={snackbarMessage} onClose={() => setSnackbarMessage(null)} />
      )}
    </div>
  );
};
