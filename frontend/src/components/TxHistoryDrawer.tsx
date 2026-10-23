import React, { useState } from 'react';
import { useTxHistory } from '../lib/TxHistoryContext';
import { CopyButton } from './CopyButton';
import './TxHistoryDrawer.css';

const STELLAR_EXPLORER = 'https://stellar.expert/explorer/testnet/tx/';

/** Format a fee expressed in stroops into a human-readable XLM string. */
function formatFee(feeStroops: string): string {
  const n = parseInt(feeStroops, 10);
  if (isNaN(n)) return feeStroops;
  return `${(n / 10_000_000).toFixed(7)} XLM`;
}

export function TxHistoryDrawer() {
  const { records } = useTxHistory();
  const [open, setOpen] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  function toggleDetails(id: string) {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

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
        <div className="tx-history-drawer" role="dialog" aria-label="Transaction history">
          <div className="drawer-header">
            <h2>Transaction History</h2>
            <button onClick={() => setOpen(false)} aria-label="Close transaction history">
              ✕
            </button>
          </div>

          <div className="drawer-content">
            {records.length === 0 ? (
              <p className="empty">No transactions yet</p>
            ) : (
              <ul className="tx-list">
                {records.map(tx => {
                  const isExpanded = expandedIds.has(tx.id);
                  const hasDetails = Boolean(tx.xdr || tx.fee || tx.hash);

                  return (
                    <li key={tx.id} className={`tx-item tx-${tx.status}`}>
                      <div className="tx-info">
                        <p className="tx-desc">{tx.description}</p>
                        <span className={`tx-status status-${tx.status}`}>
                          {tx.status}
                        </span>
                      </div>

                      <div className="tx-actions">
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

                        {hasDetails && (
                          <button
                            className="tx-expand-btn"
                            onClick={() => toggleDetails(tx.id)}
                            aria-expanded={isExpanded}
                            aria-label={
                              isExpanded ? 'Collapse transaction details' : 'Expand transaction details'
                            }
                          >
                            {isExpanded ? '▲ Details' : '▼ Details'}
                          </button>
                        )}
                      </div>

                      {isExpanded && (
                        <div className="tx-details">
                          {tx.hash && (
                            <div className="tx-detail-row">
                              <span className="tx-detail-label">Transaction hash</span>
                              <span className="tx-detail-value">
                                <span className="tx-hash-text">{tx.hash}</span>
                                <CopyButton value={tx.hash} />
                              </span>
                            </div>
                          )}

                          {tx.fee && (
                            <div className="tx-detail-row">
                              <span className="tx-detail-label">Fee</span>
                              <span className="tx-detail-value">{formatFee(tx.fee)}</span>
                            </div>
                          )}

                          {tx.xdr && (
                            <div className="tx-detail-row tx-detail-xdr">
                              <div className="tx-detail-label">
                                XDR
                                <CopyButton value={tx.xdr} />
                              </div>
                              <pre className="tx-xdr-value">{tx.xdr}</pre>
                            </div>
                          )}
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        </div>
      )}
    </>
  );
}
