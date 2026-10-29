import React, { useEffect, useState } from 'react';
import {
  Settings,
  Plus,
  Trash2,
  Database,
  RotateCcw,
  Sliders,
  Shield,
  CheckCircle2,
  Globe,
} from 'lucide-react';
import { api, DnsRewriteRecord } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Popconfirm } from '../components/ui/Popconfirm';

export const SettingsPage: React.FC = () => {
  const [rewrites, setRewrites] = useState<DnsRewriteRecord[]>([]);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [domain, setDomain] = useState('');
  const [recordType, setRecordType] = useState('A');
  const [value, setValue] = useState('');
  const [flushSuccess, setFlushSuccess] = useState(false);

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
    } catch (e) {
      console.error('Failed to add rewrite', e);
    }
  };

  const handleDeleteRewrite = async (id: string) => {
    try {
      await api.deleteRewrite(id);
      await fetchRewrites();
    } catch (e) {
      console.error('Failed to delete rewrite', e);
    }
  };

  const handleFlushCache = async () => {
    try {
      await api.flushCache();
      setFlushSuccess(true);
      setTimeout(() => setFlushSuccess(false), 2500);
    } catch (e) {
      console.error('Failed to flush cache', e);
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 模块说明 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-surface-1 border border-neutral-800">
        <div>
          <h3 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
            <Settings className="w-4 h-4 text-cyan-400 align-icon" />
            系统设置与自定义 DNS 重写
          </h3>
          <p className="text-xs text-neutral-400 mt-1 max-w-xl">
            配置本地自定义域名解析（支持 A、AAAA、CNAME 重写）以及 SmartDNS 缓存控制。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleFlushCache}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-amber-300 hover:text-amber-200 bg-amber-950/40 border border-amber-800/60 rounded-md transition-colors shadow-sm"
          >
            <RotateCcw className="w-3.5 h-3.5 align-icon" />
            {flushSuccess ? '缓存已清空！' : '立即刷新 DNS 内存缓存'}
          </button>
          <button
            type="button"
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-sm"
          >
            <Plus className="w-4 h-4 align-icon" />
            添加重写记录
          </button>
        </div>
      </div>

      {/* DNS 重写记录列表 */}
      <div className="rounded-lg bg-surface-1 border border-neutral-800 overflow-hidden">
        <div className="px-5 py-4 border-b border-neutral-800 flex items-center justify-between">
          <h4 className="text-sm font-semibold text-neutral-200">自定义 DNS 重写表</h4>
          <span className="text-xs text-neutral-400 font-mono">{rewrites.length} 条记录</span>
        </div>
        <div className="divide-y divide-neutral-800/60">
          {rewrites.length > 0 ? (
            rewrites.map((r) => (
              <div
                key={r.id}
                className="p-4 flex items-center justify-between hover:bg-surface-2/40 transition-colors"
              >
                <div className="flex items-center gap-4">
                  <span className="px-2 py-0.5 text-xs font-mono font-bold rounded bg-cyan-950/60 text-cyan-400 border border-cyan-800/40">
                    {r.record_type}
                  </span>
                  <div>
                    <div className="text-sm font-mono font-medium text-neutral-100">{r.domain}</div>
                    <div className="text-xs font-mono text-neutral-400 mt-0.5">&rarr; {r.value}</div>
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
                      className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-400 hover:text-red-400 rounded border border-neutral-800 bg-surface-2"
                      title="删除记录"
                    >
                      <Trash2 className="w-4 h-4 align-icon" />
                    </button>
                  </Popconfirm>
                </div>
              </div>
            ))
          ) : (
            <p className="text-xs text-neutral-500 py-10 text-center">暂未配置自定义重写规则</p>
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
            <label className="block text-neutral-300 font-medium mb-1">域名</label>
            <input
              type="text"
              value={domain}
              onChange={(e) => setDomain(e.target.value)}
              placeholder="例如: nas.home 或 gitlab.local"
              required
              className="w-full min-h-[38px] px-3 bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 focus:outline-none focus:border-cyan-500/60"
            />
          </div>

          <div>
            <label className="block text-neutral-300 font-medium mb-1">记录类型</label>
            <select
              value={recordType}
              onChange={(e) => setRecordType(e.target.value)}
              className="w-full min-h-[38px] px-3 bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 focus:outline-none focus:border-cyan-500/60"
            >
              <option value="A">A 记录 (IPv4)</option>
              <option value="AAAA">AAAA 记录 (IPv6)</option>
              <option value="CNAME">CNAME 别名</option>
            </select>
          </div>

          <div>
            <label className="block text-neutral-300 font-medium mb-1">目标 IP 或 目标域名</label>
            <input
              type="text"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder="例如: 192.168.1.100 或 example.com"
              required
              className="w-full min-h-[38px] px-3 bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 focus:outline-none focus:border-cyan-500/60"
            />
          </div>

          <div className="flex justify-end gap-2 pt-3 border-t border-neutral-800">
            <button
              type="button"
              onClick={() => setIsAddModalOpen(false)}
              className="min-h-[38px] px-4 text-neutral-400 hover:text-neutral-200"
            >
              取消
            </button>
            <button
              type="submit"
              className="min-h-[38px] px-5 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-md shadow-sm"
            >
              保存重写
            </button>
          </div>
        </form>
      </Modal>
    </div>
  );
};
