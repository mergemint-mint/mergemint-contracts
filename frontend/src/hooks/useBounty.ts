import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';

/**
 * Loads the verifier approvals recorded for a bounty. Used by VerifierPanel.
 * `approvals` is the list of verifier addresses that have already approved.
 */
export function useBounty(bountyId: string) {
  const [approvals, setApprovals] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const bounty = (await api.getBounty(bountyId)) as { approvals?: string[] };
      setApprovals(bounty.approvals ?? []);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [bountyId]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { approvals, loading, error, refetch };
}
