import React, { createContext, useContext } from 'react';
import { useToast, ToastType } from '../hooks/useToast';
import './Toaster.css';

const ToastContext = createContext<ReturnType<typeof useToast> | null>(null);

export function ToasterProvider({ children }: { children: React.ReactNode }) {
  const toast = useToast();
  return (
    <ToastContext.Provider value={toast}>
      {children}
      <Toaster />
    </ToastContext.Provider>
  );
}

export function useToastContext() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error('useToastContext must be used within ToasterProvider');
  }
  return context;
}

function Toaster() {
  const { toasts, remove } = useToast();

  if (toasts.length === 0) return null;

  return (
    <div className="toaster" role="region" aria-live="polite" aria-label="Notifications">
      {toasts.map(toast => (
        <div
          key={toast.id}
          className={`toast toast-${toast.type}`}
          role="status"
        >
          <span>{toast.message}</span>
          <button
            onClick={() => remove(toast.id)}
            className="toast-close"
            aria-label="Dismiss"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
