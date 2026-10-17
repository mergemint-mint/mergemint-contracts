import { useEffect, useMemo, useState } from "react";
import { useTxFlow } from "../hooks/useTxFlow";
import { NetworkName } from "../lib/types";

interface SubmitWorkModalProps {
  open: boolean;
  onClose: () => void;
  network: NetworkName;
  bountyId: number | string;
  /** Only the current assignee may submit work. */
  isAssignee: boolean;
  /** Calls the `submit_work` entrypoint with the hashed PR URL. */
  onSubmitWork: (bountyId: number | string, prHash: string) => Promise<{ hash: string; ledger?: number }>;
  onSubmitted?: (prUrl: string) => void;
}

const PR_URL_PATTERN = /^https?:\/\/github\.com\/[^/\s]+\/[^/\s]+\/pull\/\d+\/?$/i;

export function isValidPrUrl(value: string): boolean {
  return PR_URL_PATTERN.test(value.trim());
}

/**
 * Hashes the PR URL with SHA-256 and returns the hex digest.
 * Uses the Web Crypto API available in all modern browsers.
 */
export async function hashPrUrl(prUrl: string): Promise<string> {
  const data = new TextEncoder().encode(prUrl.trim());
  const digest = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export default function SubmitWorkModal({
  open,
  onClose,
  network,
  bountyId,
  isAssignee,
  onSubmitWork,
  onSubmitted,
}: SubmitWorkModalProps) {
  const [prUrl, setPrUrl] = useState("");
  const [touched, setTouched] = useState(false);
  const { pending, error, result, run, retry } = useTxFlow(network);

  const trimmed = prUrl.trim();
  const valid = useMemo(() => isValidPrUrl(trimmed), [trimmed]);
  const showError = touched && trimmed.length > 0 && !valid;

  useEffect(() => {
    if (!open) {
      setPrUrl("");
      setTouched(false);
    }
  }, [open]);

  useEffect(() => {
    if (result && onSubmitted) {
      onSubmitted(trimmed);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result]);

  if (!open || !isAssignee) return null;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setTouched(true);
    if (!valid || pending) return;
    const prHash = await hashPrUrl(trimmed);
    try {
      await run(() => onSubmitWork(bountyId, prHash));
    } catch {
      // error state is surfaced via useTxFlow
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="submit-work-title"
      onClick={pending ? undefined : onClose}
    >
      <div
        className="w-full max-w-md rounded-xl bg-white p-6 shadow-xl transition-all"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="submit-work-title" className="text-lg font-semibold text-gray-900">
          Submit work
        </h2>
        <p className="mt-1 text-sm text-gray-500">
          Paste the link to your pull request so the creator can review it.
        </p>

        <form onSubmit={handleSubmit} className="mt-4 space-y-4">
          <div>
            <label htmlFor="pr-url" className="block text-sm font-medium text-gray-700">
              Pull request URL
            </label>
            <input
              id="pr-url"
              type="url"
              inputMode="url"
              placeholder="https://github.com/owner/repo/pull/123"
              value={prUrl}
              onChange={(e) => setPrUrl(e.target.value)}
              onBlur={() => setTouched(true)}
              disabled={pending}
              aria-invalid={showError}
              className={`mt-1 w-full rounded-lg border px-3 py-2 text-sm outline-none transition-colors disabled:bg-gray-100 ${
                showError
                  ? "border-red-500 focus:border-red-500"
                  : "border-gray-300 focus:border-indigo-500"
              }`}
            />
            {showError && (
              <p className="mt-1 text-xs text-red-600">
                Enter a valid GitHub pull request URL.
              </p>
            )}
          </div>

          {error && (
            <div className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
              {error}
            </div>
          )}

          {result && (
            <div className="rounded-lg bg-green-50 px-3 py-2 text-sm text-green-700">
              Work submitted. Tx: {result.hash.slice(0, 10)}…
            </div>
          )}

          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              disabled={pending}
              className="rounded-lg px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100 disabled:opacity-50"
            >
              Cancel
            </button>
            {error ? (
              <button
                type="button"
                onClick={() => retry()}
                disabled={pending}
                className="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
              >
                Retry
              </button>
            ) : (
              <button
                type="submit"
                disabled={pending || !valid}
                className="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
              >
                {pending ? "Submitting…" : "Submit work"}
              </button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
}
