import { useCallback, useState } from 'react';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  duration?: number;
}

let toastId = 0;

export function useToast() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const add = useCallback((message: string, type: ToastType = 'info', duration = 5000) => {
    const id = `toast-${++toastId}`;
    const toast: Toast = { id, message, type, duration };

    setToasts(prev => [...prev, toast]);

    if (duration > 0) {
      setTimeout(() => {
        remove(id);
      }, duration);
    }

    return id;
  }, []);

  const remove = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const success = useCallback((message: string, duration?: number) => {
    return add(message, 'success', duration);
  }, [add]);

  const error = useCallback((message: string, duration?: number) => {
    return add(message, 'error', duration);
  }, [add]);

  const info = useCallback((message: string, duration?: number) => {
    return add(message, 'info', duration);
  }, [add]);

  return { toasts, add, remove, success, error, info };
}
