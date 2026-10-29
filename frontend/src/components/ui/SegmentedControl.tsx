import React from 'react';

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  icon?: React.ReactNode;
}

export interface SegmentedControlProps<T extends string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
}

export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  className = '',
}: SegmentedControlProps<T>) {
  return (
    <div
      className={`inline-flex items-center p-1 bg-surface-2 border border-neutral-800 rounded-lg ${className}`}
    >
      {options.map((opt) => {
        const isActive = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            className={`inline-flex items-center gap-1.5 min-h-[32px] px-3 text-xs font-medium rounded-md transition-all ${
              isActive
                ? 'bg-neutral-800 text-neutral-100 shadow-sm border border-neutral-700/60'
                : 'text-neutral-400 hover:text-neutral-200 hover:bg-surface-3/50'
            }`}
          >
            {opt.icon}
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
