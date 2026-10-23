import { useEffect, useCallback } from 'react';

export interface KeyboardShortcutsConfig {
  onSearchFocus?: () => void;
  onNextCard?: () => void;
  onPreviousCard?: () => void;
  onShowHelp?: () => void;
}

/**
 * Hook to handle keyboard shortcuts for navigation and actions.
 * - `/` : Focus search input
 * - `j` : Move to next card (down)
 * - `k` : Move to previous card (up)
 * - `?` : Show keyboard shortcuts help dialog
 */
export function useKeyboardShortcuts({
  onSearchFocus,
  onNextCard,
  onPreviousCard,
  onShowHelp,
}: KeyboardShortcutsConfig) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Don't trigger shortcuts when typing in inputs
      const target = e.target as HTMLElement;
      const isInput =
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.contentEditable === 'true';

      if (isInput && e.key !== 'Escape') {
        return;
      }

      switch (e.key) {
        case '/':
          e.preventDefault();
          onSearchFocus?.();
          break;
        case 'j':
          e.preventDefault();
          onNextCard?.();
          break;
        case 'k':
          e.preventDefault();
          onPreviousCard?.();
          break;
        case '?':
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            onShowHelp?.();
          }
          break;
      }
    },
    [onSearchFocus, onNextCard, onPreviousCard, onShowHelp]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);
}
