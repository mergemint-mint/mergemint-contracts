import React, { useCallback, useEffect, useRef } from 'react';

export interface ModalWrapperProps {
  isOpen: boolean;
  onClose: () => void;
  children: React.ReactNode;
  className?: string;
  backdropClassName?: string;
}

/**
 * Modal wrapper with focus trapping and Escape key handling.
 * Traps focus within the modal and restores focus to the trigger element when closed.
 */
export function ModalWrapper({
  isOpen,
  onClose,
  children,
  className = 'fixed inset-0 z-50 flex items-center justify-center',
  backdropClassName = 'fixed inset-0 bg-black/50',
}: ModalWrapperProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLElement | null>(null);

  // Store the element that triggered the modal so we can restore focus when it closes
  useEffect(() => {
    if (isOpen) {
      triggerRef.current = document.activeElement as HTMLElement;
    }
  }, [isOpen]);

  // Handle Escape key and focus trapping
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
        return;
      }

      // Focus trap: keep focus within modal
      if (e.key === 'Tab' && modalRef.current) {
        const focusableElements = modalRef.current.querySelectorAll(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        const firstElement = focusableElements[0] as HTMLElement;
        const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

        if (e.shiftKey) {
          // Shift+Tab on first element: move to last
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement?.focus();
          }
        } else {
          // Tab on last element: move to first
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement?.focus();
          }
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);

    // Focus the first focusable element in the modal
    const focusableElement = modalRef.current?.querySelector(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    ) as HTMLElement;
    focusableElement?.focus();

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, onClose]);

  // Restore focus when modal closes
  useEffect(() => {
    return () => {
      if (!isOpen && triggerRef.current) {
        triggerRef.current.focus();
      }
    };
  }, [isOpen]);

  if (!isOpen) return null;

  return (
    <>
      <div className={backdropClassName} onClick={onClose} />
      <div ref={modalRef} className={className} role="dialog" aria-modal="true">
        {children}
      </div>
    </>
  );
}
