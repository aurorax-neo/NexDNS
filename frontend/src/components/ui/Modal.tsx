import React, { useEffect } from 'react';
import { X } from 'lucide-react';

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  size?: 'sm' | 'md' | 'lg';
  footer?: React.ReactNode;
}

export const Modal: React.FC<ModalProps> = ({
  isOpen,
  onClose,
  title,
  children,
  size = 'md',
  footer,
}) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    if (isOpen) {
      window.addEventListener('keydown', handleKeyDown);
    }
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  // 遵循规范：小档 400px，中档 520px，大档 720px
  const widthClass = {
    sm: 'max-w-[400px]',
    md: 'max-w-[520px]',
    lg: 'max-w-[720px]',
  }[size];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* 遮罩不可点击关闭，防止表单输入丢失 */}
      <div className="fixed inset-0 bg-black/60 backdrop-blur-xs transition-opacity" />

      <div
        className={`relative w-full ${widthClass} rounded-lg bg-surface-1 border border-neutral-800 shadow-2xl transition-all duration-200 z-10`}
      >
        <div className="flex items-center justify-between px-6 py-4 border-b border-neutral-800">
          <h3 className="text-base font-medium text-neutral-100">{title}</h3>
          <button
            type="button"
            onClick={onClose}
            className="min-w-[40px] min-h-[40px] flex items-center justify-center text-neutral-400 hover:text-neutral-100 rounded-md transition-colors"
            title="关闭"
          >
            <X className="w-5 h-5 align-icon" />
          </button>
        </div>

        <div className="p-6 max-h-[75vh] overflow-y-auto">{children}</div>

        {footer && (
          <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-neutral-800 bg-surface-2/40 rounded-b-lg">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
};
