import React from 'react';
import { AlertTriangle } from 'lucide-react';

export interface AlertDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  description: string;
  confirmText: string;
}

export const AlertDialog: React.FC<AlertDialogProps> = ({
  isOpen,
  onClose,
  onConfirm,
  title,
  description,
  confirmText,
}) => {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* 遮罩不可点击关闭，禁用 ESC 关闭 */}
      <div className="fixed inset-0 bg-black/75 backdrop-blur-sm" />

      <div className="relative w-full max-w-[440px] rounded-lg bg-surface-1 border border-red-500/30 shadow-2xl p-6 z-10">
        <div className="flex items-start gap-4">
          <div className="p-3 bg-red-500/10 rounded-full text-red-400 shrink-0">
            <AlertTriangle className="w-6 h-6 align-icon" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-neutral-100">{title}</h3>
            <p className="text-sm text-neutral-400 mt-2 leading-relaxed">{description}</p>
          </div>
        </div>

        <div className="flex items-center justify-end gap-3 mt-6 pt-4 border-t border-neutral-800">
          <button
            type="button"
            onClick={onClose}
            className="min-h-[40px] px-4 text-sm font-medium text-neutral-300 hover:text-neutral-100 bg-surface-2 border border-neutral-700 rounded-md transition-colors"
          >
            取消
          </button>
          <button
            type="button"
            onClick={() => {
              onConfirm();
              onClose();
            }}
            className="min-h-[40px] px-4 text-sm font-medium text-white bg-red-600 hover:bg-red-500 rounded-md transition-colors shadow-lg shadow-red-950/50"
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
};
