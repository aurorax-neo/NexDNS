import React, { useEffect } from 'react';
import { X } from 'lucide-react';

export interface DrawerProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  children: React.ReactNode;
  width?: string;
  footer?: React.ReactNode;
}

export const Drawer: React.FC<DrawerProps> = ({
  isOpen,
  onClose,
  title,
  subtitle,
  children,
  width = 'w-[560px]',
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

  return (
    <div className="fixed inset-0 z-50 overflow-hidden">
      {/* 半透明遮罩，保留背景列表上下文 */}
      <div
        className="fixed inset-0 bg-black/40 backdrop-blur-[2px] transition-opacity"
        onClick={onClose}
      />

      <div className="fixed inset-y-0 right-0 flex max-w-full pl-10 pointer-events-none">
        <div
          className={`pointer-events-auto ${width} max-w-screen-md bg-surface-1 border-l border-neutral-800 shadow-2xl flex flex-col`}
        >
          <div className="flex items-start justify-between px-6 py-4 border-b border-neutral-800 bg-surface-2/30">
            <div>
              <h3 className="text-base font-medium text-neutral-100">{title}</h3>
              {subtitle && <p className="text-xs text-neutral-400 mt-1">{subtitle}</p>}
            </div>
            <button
              type="button"
              onClick={onClose}
              className="min-w-[40px] min-h-[40px] flex items-center justify-center text-neutral-400 hover:text-neutral-100 rounded-md transition-colors"
              title="关闭"
            >
              <X className="w-5 h-5 align-icon" />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto p-6">{children}</div>

          {footer && (
            <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-neutral-800 bg-surface-2/40">
              {footer}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
