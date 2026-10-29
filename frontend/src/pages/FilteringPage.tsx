import React, { useEffect, useState } from 'react';
import {
  Shield,
  Plus,
  RefreshCw,
  Trash2,
  CheckCircle2,
  XCircle,
  Search,
  FileText,
} from 'lucide-react';
import { api, AdblockRuleRecord } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Snackbar } from '../components/ui/Snackbar';

export const FilteringPage: React.FC = () => {
  const [rules, setRules] = useState<AdblockRuleRecord[]>([]);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [customContent, setCustomContent] = useState('');
  const [testDomain, setTestDomain] = useState('');
  const [testResult, setTestResult] = useState<{
    domain: string;
    blocked: boolean;
    matched_rule?: string;
  } | null>(null);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);
  const [lastDeletedRule, setLastDeletedRule] = useState<AdblockRuleRecord | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const fetchRules = async () => {
    try {
      const res = await api.getRules();
      setRules(res);
    } catch (e) {
      console.error('Failed to fetch rules', e);
    }
  };

  useEffect(() => {
    fetchRules();
  }, []);

  const handleAddRule = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name || (!url && !customContent)) return;
    setSubmitting(true);
    try {
      await api.addRule({
        name,
        url: url || 'custom-local',
        content: customContent || undefined,
      });
      setIsAddModalOpen(false);
      setName('');
      setUrl('');
      setCustomContent('');
      await fetchRules();
    } catch (e) {
      console.error('Failed to add rule', e);
    } finally {
      setSubmitting(false);
    }
  };

  const handleDeleteRule = async (rule: AdblockRuleRecord) => {
    try {
      await api.deleteRule(rule.id);
      setLastDeletedRule(rule);
      setSnackbarMessage(`已删除规则源: ${rule.name}`);
      await fetchRules();
    } catch (e) {
      console.error('Failed to delete rule', e);
    }
  };

  const handleUndoDelete = async () => {
    if (lastDeletedRule) {
      try {
        await api.addRule({
          name: lastDeletedRule.name,
          url: lastDeletedRule.url,
        });
        setLastDeletedRule(null);
        await fetchRules();
      } catch (e) {
        console.error('Failed to undo rule deletion', e);
      }
    }
  };

  const handleRefreshRule = async (id: string) => {
    try {
      await api.refreshRule(id);
      await fetchRules();
    } catch (e) {
      console.error('Failed to refresh rule', e);
    }
  };

  const handleTestRule = async () => {
    if (!testDomain) return;
    try {
      const res = await api.testRule(testDomain);
      setTestResult(res);
    } catch (e) {
      console.error('Failed to test rule', e);
    }
  };

  const totalRules = rules.reduce((acc, r) => acc + r.rule_count, 0);

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部总览 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-surface-1 border border-neutral-800">
        <div>
          <h3 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
            <Shield className="w-4 h-4 text-rose-400 align-icon" />
            AdGuard 规则过滤管理
          </h3>
          <p className="text-xs text-neutral-400 mt-1 max-w-xl">
            全面兼容 Adblock Plus (ABP)、Hosts 文件及正则表达式语法。支持在线 URL 订阅与本地自定义拦截/白名单规则。
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="px-3 py-1 text-xs rounded border border-neutral-700 bg-surface-2 text-neutral-300">
            生效规则总数: <span className="font-mono text-rose-400 font-semibold tabular-nums">{totalRules.toLocaleString()}</span> 条
          </span>
          <button
            type="button"
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-white bg-rose-600 hover:bg-rose-500 rounded-md transition-colors shadow-sm"
          >
            <Plus className="w-4 h-4 align-icon" />
            添加规则订阅
          </button>
        </div>
      </div>

      {/* 快速规则测试验证工具 */}
      <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800">
        <h4 className="text-sm font-semibold text-neutral-200 mb-2 flex items-center gap-2">
          <Search className="w-4 h-4 text-cyan-400 align-icon" />
          域名规则命中诊断
        </h4>
        <p className="text-xs text-neutral-400 mb-3">输入域名立即检查当前规则库是否生效阻断。</p>
        <div className="flex gap-2 max-w-xl">
          <input
            type="text"
            value={testDomain}
            onChange={(e) => setTestDomain(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleTestRule()}
            placeholder="例如: pos.baidu.com 或 analytics.google.com"
            className="flex-1 min-h-[38px] px-3 text-xs bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-cyan-500/60"
          />
          <button
            type="button"
            onClick={handleTestRule}
            className="min-h-[38px] px-4 text-xs font-medium bg-surface-2 border border-neutral-700 hover:border-neutral-600 rounded-md text-neutral-200"
          >
            开始测试
          </button>
        </div>

        {testResult && (
          <div className="mt-4 p-3.5 rounded-lg bg-surface-2 border border-neutral-800 flex items-center justify-between text-xs max-w-xl animate-in fade-in-50 duration-100">
            <div className="flex items-center gap-2">
              {testResult.blocked ? (
                <XCircle className="w-4 h-4 text-rose-400 align-icon" />
              ) : (
                <CheckCircle2 className="w-4 h-4 text-emerald-400 align-icon" />
              )}
              <span className="font-mono text-neutral-200 font-semibold">{testResult.domain}</span>
              <span
                className={`px-2 py-0.5 rounded font-medium ${
                  testResult.blocked ? 'bg-rose-950/60 text-rose-400' : 'bg-emerald-950/60 text-emerald-400'
                }`}
              >
                {testResult.blocked ? '将被阻断拦截' : '允许正常解析'}
              </span>
            </div>
            {testResult.matched_rule && (
              <span className="font-mono text-neutral-400">规则: {testResult.matched_rule}</span>
            )}
          </div>
        )}
      </div>

      {/* 规则源列表 */}
      <div className="rounded-lg bg-surface-1 border border-neutral-800 overflow-hidden">
        <div className="px-5 py-4 border-b border-neutral-800 flex items-center justify-between">
          <h4 className="text-sm font-semibold text-neutral-200">已订阅规则库列表</h4>
        </div>
        <div className="divide-y divide-neutral-800/60">
          {rules.length > 0 ? (
            rules.map((rule) => (
              <div key={rule.id} className="p-4 flex items-center justify-between hover:bg-surface-2/40 transition-colors">
                <div className="flex items-start gap-3">
                  <div className="p-2 rounded bg-surface-2 border border-neutral-800 text-rose-400 mt-0.5">
                    <FileText className="w-4 h-4 align-icon" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-neutral-100">{rule.name}</span>
                      <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-surface-3 text-neutral-400">
                        {rule.rule_count.toLocaleString()} 条
                      </span>
                    </div>
                    <p className="text-xs font-mono text-neutral-400 mt-1 truncate max-w-md">
                      {rule.url}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  {rule.url.startsWith('http') && (
                    <button
                      type="button"
                      onClick={() => handleRefreshRule(rule.id)}
                      className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-400 hover:text-neutral-100 rounded border border-neutral-800 bg-surface-2"
                      title="立即更新"
                    >
                      <RefreshCw className="w-4 h-4 align-icon" />
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => handleDeleteRule(rule)}
                    className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-400 hover:text-red-400 rounded border border-neutral-800 bg-surface-2"
                    title="删除规则源"
                  >
                    <Trash2 className="w-4 h-4 align-icon" />
                  </button>
                </div>
              </div>
            ))
          ) : (
            <p className="text-xs text-neutral-500 py-10 text-center">暂未添加任何拦截规则源</p>
          )}
        </div>
      </div>

      {/* 添加规则 Modal (520px) */}
      <Modal
        isOpen={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        title="添加广告拦截规则源"
        size="md"
      >
        <form onSubmit={handleAddRule} className="space-y-4 text-xs">
          <div>
            <label className="block text-neutral-300 font-medium mb-1">规则源名称</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如: EasyList China 或 自定义反诈拦截"
              required
              className="w-full min-h-[38px] px-3 bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 focus:outline-none focus:border-cyan-500/60"
            />
          </div>

          <div>
            <label className="block text-neutral-300 font-medium mb-1">在线订阅 URL (可选)</label>
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/rules.txt"
              className="w-full min-h-[38px] px-3 bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 focus:outline-none focus:border-cyan-500/60"
            />
          </div>

          <div>
            <label className="block text-neutral-300 font-medium mb-1">
              直接粘贴规则内容 (支持 ||domain^、Hosts 或 @@白名单)
            </label>
            <textarea
              rows={5}
              value={customContent}
              onChange={(e) => setCustomContent(e.target.value)}
              placeholder="||ad.example.com^&#10;0.0.0.0 tracking.com&#10;@@||whitelist.com^"
              className="w-full p-3 font-mono bg-surface-2 border border-neutral-700/80 rounded-md text-neutral-100 focus:outline-none focus:border-cyan-500/60"
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
              disabled={submitting}
              className="min-h-[38px] px-5 bg-rose-600 hover:bg-rose-500 text-white font-medium rounded-md shadow-sm"
            >
              {submitting ? '加载规则中...' : '确认添加'}
            </button>
          </div>
        </form>
      </Modal>

      {/* 可恢复删除 Snackbar 提示 */}
      {snackbarMessage && (
        <Snackbar
          message={snackbarMessage}
          onUndo={handleUndoDelete}
          onClose={() => setSnackbarMessage(null)}
        />
      )}
    </div>
  );
};
