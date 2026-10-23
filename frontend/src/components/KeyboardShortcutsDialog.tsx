import React from 'react';
import { ModalWrapper } from './ModalWrapper';

export interface KeyboardShortcutsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * Dialog displaying available keyboard shortcuts for power users.
 */
export function KeyboardShortcutsDialog({ isOpen, onClose }: KeyboardShortcutsDialogProps) {
  const shortcuts = [
    { key: '/', description: 'Focus search input' },
    { key: 'j', description: 'Move to next bounty' },
    { key: 'k', description: 'Move to previous bounty' },
    { key: '?', description: 'Show this help dialog' },
    { key: 'Esc', description: 'Close modal' },
  ];

  return (
    <ModalWrapper isOpen={isOpen} onClose={onClose}>
      <div className="bg-white rounded-lg shadow-lg p-6 max-w-md w-full mx-4">
        <h2 className="text-xl font-bold mb-4">Keyboard Shortcuts</h2>
        <div className="space-y-3">
          {shortcuts.map(({ key, description }) => (
            <div key={key} className="flex items-center justify-between">
              <span className="text-gray-700">{description}</span>
              <kbd className="bg-gray-100 border border-gray-300 rounded px-2 py-1 text-sm font-mono">
                {key}
              </kbd>
            </div>
          ))}
        </div>
        <button
          onClick={onClose}
          className="mt-6 w-full bg-blue-600 text-white py-2 rounded hover:bg-blue-700 transition-colors font-medium"
        >
          Close
        </button>
      </div>
    </ModalWrapper>
  );
}
