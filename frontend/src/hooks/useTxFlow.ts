import { useRef, useState } from "react";
import { SubmitResult, NetworkName } from "../lib/types";
import { generateIdempotencyKey } from "../lib/api";

interface TxFlowState {
  pending: boolean;
  error: string | null;
  result: SubmitResult | null;
  /** True while `result` is a locally-assumed value that hasn't been confirmed on-chain yet. */
  optimistic: boolean;
  /** Active idempotency key associated with the current or most recent submission attempt. */
  idempotencyKey?: string | null;
}

const IDLE_STATE: TxFlowState = {
  pending: false,
  error: null,
  result: null,
  optimistic: false,
  idempotencyKey: null,
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

type Submit = (context?: { idempotencyKey: string }) => Promise<{ hash: string; ledger?: number }>;

export function useTxFlow(network: NetworkName) {
  const [state, setState] = useState<TxFlowState>(IDLE_STATE);
  const inFlightRef = useRef(false);
  const [lastSubmit, setLastSubmit] = useState<{
    submit: Submit;
    optimisticResult?: Omit<SubmitResult, "network">;
    idempotencyKey: string;
  } | null>(null);

  async function run(
    submit: Submit,
    optimisticResult?: Omit<SubmitResult, "network">,
    existingIdempotencyKey?: string
  ) {
    if (inFlightRef.current) {
      throw new Error("Transaction submission already in progress");
    }
    inFlightRef.current = true;
    const idempotencyKey = existingIdempotencyKey ?? generateIdempotencyKey();
    setLastSubmit({ submit, optimisticResult, idempotencyKey });
    setState({ ...buildOptimisticState(network, optimisticResult), idempotencyKey });
    try {
      const { hash, ledger } = await submit({ idempotencyKey });
      const result: SubmitResult = { hash, network, ledger };
      setState({ ...buildConfirmedState(result), idempotencyKey });
      return result;
    } catch (err) {
      const message = err instanceof Error ? err.message : "Transaction failed";
      setState({ ...buildFailedState(message), idempotencyKey });
      throw err;
    } finally {
      inFlightRef.current = false;
    }
  }

  /** Re-invokes the most recent submit call, reusing its idempotency key. */
  async function retry() {
    if (!lastSubmit || inFlightRef.current) return undefined;
    return run(lastSubmit.submit, lastSubmit.optimisticResult, lastSubmit.idempotencyKey);
  }

  return { ...state, run, retry };
}
