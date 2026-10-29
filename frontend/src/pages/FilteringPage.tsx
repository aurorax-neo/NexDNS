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
  Eye,
  Edit3,
  Save,
} from 'lucide-react';
import { api, AdblockRuleRecord } from '../lib/api';
import { Modal } from '../components/ui/Modal';
import { Snackbar } from '../components/ui/Snackbar';
import { Switch } from '../components/ui/Switch';
import { Drawer } from '../components/ui/Drawer';
import { Popconfirm } from '../components/ui/Popconfirm';

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

  // Drawer view/edit rule content state
  const [selectedRuleForView, setSelectedRuleForView] = useState<AdblockRuleRecord | null>(null);
  const [drawerContent, setDrawerContent] = useState('');
  const [drawerSearch, setDrawerSearch] = useState('');
  const [isEditingContent, setIsEditingContent] = useState(false);
  const [savingContent, setSavingContent] = useState(false);

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

  const handleToggleRule = async (ruleId: string, currentEnabled: boolean) => {
    try {
      await api.toggleRule(ruleId);
      setRules((prev) =>
        prev.map((r) => (r.id === ruleId ? { ...r, enabled: !currentEnabled } : r))
      );
      setSnackbarMessage(`已${!currentEnabled ? '启用' : '停用'}规则源`);
    } catch (e) {
      console.error('Failed to toggle rule', e);
    }
  };

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
      setSnackbarMessage('规则源已从在线订阅源拉取最新版本！');
    } catch (e) {
      console.error('Failed to refresh rule', e);
    }
  };

  const handleOpenContentDrawer = async (rule: AdblockRuleRecord) => {
    setSelectedRuleForView(rule);
    setDrawerSearch('');
    setIsEditingContent(false);
    try {
      const res = await api.getRuleContent(rule.id);
      setDrawerContent(res.content || '');
    } catch (e) {
      console.error('Failed to load rule content', e);
      setDrawerContent('');
    }
  };

  const handleSaveContent = async () => {
    if (!selectedRuleForView) return;
    setSavingContent(true);
    try {
      await api.updateRuleContent(selectedRuleForView.id, drawerContent);
      setIsEditingContent(false);
      await fetchRules();
      setSnackbarMessage(`已保存规则源 [${selectedRuleForView.name}] 并实时热生效！`);
    } catch (e) {
      console.error('Failed to update rule content', e);
    } finally {
      setSavingContent(false);
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

  const handleAddQuickFromTest = async (action: 'block' | 'whitelist') => {
    if (!testDomain) return;
    try {
      await api.quickAddRule(testDomain, action);
      setSnackbarMessage(action === 'block' ? `已将 ${testDomain} 加入黑名单` : `已将 ${testDomain} 加入白名单`);
      await fetchRules();
      await handleTestRule();
    } catch (e) {
      console.error('Failed to quick add rule', e);
    }
  };

  const totalRules = rules.reduce((acc, r) => acc + r.rule_count, 0);

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部总览 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <Shield className="w-4 h-4 text-rose-500 dark:text-rose-400 align-icon" />
            规则订阅与过滤管理
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            支持 Hosts 规范、通配符规则及正则表达式语法。支持在线 URL 订阅与本地自定义拦截/白名单规则。
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="px-3 py-1 text-xs rounded border border-neutral-200 dark:border-neutral-700 bg-neutral-100 dark:bg-surface-2 text-neutral-700 dark:text-neutral-300">
            生效规则总数: <span className="font-mono text-rose-600 dark:text-rose-400 font-semibold tabular-nums">{totalRules.toLocaleString()}</span> 条
          </span>
          <button
            type="button"
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-white bg-rose-600 hover:bg-rose-500 rounded-md transition-colors shadow-xs"
          >
            <Plus className="w-4 h-4 align-icon" />
            添加规则订阅
          </button>
        </div>
      </div>

      {/* 快速规则测试验证工具 */}
      <div className="p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 mb-2 flex items-center gap-2">
          <Search className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
          域名规则命中诊断
        </h4>
        <p className="text-xs text-neutral-500 dark:text-neutral-400 mb-3">输入域名即时核验当前规则库是否生效，并可一键加入黑白名单。</p>
        <div className="flex gap-2 max-w-xl">
          <input
            type="text"
            value={testDomain}
            onChange={(e) => setTestDomain(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleTestRule()}
            placeholder="例如: pos.baidu.com 或 analytics.google.com"
            className="flex-1 min-h-[38px] px-3 text-xs bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 rounded-md text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500"
          />
          <button
            type="button"
            onClick={handleTestRule}
            className="min-h-[38px] px-4 text-xs font-medium bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 rounded-md text-neutral-800 dark:text-neutral-200"
          >
            开始测试
          </button>
        </div>

        {testResult && (
          <div className="mt-4 p-3.5 rounded-lg bg-neutral-50 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 flex flex-wrap items-center justify-between gap-3 text-xs max-w-xl animate-in fade-in-50 duration-100">
            <div className="flex items-center gap-2.5">
              {testResult.blocked ? (
                <XCircle className="w-4 h-4 text-rose-500 align-icon" />
              ) : (
                <CheckCircle2 className="w-4 h-4 text-emerald-500 align-icon" />
              )}
              <span className="font-mono text-neutral-900 dark:text-neutral-100 font-semibold">{testResult.domain}</span>
              <span
                className={`px-2 py-0.5 rounded font-medium ${
                  testResult.blocked ? 'bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-400 border border-rose-200 dark:border-rose-800/40' : 'bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800/40'
                }`}
              >
                {testResult.blocked ? '将被阻断拦截' : '允许正常解析'}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => handleAddQuickFromTest('block')}
                className="px-2 py-1 text-[11px] rounded bg-rose-600 hover:bg-rose-500 text-white font-medium"
              >
                加入拦截
              </button>
              <button
                type="button"
                onClick={() => handleAddQuickFromTest('whitelist')}
                className="px-2 py-1 text-[11px] rounded bg-emerald-600 hover:bg-emerald-500 text-white font-medium"
              >
                加入白名单
              </button>
            </div>
          </div>
        )}
      </div>

      {/* 规则源列表 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="px-5 py-4 border-b border-neutral-200 dark:border-neutral-800 flex items-center justify-between">
          <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">已订阅规则库列表</h4>
        </div>
        <div className="divide-y divide-neutral-200 dark:divide-neutral-800/60">
          {rules.length > 0 ? (
            rules.map((rule) => (
              <div key={rule.id} className="p-4 flex items-center justify-between hover:bg-neutral-50 dark:hover:bg-surface-2/40 transition-colors">
                <div className="flex items-start gap-3">
                  <div className="p-2 rounded bg-neutral-100 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 text-rose-500 dark:text-rose-400 mt-0.5">
                    <FileText className="w-4 h-4 align-icon" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-neutral-900 dark:text-neutral-100">{rule.name}</span>
                      <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-neutral-100 dark:bg-surface-3 text-neutral-600 dark:text-neutral-400">
                        {rule.rule_count.toLocaleString()} 条
                      </span>
                      {!rule.enabled && (
                        <span className="px-1.5 py-0.5 text-[10px] rounded bg-neutral-200 dark:bg-neutral-800 text-neutral-500">
                          已禁用
                        </span>
                      )}
                    </div>
                    <p className="text-xs font-mono text-neutral-500 dark:text-neutral-400 mt-1 truncate max-w-md">
                      {rule.url}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-3">
                  <Switch
                    checked={rule.enabled}
                    onChange={() => handleToggleRule(rule.id, rule.enabled)}
                    ariaLabel={`切换 ${rule.name}`}
                  />
                  <button
                    type="button"
                    onClick={() => handleOpenContentDrawer(rule)}
                    className="min-h-[36px] px-3.5 flex items-center gap-1.5 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                    title="查看与编辑规则内容"
                  >
                    <Eye className="w-3.5 h-3.5 align-icon" />
                    查看内容
                  </button>
                  <button
                    type="button"
                    onClick={() => handleRefreshRule(rule.id)}
                    disabled={!rule.url || !rule.url.startsWith('http')}
                    className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 disabled:opacity-30 disabled:cursor-not-allowed rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                    title={rule.url && rule.url.startsWith('http') ? '立即更新' : '本地自定义规则无需在线更新'}
                  >
                    <RefreshCw className="w-4 h-4 align-icon" />
                  </button>
                  <Popconfirm
                    title="确认删除该规则源？"
                    actionText="确认删除"
                    onConfirm={() => handleDeleteRule(rule)}
                  >
                    <button
                      type="button"
                    className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-600 dark:text-neutral-400 hover:text-red-500 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2"
                    title="删除规则源"
                  >
                    <Trash2 className="w-4 h-4 align-icon" />
                  </button>
                  </Popconfirm>
                </div>
              </div>
            ))
          ) : (
            <p className="text-xs text-neutral-400 py-10 text-center">暂未添加任何拦截规则源</p>
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
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">规则源名称</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如: 基础防追踪规则 或 自定义拦截名单"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
            {name && rules.some((r) => r.name.trim().toLowerCase() === name.trim().toLowerCase()) && (
              <p className="mt-1 text-[11px] text-amber-600 dark:text-amber-400">
                已存在同名规则源，保存时系统将自动添加序号命名保存
              </p>
            )}
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">在线订阅 URL (可选)</label>
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/rules.txt"
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              直接粘贴规则内容 (支持 ||domain^、Hosts 或 @@白名单)
            </label>
            <textarea
              rows={5}
              value={customContent}
              onChange={(e) => setCustomContent(e.target.value)}
              placeholder="||ad.example.com^&#10;0.0.0.0 tracking.com&#10;@@||whitelist.com^"
              className="w-full p-3 font-mono bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div className="flex justify-end gap-2 pt-3 border-t border-neutral-200 dark:border-neutral-800">
            <button
              type="button"
              onClick={() => setIsAddModalOpen(false)}
              className="min-h-[38px] px-4 text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-200"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="min-h-[38px] px-5 bg-rose-600 hover:bg-rose-500 text-white font-medium rounded-md shadow-xs"
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

      {/* 规则源内容查看与编辑 Drawer */}
      <Drawer
        isOpen={!!selectedRuleForView}
        onClose={() => setSelectedRuleForView(null)}
        title={selectedRuleForView?.name || '规则内容详情'}
        subtitle={`条目总数: ${selectedRuleForView?.rule_count.toLocaleString() ?? 0} 条 | 来源: ${selectedRuleForView?.url}`}
      >
        {selectedRuleForView && (
          <div className="space-y-4">
            <div className="flex items-center justify-between gap-3">
              <div className="relative flex-1">
                <Search className="w-4 h-4 text-neutral-400 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
                <input
                  type="text"
                  value={drawerSearch}
                  onChange={(e) => setDrawerSearch(e.target.value)}
                  placeholder="搜索规则内容 (例如: ||example^ 或 google)..."
                  className="w-full min-h-[36px] pl-9 pr-3 text-xs bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-rose-500 font-mono"
                />
              </div>

              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setIsEditingContent(!isEditingContent)}
                  className={`min-h-[36px] px-3.5 text-xs flex items-center gap-1.5 rounded border transition-colors ${
                    isEditingContent
                      ? 'bg-rose-50 dark:bg-rose-950/60 border-rose-300 dark:border-rose-800 text-rose-700 dark:text-rose-400 font-medium'
                      : 'bg-white dark:bg-surface-2 border-neutral-300 dark:border-neutral-700 text-neutral-700 dark:text-neutral-300'
                  }`}
                >
                  <Edit3 className="w-3.5 h-3.5 align-icon" />
                  {isEditingContent ? '正在编辑' : '在线编辑'}
                </button>

                {isEditingContent && (
                  <button
                    type="button"
                    onClick={handleSaveContent}
                    disabled={savingContent}
                    className="min-h-[36px] px-4 text-xs font-medium bg-rose-600 hover:bg-rose-500 text-white rounded shadow-xs flex items-center gap-1.5 transition-colors disabled:opacity-50"
                  >
                    <Save className="w-3.5 h-3.5 align-icon" />
                    {savingContent ? '保存中...' : '保存修改'}
                  </button>
                )}
              </div>
            </div>

            {isEditingContent ? (
              <textarea
                rows={22}
                value={drawerContent}
                onChange={(e) => setDrawerContent(e.target.value)}
                className="w-full p-3 font-mono text-xs bg-neutral-900 text-neutral-100 border border-neutral-800 rounded-md focus:outline-none focus:border-rose-500 leading-relaxed resize-y"
                placeholder="在此直接修改或输入规则..."
              />
            ) : (
              <div className="max-h-[560px] overflow-y-auto p-3 font-mono text-xs rounded-md bg-neutral-900 text-neutral-100 border border-neutral-800 space-y-1">
                {drawerContent
                  .split('\n')
                  .filter((l) => !drawerSearch || l.toLowerCase().includes(drawerSearch.toLowerCase()))
                  .slice(0, 500)
                  .map((line, idx) => (
                    <div key={idx} className="flex gap-3 hover:bg-neutral-800/60 px-1 py-0.5 rounded">
                      <span className="text-neutral-600 select-none text-right w-10 shrink-0 font-mono text-[11px]">
                        {idx + 1}
                      </span>
                      <span
                        className={`break-all ${
                          line.startsWith('!') || line.startsWith('#')
                            ? 'text-neutral-500 italic'
                            : line.startsWith('@@')
                            ? 'text-emerald-400 font-semibold'
                            : 'text-neutral-200'
                        }`}
                      >
                        {line || ' '}
                      </span>
                    </div>
                  ))}
                {drawerContent.split('\n').length > 500 && !drawerSearch && (
                  <div className="text-center py-2 text-neutral-500 text-[11px] border-t border-neutral-800 mt-2">
                    已预览前 500 行，使用上方搜索栏可精确定位全文规则
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </Drawer>
    </div>
  );
};
