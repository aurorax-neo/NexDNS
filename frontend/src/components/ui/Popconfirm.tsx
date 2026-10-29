import React, { useState, useRef, useEffect } from 'react';
import { AlertCircle } from 'lucide-react';

export interface PopconfirmProps {
  title: string;
  actionText: string;
  onConfirm: () => void;
  children: React.ReactElement<{ onClick?: (e: React.MouseEvent) => void }>;
}

export const Popconfirm: React.FC<PopconfirmProps> = ({
  title,
  actionText,
  onConfirm,
  children,
}) => {
  const [visible, setVisible] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setVisible(false);
      }
    };
    if (visible) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [visible]);

  return (
    <div className="relative inline-block" ref={containerRef}>
      {React.cloneElement(children, ({
        onClick: (e: React.MouseEvent) => {
          e.stopPropagation();
          setVisible(!visible);
        },
      }) as React.Attributes)}

      {visible && (
        <div className="absolute right-0 top-full mt-2 z-50 w-64 p-3 bg-surface-2 border border-neutral-700 rounded-lg shadow-xl text-left animate-in fade-in-50 zoom-in-95 duration-100">
          <div className="flex items-start gap-2 text-xs text-neutral-200 mb-3">
            <AlertCircle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5 align-icon" />
            <span>{title}</span>
          </div>
          <div className="flex items-center justify-end gap-2">
            <button
              type="button"
              onClick={() => setVisible(false)}
              className="min-h-[32px] px-2.5 text-xs text-neutral-400 hover:text-neutral-200 rounded border border-neutral-700 bg-surface-1"
            >
              取消
            </button>
            <button
              type="button"
              onClick={() => {
                onConfirm();
                setVisible(false);
              }}
              className="min-h-[32px] px-2.5 text-xs text-white bg-red-600 hover:bg-red-500 rounded font-medium"
            >
              {actionText}
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
