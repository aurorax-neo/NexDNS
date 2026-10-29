import React, { useEffect, useState } from 'react';
import {
  FileCode,
  RefreshCw,
  Search,
  Save,
  Edit3,
  CheckCircle2,
  Download,
} from 'lucide-react';
import { api, RuleFileInfo } from '../lib/api';
import { Drawer } from '../components/ui/Drawer';
import { Snackbar } from '../components/ui/Snackbar';

export const RoutingPage: React.FC = () => {
  const [ruleFiles, setRuleFiles] = useState<RuleFileInfo[]>([]);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [selectedFile, setSelectedFile] = useState<RuleFileInfo | null>(null);
  const [fileContent, setFileContent] = useState('');
  const [searchLine, setSearchLine] = useState('');
  const [savingFile, setSavingFile] = useState(false);
  const [snackbarMessage, setSnackbarMessage] = useState<string | null>(null);

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
      setUpdatingAll(false);
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

  const allLines = fileContent.split('\n');
  const filteredLines = searchLine
    ? allLines.filter((l) => l.toLowerCase().includes(searchLine.toLowerCase()))
    : allLines;

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 顶部控制横幅 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs">
        <div>
          <h3 className="text-base font-semibold text-neutral-900 dark:text-neutral-100 flex items-center gap-2">
            <FileCode className="w-4 h-4 text-cyan-600 dark:text-cyan-400 align-icon" />
            分流规则集与中国 IP CIDR 库
          </h3>
          <p className="text-xs text-neutral-500 dark:text-neutral-400 mt-1 max-w-xl">
            管理直连域名、代理域名、私有域及中国大陆 IP 段。支持从开源库全量同步、行级搜索、查看及在线编辑。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleUpdateAll}
            disabled={updatingAll}
            className="inline-flex items-center gap-2 min-h-[38px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md shadow-xs disabled:opacity-50 transition-colors"
          >
            <RefreshCw className={`w-3.5 h-3.5 align-icon ${updatingAll ? 'animate-spin' : ''}`} />
            {updatingAll ? '正在在线拉取中...' : '一键全量更新规则集'}
          </button>
        </div>
      </div>

      {/* 规则集列表卡片 */}
      <div className="rounded-lg bg-white dark:bg-surface-1 border border-neutral-200 dark:border-neutral-800 shadow-xs overflow-hidden">
        <div className="px-5 py-4 border-b border-neutral-200 dark:border-neutral-800 flex items-center justify-between">
          <h4 className="text-sm font-semibold text-neutral-900 dark:text-neutral-100">已加载规则文件</h4>
          <span className="text-xs text-neutral-500 font-mono">共 {ruleFiles.length} 个规则集</span>
        </div>
        <div className="divide-y divide-neutral-200 dark:divide-neutral-800/60">
          {ruleFiles.map((rf) => (
            <div
              key={rf.file_name}
              className="p-4 flex flex-wrap items-center justify-between gap-3 hover:bg-neutral-50 dark:hover:bg-surface-2/40 transition-colors"
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
                    <span className="px-1.5 py-0.5 text-[11px] font-mono rounded bg-neutral-100 dark:bg-surface-3 text-neutral-700 dark:text-neutral-300 shrink-0">
                      {rf.count.toLocaleString()} 条规则
                    </span>
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

              <div className="flex items-center gap-2 shrink-0">
                <button
                  type="button"
                  onClick={() => handleOpenFile(rf)}
                  className="min-h-[36px] px-3.5 flex items-center gap-1.5 text-xs text-neutral-700 dark:text-neutral-300 hover:text-neutral-900 dark:hover:text-neutral-100 rounded border border-neutral-300 dark:border-neutral-800 bg-white dark:bg-surface-2 transition-colors"
                  title="查看与编辑文件内容"
                >
                  <Edit3 className="w-3.5 h-3.5 align-icon" />
                  查看与编辑内容
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

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
