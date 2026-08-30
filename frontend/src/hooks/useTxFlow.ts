import { useState } from "react";
import { SubmitResult, NetworkName } from "../lib/types";

interface TxFlowState {
  pending: boolean;
  error: string | null;
  result: SubmitResult | null;
  /** True while `result` is a locally-assumed value that hasn't been confirmed on-chain yet. */
  optimistic: boolean;
}

const IDLE_STATE: TxFlowState = {
  pending: false,
  error: null,
  result: null,
  optimistic: false,
};

/**
 * State to show the instant the user submits, before on-chain confirmation lands.
 * Exported (alongside buildConfirmedState/buildFailedState) so the optimistic
 * update and rollback transitions can be unit tested without rendering the hook.
 */
export function buildOptimisticState(
  network: NetworkName,
  optimisticResult?: Omit<SubmitResult, "network">
): TxFlowState {
  return {
    pending: true,
    error: null,
    result: optimisticResult ? { ...optimisticResult, network } : null,
    optimistic: Boolean(optimisticResult),
  };
}

/** State to show once the on-chain confirmation succeeds. */
export function buildConfirmedState(result: SubmitResult): TxFlowState {
  return { pending: false, error: null, result, optimistic: false };
}

/** State to show when the transaction fails — rolls back any optimistic result. */
export function buildFailedState(message: string): TxFlowState {
  return { pending: false, error: message, result: null, optimistic: false };
}

type Submit = () => Promise<{ hash: string; ledger?: number }>;

export function useTxFlow(network: NetworkName) {
  const [state, setState] = useState<TxFlowState>(IDLE_STATE);
  const [lastSubmit, setLastSubmit] = useState<{
    submit: Submit;
    optimisticResult?: Omit<SubmitResult, "network">;
  } | null>(null);

  async function run(submit: Submit, optimisticResult?: Omit<SubmitResult, "network">) {
    setLastSubmit({ submit, optimisticResult });
    setState(buildOptimisticState(network, optimisticResult));
    try {
      const { hash, ledger } = await submit();
      const result: SubmitResult = { hash, network, ledger };
      setState(buildConfirmedState(result));
      return result;
    } catch (err) {
      const message = err instanceof Error ? err.message : "Transaction failed";
      setState(buildFailedState(message));
      throw err;
    }
  }

  /** Re-invokes the most recent submit call, e.g. from a "Retry" action after a failure. */
  async function retry() {
    if (!lastSubmit) return undefined;
    return run(lastSubmit.submit, lastSubmit.optimisticResult);
  }

  return { ...state, run, retry };
}
