import React, { useState, useRef, useEffect, useLayoutEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
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
  const [coords, setCoords] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const updatePosition = useCallback(() => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const popWidth = popoverRef.current?.offsetWidth || 256;
    const popHeight = popoverRef.current?.offsetHeight || 110;

    let left = rect.right - popWidth;
    left = Math.max(8, Math.min(left, window.innerWidth - popWidth - 8));

    const spaceBelow = window.innerHeight - rect.bottom;
    let top = rect.bottom + 8;
    if (spaceBelow < popHeight + 12 && rect.top > popHeight + 12) {
      top = rect.top - popHeight - 8;
    }
    top = Math.max(8, Math.min(top, window.innerHeight - popHeight - 8));

    setCoords({ top, left });
  }, []);

  useLayoutEffect(() => {
    if (visible) {
      updatePosition();
    }
  }, [visible, updatePosition]);

  useEffect(() => {
    if (!visible) return;
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        containerRef.current?.contains(target) ||
        popoverRef.current?.contains(target)
      ) {
        return;
      }
      setVisible(false);
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setVisible(false);
      }
    };
    const handleScrollOrResize = () => {
      updatePosition();
    };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown);
    window.addEventListener('scroll', handleScrollOrResize, true);
    window.addEventListener('resize', handleScrollOrResize);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('scroll', handleScrollOrResize, true);
      window.removeEventListener('resize', handleScrollOrResize);
    };
  }, [visible, updatePosition]);

  return (
    <div className="relative inline-block" ref={containerRef}>
      {React.cloneElement(children, ({
        onClick: (e: React.MouseEvent) => {
          e.stopPropagation();
          setVisible(!visible);
        },
      }) as React.Attributes)}

      {visible &&
        createPortal(
          <div
            ref={popoverRef}
            style={{ top: `${coords.top}px`, left: `${coords.left}px` }}
            onClick={(e) => e.stopPropagation()}
            className="fixed z-[120] w-64 p-3.5 bg-white dark:bg-surface-2 border border-neutral-200 dark:border-neutral-700 rounded-lg shadow-2xl text-left animate-in fade-in-50 zoom-in-95 duration-100"
          >
            <div className="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 mb-3 leading-relaxed">
              <AlertCircle className="w-4 h-4 text-amber-500 dark:text-amber-400 shrink-0 mt-0.5 align-icon" />
              <span>{title}</span>
            </div>
            <div className="flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => setVisible(false)}
                className="min-h-[30px] px-2.5 text-xs text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-200 rounded border border-neutral-300 dark:border-neutral-700 bg-neutral-100 dark:bg-surface-1 transition-colors"
              >
                取消
              </button>
              <button
                type="button"
                onClick={() => {
                  onConfirm();
                  setVisible(false);
                }}
                className="min-h-[30px] px-2.5 text-xs text-white bg-red-600 hover:bg-red-500 rounded font-medium transition-colors shadow-2xs"
              >
                {actionText}
              </button>
            </div>
          </div>,
          document.body
        )}
    </div>
  );
};
