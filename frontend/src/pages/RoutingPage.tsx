import React, { useEffect, useState } from 'react';
import {
  FileCode,
  RefreshCw,
  Search,
  Save,
  Edit3,
  Settings2,
  Plus,
  Trash2,
  Eye,
} from 'lucide-react';
import { api, RuleFileInfo } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';
import { Modal } from '../components/ui/Modal';
import { Switch } from '../components/ui/Switch';
import { CustomSelect } from '../components/ui/CustomSelect';
import { Snackbar } from '../components/ui/Snackbar';
import { Popconfirm } from '../components/ui/Popconfirm';

export const RoutingPage: React.FC = () => {
  const [ruleFiles, setRuleFiles] = useState<RuleFileInfo[]>([]);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [activeGroup, setActiveGroup] = useState<string>('all');

  // Drawer view/edit state
  const [selectedFile, setSelectedFile] = useState<RuleFileInfo | null>(null);
  const [fileContent, setFileContent] = useState('');
  const [searchLine, setSearchLine] = useState('');
  const [savingFile, setSavingFile] = useState(false);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

  // Add rule modal state
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [addName, setAddName] = useState('');
  const [addFileName, setAddFileName] = useState('');
  const [addGroup, setAddGroup] = useState('mainland');
  const [addUrl, setAddUrl] = useState('');
  const [addContent, setAddContent] = useState('');
  const [addingRule, setAddingRule] = useState(false);

  // Edit rule info modal state
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<RuleFileInfo | null>(null);
  const [editName, setEditName] = useState('');
  const [editGroup, setEditGroup] = useState('mainland');
  const [editUrl, setEditUrl] = useState('');
  const [savingEdit, setSavingEdit] = useState(false);

  const fetchRules = async () => {
    try {
      const res = await api.getRoutingRules();
      setRuleFiles(res);
    } catch (e) {
      console.error('Failed to load routing rules', e);
    }
  };

  useEffect(() => {
    fetchRules();
  }, []);

  const handleUpdateAll = async () => {
    setUpdatingAll(true);
    try {
      await api.updateRoutingRules();
      await fetchRules();
      setSnackbarMessage('已全量拉取并重载所有分流规则集！');
    } catch (e) {
      console.error('Failed to update routing rules', e);
    } finally {
      setTimeout(() => setUpdatingAll(false), 600);
    }
  };

  const handleToggleRule = async (id: string, currentEnabled: boolean) => {
    try {
      await api.toggleRoutingRule(id);
      setRuleFiles((prev) =>
        prev.map((rf) => (rf.id === id ? { ...rf, enabled: !currentEnabled } : rf))
      );
      setSnackbarMessage(`已${!currentEnabled ? '启用' : '停用'}规则文件`);
    } catch (e) {
      console.error('Failed to toggle routing rule', e);
    }
  };

  const handleRefreshSingle = async (id: string) => {
    setRefreshingId(id);
    try {
      await api.refreshRoutingRule(id);
      await fetchRules();
      setSnackbarMessage('规则文件已从在线源拉取最新版本！');
    } catch (e) {
      console.error('Failed to refresh rule file', e);
    } finally {
      setTimeout(() => setRefreshingId(null), 600);
    }
  };

  const handleOpenEditModal = (file: RuleFileInfo) => {
    setEditingRule(file);
    setEditName(file.name);
    setEditGroup(file.group_type);
    setEditUrl(file.url || '');
    setIsEditModalOpen(true);
  };

  const handleEditSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingRule || !editName.trim()) return;
    setSavingEdit(true);
    try {
      await api.editRoutingRuleInfo(editingRule.id, {
        name: editName.trim(),
        group_type: editGroup,
        url: editUrl.trim(),
      });
      setIsEditModalOpen(false);
      setEditingRule(null);
      await fetchRules();
      setSnackbarMessage('已成功更新规则集详细信息并热重载！');
    } catch (e) {
      console.error('Failed to edit routing rule info', e);
    } finally {
      setSavingEdit(false);
    }
  };

  const handleDeleteFile = async (id: string) => {
    try {
      await api.deleteRoutingRule(id);
      await fetchRules();
      setSnackbarMessage('已删除规则文件');
    } catch (e) {
      console.error('Failed to delete rule file', e);
    }
  };

  const handleAddSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!addName || !addFileName) return;
    setAddingRule(true);
    try {
      await api.addRoutingRule({
        name: addName,
        file_name: addFileName.endsWith('.txt') ? addFileName : `${addFileName}.txt`,
        group_type: addGroup,
        url: addUrl || undefined,
        content: addContent || undefined,
      });
      setIsAddModalOpen(false);
      setAddName('');
      setAddFileName('');
      setAddUrl('');
      setAddContent('');
      await fetchRules();
      setSnackbarMessage('已成功添加新规则文件！');
    } catch (e) {
      console.error('Failed to add routing rule', e);
    } finally {
      setAddingRule(false);
    }
  };

  const handleOpenFile = async (file: RuleFileInfo) => {
    setSelectedFile(file);
    setSearchLine('');
    try {
      const res = await api.getRoutingRuleContent(file.file_name);
      setFileContent(res.content || '');
    } catch (e) {
      console.error('Failed to load file content', e);
    }
  };

  const handleSaveFile = async () => {
    if (!selectedFile) return;
    setSavingFile(true);
    try {
      await api.updateRoutingRuleContent(selectedFile.file_name, fileContent);
      setSnackbarMessage(`${selectedFile.file_name} 已保存并即时热重载！`);
      setSelectedFile(null);
      await fetchRules();
    } catch (e) {
      console.error('Failed to save file content', e);
    } finally {
      setSavingFile(false);
    }
  };

  const getGroupBadge = (group: string) => {
    switch (group) {
      case 'mainland':
        return (
          <span className="px-2 py-0.5 text-xs font-medium rounded bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-400 border border-cyan-200 dark:border-cyan-800/40">
            大陆直连组
          </span>
        );
      case 'gfw':
      case 'proxy':
        return (
          <span className="px-2 py-0.5 text-xs font-medium rounded bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-400 border border-rose-200 dark:border-rose-800/40">
            代理过滤组
          </span>
        );
      case 'private':
        return (
          <span className="px-2 py-0.5 text-xs font-medium rounded bg-purple-50 dark:bg-purple-950/60 text-purple-700 dark:text-purple-400 border border-purple-200 dark:border-purple-800/40">
            私网局域网
          </span>
        );
      case 'china_ip':
        return (
          <span className="px-2 py-0.5 text-xs font-medium rounded bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-800/40">
            中国 IP 库
          </span>
        );
      default:
        return (
          <span className="px-2 py-0.5 text-xs font-medium rounded bg-neutral-100 dark:bg-surface-3 text-neutral-600 dark:text-neutral-400">
            {group}
          </span>
        );
    }
  };

  const allLines = fileContent.split('\n');
  const filteredLines = searchLine
    ? allLines.filter((l) => l.toLowerCase().includes(searchLine.toLowerCase()))
    : allLines;

  const filteredFiles = activeGroup === 'all'
    ? ruleFiles
    : ruleFiles.filter((rf) => rf.group_type === activeGroup);

  const groupFilterOptions = [
    { value: 'all', label: '全部规则文件' },
    { value: 'mainland', label: '大陆直连组' },
    { value: 'gfw', label: '代理过滤组' },
    { value: 'private', label: '私网局域网' },
    { value: 'china_ip', label: '中国 IP 库' },
  ];

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部控制横幅 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <FileCode className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            分流规则集与分组多文件管理
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            支持多文件分组配置。不同规则文件可自由划分至大陆直连组、代理过滤组、私网组或中国 IP 库，支持独立启停与行级查看编辑。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setIsAddModalOpen(true)}
            className="inline-flex items-center gap-1.5 min-h-[38px] px-3.5 text-xs font-medium text-neutral-800 dark:text-neutral-200 bg-neutral-100 dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 hover:border-neutral-400 rounded-md shadow-xs transition-colors"
          >
            <Plus className="w-3.5 h-3.5 align-icon" />
            添加规则文件
          </button>
          <button
            type="button"
            onClick={handleUpdateAll}
            disabled={updatingAll}
            className="inline-flex items-center gap-2 min-h-[38px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md shadow-xs disabled:opacity-50 transition-colors"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${updatingAll ? 'animate-spin' : ''}`} />
            {updatingAll ? '正在更新中...' : '一键全量更新规则集'}
          </button>
        </div>
      </div>

      {/* 分组筛选栏 */}
      <div className="flex items-center justify-between gap-4 p-4 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div className="flex items-center gap-2">
          <span className="text-xs text-neutral-500 dark:text-neutral-400">所属分组:</span>
          <CustomSelect
            options={groupFilterOptions}
            value={activeGroup}
            onChange={setActiveGroup}
          />
        </div>
        <span className="text-xs font-mono text-neutral-500">
          当前显示 {filteredFiles.length} 个规则文件
        </span>
      </div>

      {/* 规则集列表卡片 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="divide-y divide-neutral-200 dark:divide-neutral-800/60">
          {filteredFiles.map((rf) => (
            <div
              key={rf.id || rf.file_name}
              className={`p-4 flex flex-wrap items-center justify-between gap-3 transition-colors ${
                rf.enabled ? 'hover:bg-neutral-50 dark:hover:bg-surface-2/40' : 'opacity-60 bg-neutral-50/50 dark:bg-surface-2/10'
              }`}
            >
              <div className="flex items-start gap-3 flex-1 min-w-0 pr-4">
                <div className="p-2 rounded bg-neutral-100 dark:bg-surface-2 border border-neutral-200 dark:border-neutral-800 text-cyan-600 dark:text-cyan-400 mt-0.5 shrink-0">
                  <FileCode className="w-4 h-4 align-icon" />
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold text-neutral-900 dark:text-neutral-100 truncate">
                      {rf.name}
                    </span>
                    {getGroupBadge(rf.group_type)}
                    <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-neutral-100 dark:bg-surface-3 text-neutral-700 dark:text-neutral-300 shrink-0">
                      {rf.count.toLocaleString()} 条规则
                    </span>
                    {!rf.enabled && (
                      <span className="px-1.5 py-0.5 text-[10px] rounded bg-neutral-200 dark:bg-neutral-800 text-neutral-500">
                        已停用
                      </span>
                    )}
                  </div>
                  <p className="text-xs font-mono text-neutral-500 dark:text-neutral-400 mt-1 truncate">
                    文件名: {rf.file_name}
                    <span className="mx-2 text-neutral-300 dark:text-neutral-700">|</span>
                    大小: {(rf.size_bytes / 1024).toFixed(1)} KB
                    <span className="mx-2 text-neutral-300 dark:text-neutral-700">|</span>
                    更新: {rf.updated_at.split('T')[0] || '-'}
                  </p>
                </div>
              </div>

              <div className="flex items-center gap-3 shrink-0">
                {/* 启停 Switch */}
                <Switch
                  checked={rf.enabled}
                  onChange={() => handleToggleRule(rf.id, rf.enabled)}
                  ariaLabel={`切换 ${rf.name}`}
                />

                {/* 编辑详细信息: 纯图标按钮无文字 */}
                <button
                  type="button"
                  onClick={() => handleOpenEditModal(rf)}
                  className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                  title="编辑规则集信息"
                >
                  <Settings2 className="w-4 h-4 align-icon" />
                </button>

                {/* 查看规则内容: 纯图标按钮无文字 */}
                <button
                  type="button"
                  onClick={() => handleOpenFile(rf)}
                  className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                  title="查看与编辑规则内容"
                >
                  <Eye className="w-4 h-4 align-icon" />
                </button>

                {/* 在线刷新按钮: 有 URL 则可点，无 URL 禁用置灰 */}
                <button
                  type="button"
                  onClick={() => handleRefreshSingle(rf.id)}
                  disabled={!rf.url || !rf.url.startsWith('http') || refreshingId === rf.id}
                  className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 disabled:opacity-30 disabled:cursor-not-allowed rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                  title={rf.url && rf.url.startsWith('http') ? '拉取在线最新版本' : '未配置在线订阅 URL，不可在线刷新'}
                >
                  <RefreshCw className={`w-4 h-4 align-icon ${refreshingId === rf.id ? 'animate-spin text-cyan-500' : ''}`} />
                </button>

                {/* 删除规则文件: 核心文件禁用置灰，自定义文件可删除 */}
                {['direct.txt', 'proxy.txt', 'private.txt', 'ipv4_china.txt', 'ipv6_china.txt'].includes(rf.file_name) ? (
                  <button
                    type="button"
                    disabled
                    className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-300 dark:text-neutral-700 cursor-not-allowed rounded border border-neutral-200 dark:border-neutral-800/40 bg-neutral-100 dark:bg-surface-3/30"
                    title="系统核心规则文件禁止删除"
                  >
                    <Trash2 className="w-4 h-4 align-icon" />
                  </button>
                ) : (
                  <Popconfirm
                    title="确认删除该规则文件？"
                    actionText="确认删除"
                    onConfirm={() => handleDeleteFile(rf.id)}
                  >
                    <button
                      type="button"
                      className="min-w-[36px] min-h-[36px] flex items-center justify-center text-neutral-500 hover:text-red-500 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                      title="删除规则文件"
                    >
                      <Trash2 className="w-4 h-4 align-icon" />
                    </button>
                  </Popconfirm>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* 添加分流规则文件 Modal */}
      <Modal
        isOpen={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        title="添加分流规则文件"
        size="md"
      >
        <form onSubmit={handleAddSubmit} className="space-y-4 text-xs">
          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              规则集名称
            </label>
            <input
              type="text"
              value={addName}
              onChange={(e) => setAddName(e.target.value)}
              placeholder="例如: Apple 国内加速直连 或 常用国外代理"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
                归属分流组
              </label>
              <CustomSelect
                options={[
                  { value: 'mainland', label: '大陆直连组 (mainland)' },
                  { value: 'gfw', label: '代理过滤组 (gfw)' },
                  { value: 'private', label: '私网局域网 (private)' },
                  { value: 'china_ip', label: '中国 IP 库 (china_ip)' },
                ]}
                value={addGroup}
                onChange={setAddGroup}
                className="w-full"
              />
            </div>
            <div>
              <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
                存储文件名
              </label>
              <input
                type="text"
                value={addFileName}
                onChange={(e) => setAddFileName(e.target.value)}
                placeholder="例如: apple_direct.txt"
                required
                className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
              />
              {addFileName && ruleFiles.some((f) => f.file_name.toLowerCase() === (addFileName.endsWith('.txt') ? addFileName : `${addFileName}.txt`).toLowerCase()) && (
                <p className="mt-1 text-[11px] text-amber-600 dark:text-amber-400">
                  检测到同名规则文件，保存时将自动重命名并独立保存，不会覆盖现有文件
                </p>
              )}
            </div>
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              在线订阅 URL (可选)
            </label>
            <input
              type="url"
              value={addUrl}
              onChange={(e) => setAddUrl(e.target.value)}
              placeholder="https://raw.githubusercontent.com/.../rules.txt"
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              规则内容 (支持直接粘贴域名列表或 CIDR)
            </label>
            <textarea
              rows={5}
              value={addContent}
              onChange={(e) => setAddContent(e.target.value)}
              placeholder="+.apple.com&#10;+.icloud.com"
              className="w-full p-3 font-mono bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
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
              disabled={addingRule}
              className="min-h-[36px] px-5 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-md shadow-xs disabled:opacity-50"
            >
              {addingRule ? '正在添加...' : '确认添加'}
            </button>
          </div>
        </form>
      </Modal>

      {/* 编辑规则集详细信息 Modal */}
      <Modal
        isOpen={isEditModalOpen}
        onClose={() => setIsEditModalOpen(false)}
        title="编辑分流规则集详细信息"
        size="md"
      >
        <form onSubmit={handleEditSubmit} className="space-y-4 text-xs">
          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              规则集名称
            </label>
            <input
              type="text"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              placeholder="规则集显示名称"
              required
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
                归属分流组
              </label>
              <CustomSelect
                options={[
                  { value: 'mainland', label: '大陆直连组 (mainland)' },
                  { value: 'gfw', label: '代理过滤组 (gfw)' },
                  { value: 'private', label: '私网局域网 (private)' },
                  { value: 'china_ip', label: '中国 IP 库 (china_ip)' },
                ]}
                value={editGroup}
                onChange={setEditGroup}
                className="w-full"
              />
            </div>
            <div>
              <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
                存储文件名 (只读)
              </label>
              <input
                type="text"
                value={editingRule?.file_name || ''}
                disabled
                className="w-full min-h-[38px] px-3 bg-neutral-100 dark:bg-surface-3/50 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-500 font-mono cursor-not-allowed"
              />
            </div>
          </div>

          <div>
            <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
              在线更新订阅 URL
            </label>
            <input
              type="url"
              value={editUrl}
              onChange={(e) => setEditUrl(e.target.value)}
              placeholder="https://raw.githubusercontent.com/.../rules.txt"
              className="w-full min-h-[38px] px-3 bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 font-mono focus:outline-none focus:border-cyan-500"
            />
          </div>

          <div className="flex justify-end gap-2 pt-3 border-t border-neutral-200 dark:border-neutral-800">
            <button
              type="button"
              onClick={() => setIsEditModalOpen(false)}
              className="min-h-[36px] px-4 text-neutral-600 dark:text-neutral-400"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={savingEdit}
              className="min-h-[36px] px-5 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-md shadow-xs disabled:opacity-50"
            >
              {savingEdit ? '正在保存...' : '保存修改'}
            </button>
          </div>
        </form>
      </Modal>

      {/* 规则文件查看与编辑 Drawer */}
      <Drawer
        isOpen={!!selectedFile}
        onClose={() => setSelectedFile(null)}
        title={selectedFile ? `规则文件: ${selectedFile.file_name}` : '规则文件'}
        subtitle={`共 ${allLines.length} 行 | 支持直接编辑修改`}
        width="w-[700px]"
        footer={
          <div className="flex items-center justify-between w-full">
            <span className="text-xs text-neutral-500">
              保存后分流引擎立即热重载
            </span>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setSelectedFile(null)}
                className="min-h-[36px] px-4 text-xs text-neutral-600 dark:text-neutral-400"
              >
                关闭
              </button>
              <button
                type="button"
                disabled={savingFile}
                onClick={handleSaveFile}
                className="inline-flex items-center gap-1.5 min-h-[36px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md shadow-xs disabled:opacity-50"
              >
                <Save className="w-3.5 h-3.5 align-icon" />
                {savingFile ? '保存中...' : '保存文件修改'}
              </button>
            </div>
          </div>
        }
      >
        <div className="space-y-4 text-xs">
          {/* 行内搜索 */}
          <div className="relative">
            <Search className="w-3.5 h-3.5 text-neutral-400 absolute left-3 top-1/2 -translate-y-1/2 align-icon" />
            <input
              type="text"
              value={searchLine}
              onChange={(e) => setSearchLine(e.target.value)}
              placeholder="在规则内容中搜索域名或 CIDR..."
              className="w-full min-h-[34px] pl-8 pr-3 text-xs bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 rounded-md text-neutral-900 dark:text-neutral-100 placeholder-neutral-400 focus:outline-none focus:border-cyan-500"
            />
          </div>

          {searchLine ? (
            <div className="space-y-1.5">
              <div className="text-[11px] text-neutral-500 font-mono">
                匹配到 {filteredLines.length} 行:
              </div>
              <div className="max-h-[520px] overflow-y-auto p-3 font-mono text-xs rounded-md bg-neutral-900 text-neutral-100 border border-neutral-800 space-y-1">
                {filteredLines.slice(0, 300).map((l, idx) => (
                  <div key={idx} className="hover:bg-neutral-800 px-1 py-0.5 rounded">
                    {l}
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div>
              <label className="block text-neutral-700 dark:text-neutral-300 font-medium mb-1">
                原始规则行 (支持自由编辑修改)
              </label>
              <textarea
                rows={20}
                value={fileContent}
                onChange={(e) => setFileContent(e.target.value)}
                className="w-full p-3 font-mono text-xs bg-neutral-900 text-neutral-100 border border-neutral-800 rounded-md focus:outline-none focus:border-cyan-500 leading-relaxed resize-y"
              />
            </div>
          )}
        </div>
      </Drawer>

      {snackbarMessage && (
        <Snackbar message={snackbarMessage} onClose={() => setSnackbarMessage(null)} />
      )}
    </div>
  );
};
