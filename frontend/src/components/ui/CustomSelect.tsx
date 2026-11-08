import React, { useState, useRef, useEffect, useLayoutEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, Check } from 'lucide-react';

export interface SelectOption {
  value: string;
  label: string;
  badge?: React.ReactNode;
}

export interface CustomSelectProps {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  direction?: 'up' | 'down' | 'auto';
}

export const CustomSelect: React.FC<CustomSelectProps> = ({
  options,
  value,
  onChange,
  placeholder = '请选择',
  className = '',
  direction = 'auto',
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({});
  const containerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const selectedOption = options.find((o) => o.value === value);

  const updatePosition = useCallback(() => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const estimatedHeight = dropdownRef.current?.offsetHeight || Math.min(264, options.length * 36 + 10);
    const spaceBelow = window.innerHeight - rect.bottom;
    const openUp =
      direction === 'up' ||
      (spaceBelow < estimatedHeight + 12 && rect.top > spaceBelow);

    const width = Math.max(rect.width, 140);
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));

    if (openUp) {
      setDropdownStyle({
        position: 'fixed',
        left: `${left}px`,
        bottom: `${Math.max(8, window.innerHeight - rect.top + 6)}px`,
        minWidth: `${rect.width}px`,
        maxWidth: `${Math.max(rect.width, 320)}px`,
      });
    } else {
      setDropdownStyle({
        position: 'fixed',
        left: `${left}px`,
        top: `${rect.bottom + 6}px`,
        minWidth: `${rect.width}px`,
        maxWidth: `${Math.max(rect.width, 320)}px`,
      });
    }
  }, [direction, options.length]);

  useLayoutEffect(() => {
    if (isOpen) {
      updatePosition();
    }
  }, [isOpen, updatePosition]);

  useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        containerRef.current?.contains(target) ||
        dropdownRef.current?.contains(target)
      ) {
        return;
      }
      setIsOpen(false);
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false);
      }
    };
    const handleScroll = (e: Event) => {
      if (dropdownRef.current && dropdownRef.current.contains(e.target as Node)) {
        return;
      }
      updatePosition();
    };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown);
    window.addEventListener('scroll', handleScroll, true);
    window.addEventListener('resize', updatePosition);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('scroll', handleScroll, true);
      window.removeEventListener('resize', updatePosition);
    };
  }, [isOpen, updatePosition]);

  return (
    <div className={`relative inline-block ${className}`} ref={containerRef}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="w-full min-h-[40px] px-3 py-1.5 flex items-center justify-between gap-2 text-xs rounded-md bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700/80 text-neutral-800 dark:text-neutral-200 hover:border-neutral-400 dark:hover:border-neutral-600 focus:outline-none transition-colors shadow-xs"
      >
        <div className="flex items-center gap-1.5 truncate">
          {selectedOption?.badge}
          <span className="truncate">{selectedOption ? selectedOption.label : placeholder}</span>
        </div>
        <ChevronDown
          className={`w-3.5 h-3.5 text-neutral-400 transition-transform duration-150 align-icon shrink-0 ${
            isOpen ? 'rotate-180' : ''
          }`}
        />
      </button>

      {isOpen &&
        createPortal(
          <div
            ref={dropdownRef}
            style={dropdownStyle}
            className="z-[120] max-h-[264px] overflow-y-auto rounded-md bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 shadow-2xl py-1 animate-in fade-in-50 zoom-in-95 duration-100"
          >
            {options.map((opt) => {
              const isSelected = opt.value === value;
              return (
                <div
                  key={opt.value}
                  onClick={() => {
                    onChange(opt.value);
                    setIsOpen(false);
                  }}
                  className={`px-3 py-2 text-xs flex items-center justify-between gap-2 cursor-pointer transition-colors ${
                    isSelected
                      ? 'bg-neutral-100 dark:bg-neutral-800 text-cyan-600 dark:text-cyan-400 font-medium'
                      : 'text-neutral-700 dark:text-neutral-300 hover:bg-neutral-50 dark:hover:bg-surface-3/60'
                  }`}
                >
                  <div className="flex items-center gap-2 truncate">
                    {opt.badge}
                    <span className="truncate">{opt.label}</span>
                  </div>
                  {isSelected && <Check className="w-3.5 h-3.5 align-icon shrink-0" />}
                </div>
              );
            })}
          </div>,
          document.body
        )}
    </div>
  );
};
