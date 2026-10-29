import React, { useEffect } from 'react';
import { RotateCcw } from 'lucide-react';

export interface SnackbarProps {
  message: string;
  onUndo?: () => void;
  onClose: () => void;
}

export const Snackbar: React.FC<SnackbarProps> = ({ message, onUndo, onClose }) => {
  useEffect(() => {
    const timer = setTimeout(() => {
      onClose();
    }, 6000);
    return () => clearTimeout(timer);
  }, [onClose]);

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 flex items-center gap-4 px-4 py-3 bg-neutral-900 border border-neutral-700 text-neutral-200 text-sm rounded-lg shadow-2xl animate-in fade-in-50 slide-in-from-bottom-5">
      <span>{message}</span>
      {onUndo && (
        <button
          type="button"
          onClick={() => {
            onUndo();
            onClose();
          }}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-semibold text-cyan-400 hover:text-cyan-300 hover:bg-cyan-950/40 rounded transition-colors"
        >
          <RotateCcw className="w-3.5 h-3.5 align-icon" />
          撤销
        </button>
      )}
    </div>
  );
};
