import React, { useEffect, useState } from 'react';
import {
  GitFork,
  Plus,
  Trash2,
  RotateCcw,
  Search,
} from 'lucide-react';
import { api, DnsRewriteRecord } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Popconfirm } from '../components/ui/Popconfirm';
import { CustomSelect } from '../components/ui/CustomSelect';
import { Snackbar } from '../components/ui/Snackbar';

export const RewritesPage: React.FC = () => {
  const [rewrites, setRewrites] = useState<DnsRewriteRecord[]>([]);
  const [search, setSearch] = useState('');
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [domain, setDomain] = useState('');
  const [recordType, setRecordType] = useState('A');
  const [value, setValue] = useState('');
  const [flushSuccess, setFlushSuccess] = useState(false);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  const loadData = async () => {
    try {
      const res = await api.getRewrites();
      setRewrites(res);
    } catch (e) {
      console.error('Failed to load rewrites', e);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleAddRewrite = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!domain || !value) return;
    try {
      await api.addRewrite({ domain, record_type: recordType, value });
      setIsAddModalOpen(false);
      setDomain('');
      setValue('');
      await loadData();
      setSnackbarMessage('已成功添加 DNS 本地重写映射！');
    } catch (e) {
      console.error('Failed to add rewrite', e);
    }
  };

  const handleDeleteRewrite = async (id: string) => {
    try {
      await api.deleteRewrite(id);
      await loadData();
      setSnackbarMessage('已删除 DNS 重写记录');
    } catch (e) {
      console.error('Failed to delete rewrite', e);
    }
  };

  const handleFlushCache = async () => {
    try {
      await api.flushCache();
      setFlushSuccess(true);
      setSnackbarMessage('已成功清空内存缓存！');
      setTimeout(() => setFlushSuccess(false), 2000);
    } catch (e) {
      console.error('Failed to flush cache', e);
    }
  };

  const filteredRewrites = search
    ? rewrites.filter((r) => r.domain.toLowerCase().includes(search.toLowerCase()) || r.value.toLowerCase().includes(search.toLowerCase()))
    : rewrites;

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 头部控制栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <GitFork className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            自定义 DNS 重写与本地 Hosts
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            配置本地专用的 A / AAAA / CNAME 权威映射。优先应答指定域名，无需依赖外部公共上游。
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
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-xs"
          >
            <Plus className="w-3.5 h-3.5 align-icon" />
            添加重写记录
          </button>
        </div>
      </div>

      {/* 筛选与记录列表卡片 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="p-4 border-b border-neutral-200 dark:border-neutral-800 flex items-center justify-between gap-4">
          <div className="relative flex-1 max-w-md">
            <Search className="w-4 h-4 text-neutral-400 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索重写域名或目标 IP..."
              className="w-full min-h-[36px] pl-9 pr-3 text-xs bg-neutral-50 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500 font-mono"
            />
          </div>
          <span className="text-xs text-neutral-500 font-mono">共 {filteredRewrites.length} 条映射记录</span>
        </div>

        <div className="divide-y divide-neutral-200 dark:divide-neutral-800/60">
          {filteredRewrites.length > 0 ? (
            filteredRewrites.map((r) => (
              <div
                key={r.id}
                className="p-4 flex items-center justify-between hover:bg-neutral-50 dark:hover:bg-surface-2/40 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <span
                    className={`px-2 py-0.5 text-xs font-mono font-bold rounded ${
                      r.record_type === 'A'
                        ? 'bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-400 border border-cyan-200 dark:border-cyan-800/40'
                        : r.record_type === 'AAAA'
                        ? 'bg-purple-50 dark:bg-purple-950/60 text-purple-700 dark:text-purple-400 border border-purple-200 dark:border-purple-800/40'
                        : 'bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-400 border border-indigo-200 dark:border-indigo-800/40'
                    }`}
                  >
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
                      className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-500 dark:text-neutral-400 hover:text-red-500 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
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

      {/* 添加重写 Modal */}
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
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
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
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">目标 IP 或 目标别名</label>
            <input
              type="text"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder="例如: 192.168.1.100 或 local.example.com"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
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
