import React, { createContext, useCallback, useContext, useState } from 'react';

export type TxStatus = 'pending' | 'success' | 'failed';

export interface TxRecord {
  id: string;
  hash: string;
  status: TxStatus;
  description: string;
  timestamp: number;
  /** Raw XDR of the transaction envelope, available once the tx is submitted. */
  xdr?: string;
  /** Transaction fee in stroops (1 XLM = 10,000,000 stroops). */
  fee?: string;
}

interface TxHistoryContextType {
  records: TxRecord[];
  add: (description: string) => string;
  update: (id: string, status: TxStatus, hash: string, xdr?: string, fee?: string) => void;
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

  const update = useCallback(
    (id: string, status: TxStatus, hash: string, xdr?: string, fee?: string) => {
      setRecords(prev =>
        prev.map(r => (r.id === id ? { ...r, status, hash, xdr, fee } : r))
      );
    },
    []
  );

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
