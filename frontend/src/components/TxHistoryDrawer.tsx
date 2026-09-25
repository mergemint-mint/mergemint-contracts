import React, { useState } from 'react';
import { useTxHistory } from '../lib/TxHistoryContext';
import './TxHistoryDrawer.css';

const STELLAR_EXPLORER = 'https://stellar.expert/explorer/testnet/tx/';

export function TxHistoryDrawer() {
  const { records } = useTxHistory();
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        className="tx-history-btn"
        onClick={() => setOpen(!open)}
        aria-label={`Transaction history (${records.length} items)`}
      >
        History {records.length > 0 && <span className="badge">{records.length}</span>}
      </button>

      {open && (
        <div className="tx-history-drawer">
          <div className="drawer-header">
            <h2>Transaction History</h2>
            <button onClick={() => setOpen(false)}>✕</button>
          </div>

          <div className="drawer-content">
            {records.length === 0 ? (
              <p className="empty">No transactions yet</p>
            ) : (
              <ul className="tx-list">
                {records.map(tx => (
                  <li key={tx.id} className={`tx-item tx-${tx.status}`}>
                    <div className="tx-info">
                      <p className="tx-desc">{tx.description}</p>
                      <span className={`tx-status status-${tx.status}`}>
                        {tx.status}
                      </span>
                    </div>
                    {tx.hash && (
                      <a
                        href={`${STELLAR_EXPLORER}${tx.hash}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="tx-link"
                      >
                        View on Explorer ↗
                      </a>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </>
  );
}
