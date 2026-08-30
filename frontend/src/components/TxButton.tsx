import React from 'react';

interface TxButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  pending: boolean;
  pendingLabel: string;
  children: React.ReactNode;
}

/**
 * Button used for claim/complete/create actions driven by useTxFlow.ts.
 * Applies an explicit visual "pending" style (not just the native disabled
 * state) and aria-busy so screen readers and sighted users alike can tell a
 * transaction is in flight and avoid double-clicking submit.
 */
export function TxButton({ pending, pendingLabel, children, className, style, disabled, ...rest }: TxButtonProps) {
  return (
    <button
      {...rest}
      disabled={disabled || pending}
      aria-busy={pending}
      className={[className, 'tx-button', pending ? 'tx-button--pending' : ''].filter(Boolean).join(' ')}
      style={{
        ...style,
        ...(pending
          ? {
              opacity: 0.6,
              cursor: 'not-allowed',
              filter: 'grayscale(30%)',
            }
          : undefined),
      }}
    >
      {pending ? pendingLabel : children}
    </button>
  );
}
