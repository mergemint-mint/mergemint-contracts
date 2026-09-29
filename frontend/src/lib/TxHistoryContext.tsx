import React, { createContext, useCallback, useContext, useState } from 'react';

export type TxStatus = 'pending' | 'success' | 'failed';

export interface TxRecord {
  id: string;
  hash: string;
  status: TxStatus;
  description: string;
  timestamp: number;
}

interface TxHistoryContextType {
  records: TxRecord[];
  add: (description: string) => string;
  update: (id: string, status: TxStatus, hash: string) => void;
  clear: () => void;
}

const TxHistoryContext = createContext<TxHistoryContextType | null>(null);

export function TxHistoryProvider({ children }: { children: React.ReactNode }) {
  const [records, setRecords] = useState<TxRecord[]>([]);
  let txId = 0;

  const add = useCallback((description: string) => {
    const id = `tx-${++txId}`;
    const record: TxRecord = {
      id,
      hash: '',
      status: 'pending',
      description,
      timestamp: Date.now(),
    };
    setRecords(prev => [record, ...prev]);
    return id;
  }, []);

  const update = useCallback((id: string, status: TxStatus, hash: string) => {
    setRecords(prev => prev.map(r => r.id === id ? { ...r, status, hash } : r));
  }, []);

  const clear = useCallback(() => {
    setRecords([]);
  }, []);

  return (
    <TxHistoryContext.Provider value={{ records, add, update, clear }}>
      {children}
    </TxHistoryContext.Provider>
  );
}

export function useTxHistory() {
  const context = useContext(TxHistoryContext);
  if (!context) {
    throw new Error('useTxHistory must be used within TxHistoryProvider');
  }
  return context;
}
