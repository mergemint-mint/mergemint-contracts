/**
 * @file events.ts
 * Event subscription helper for the MergeMint Soroban contract.
 *
 * `onEvent` polls the Soroban RPC for contract events at a configurable
 * interval, decodes each raw XDR event into a strongly-typed {@link ContractEvent}
 * object, and invokes your callback.  It returns an `unsubscribe` function that
 * stops polling when called.
 *
 * @example
 * ```ts
 * import { onEvent } from "@mergemint/sdk/events";
 *
 * const unsub = onEvent(rpcUrl, contractId, "bounty_created", (event) => {
 *   console.log("New bounty:", event.data);
 * });
 *
 * // Later, stop polling:
 * unsub();
 * ```
 */

import { SorobanRpc, scValToNative, xdr } from "@stellar/stellar-sdk";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/** All event type names emitted by the MergeMint contract. */
export type ContractEventType =
  | "bounty_created"
  | "bounty_claimed"
  | "bounty_disputed"
  | "bounty_completed"
  | "reward_paid"
  | "bounty_cancelled"
  | "bounty_expired"
  | "approval_recorded"
  | "dispute_resolved"
  | "milestone_completed";

/**
 * A decoded contract event.  `T` narrows the `data` field for the specific
 * event type you are subscribing to.
 */
export interface ContractEvent<T = unknown> {
  /** The event type symbol, matching one of {@link ContractEventType}. */
  type: ContractEventType;
  /** The ledger sequence number the event was emitted in. */
  ledger: number;
  /** The transaction hash the event was emitted in. */
  txHash: string;
  /**
   * The first topic beyond the event-type symbol — typically a relevant
   * address (creator, contributor, verifier, …).
   */
  topic: string | null;
  /** The decoded event data payload. Shape depends on the event type. */
  data: T;
  /** The raw Soroban RPC event object, for advanced use / debugging. */
  raw: SorobanRpc.Api.EventResponse;
}

/** Typed data payloads for each event type. */
export interface BountyCreatedData {
  bountyId: string;
  rewardAmount: bigint;
}

export interface BountyClaimedData {
  bountyId: string;
}

export interface BountyDisputedData {
  bountyId: string;
}

export interface BountyCompletedData {
  bountyId: string;
}

export interface RewardPaidData {
  bountyId: string;
  amount: bigint;
}

export interface BountyCancelledData {
  bountyId: string;
}

export interface BountyExpiredData {
  bountyId: string;
}

export interface ApprovalRecordedData {
  bountyId: string;
  approvalCount: number;
}

export interface DisputeResolvedData {
  bountyId: string;
  resolution: string;
}

export interface MilestoneCompletedData {
  bountyId: string;
  amount: bigint;
}

/** Union of all concrete event data shapes. */
export type AnyContractEventData =
  | BountyCreatedData
  | BountyClaimedData
  | BountyDisputedData
  | BountyCompletedData
  | RewardPaidData
  | BountyCancelledData
  | BountyExpiredData
  | ApprovalRecordedData
  | DisputeResolvedData
  | MilestoneCompletedData;

/** Callback signature used by {@link onEvent}. */
export type EventCallback<T = unknown> = (event: ContractEvent<T>) => void;

/**
 * Options for the {@link onEvent} subscription.
 */
export interface OnEventOptions {
  /**
   * How often (in milliseconds) to poll the RPC for new events.
   * @default 5000
   */
  pollIntervalMs?: number;
  /**
   * Ledger sequence to start polling from.  Defaults to the latest ledger at
   * the time of the first poll.
   */
  startLedger?: number;
}

// ---------------------------------------------------------------------------
// Internal decoder helpers
// ---------------------------------------------------------------------------

/**
 * Safely decodes an XDR `ScVal` string to a native JS value, returning `null`
 * on failure.
 */
function safeScValToNative(xdrBase64: string): unknown {
  try {
    const scVal = xdr.ScVal.fromXDR(xdrBase64, "base64");
    return scValToNative(scVal);
  } catch {
    return null;
  }
}

/** Converts a native value that represents a BytesN<32> to a hex string. */
function toHexId(raw: unknown): string {
  if (raw instanceof Uint8Array || Buffer.isBuffer(raw)) {
    return Buffer.from(raw as Buffer).toString("hex");
  }
  return String(raw);
}

/**
 * Decodes the `data` XDR field of a raw Soroban event into the typed payload
 * for the given event type.
 */
function decodeEventData(
  type: ContractEventType,
  dataXdr: string | undefined
): AnyContractEventData {
  if (!dataXdr) {
    return {} as AnyContractEventData;
  }

  const native = safeScValToNative(dataXdr);

  switch (type) {
    case "bounty_created": {
      // Data: (bounty_id, reward_amount)
      const tuple = native as unknown[];
      return {
        bountyId: toHexId(tuple?.[0]),
        rewardAmount: BigInt(String(tuple?.[1] ?? 0)),
      } as BountyCreatedData;
    }

    case "bounty_claimed":
    case "bounty_disputed":
    case "bounty_completed":
    case "bounty_cancelled":
    case "bounty_expired": {
      // Data: bounty_id
      return { bountyId: toHexId(native) } as BountyClaimedData;
    }

    case "reward_paid": {
      // Data: (bounty_id, amount)
      const tuple = native as unknown[];
      return {
        bountyId: toHexId(tuple?.[0]),
        amount: BigInt(String(tuple?.[1] ?? 0)),
      } as RewardPaidData;
    }

    case "approval_recorded": {
      // Data: (bounty_id, approval_count)
      const tuple = native as unknown[];
      return {
        bountyId: toHexId(tuple?.[0]),
        approvalCount: Number(tuple?.[1] ?? 0),
      } as ApprovalRecordedData;
    }

    case "dispute_resolved": {
      // Data: (bounty_id, resolution)
      const tuple = native as unknown[];
      return {
        bountyId: toHexId(tuple?.[0]),
        resolution: String(tuple?.[1] ?? ""),
      } as DisputeResolvedData;
    }

    case "milestone_completed": {
      // Data: (bounty_id, amount)
      const tuple = native as unknown[];
      return {
        bountyId: toHexId(tuple?.[0]),
        amount: BigInt(String(tuple?.[1] ?? 0)),
      } as MilestoneCompletedData;
    }

    default:
      return {} as AnyContractEventData;
  }
}

/**
 * Attempts to decode the `topic` beyond the event-type symbol (index 1),
 * returning the address string or `null`.
 */
function decodeSecondTopic(topics: string[] | undefined): string | null {
  if (!topics || topics.length < 2) return null;
  try {
    const native = safeScValToNative(topics[1]);
    return native != null ? String(native) : null;
  } catch {
    return null;
  }
}

/**
 * Derives the event type from the first topic XDR string.  Returns `null` if
 * the topic cannot be decoded or is not a recognised event type symbol.
 */
function decodeEventType(topics: string[] | undefined): ContractEventType | null {
  if (!topics || topics.length === 0) return null;
  const native = safeScValToNative(topics[0]);
  const knownTypes = new Set<string>([
    "bounty_created",
    "bounty_claimed",
    "bounty_disputed",
    "bounty_completed",
    "reward_paid",
    "bounty_cancelled",
    "bounty_expired",
    "approval_recorded",
    "dispute_resolved",
    "milestone_completed",
  ]);
  if (typeof native === "string" && knownTypes.has(native)) {
    return native as ContractEventType;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Subscribes to a specific contract event type by polling the Soroban RPC.
 *
 * @param rpcUrl - Soroban RPC endpoint URL.
 * @param contractId - The deployed MergeMint contract address (`C...`).
 * @param eventType - Which event to subscribe to (e.g. `"bounty_created"`).
 * @param callback - Called with each decoded {@link ContractEvent} as it
 * arrives.  Invoked once per matching event per poll cycle.
 * @param options - Optional polling configuration; see {@link OnEventOptions}.
 * @returns An `unsubscribe` function.  Call it to stop polling.
 *
 * @example
 * ```ts
 * const unsub = onEvent(
 *   "https://soroban-testnet.stellar.org",
 *   "CABC...",
 *   "bounty_created",
 *   (ev) => console.log(ev.data.bountyId)
 * );
 * setTimeout(unsub, 60_000); // stop after 1 minute
 * ```
 */
export function onEvent<T = AnyContractEventData>(
  rpcUrl: string,
  contractId: string,
  eventType: ContractEventType,
  callback: EventCallback<T>,
  options: OnEventOptions = {}
): () => void {
  const { pollIntervalMs = 5000 } = options;

  const server = new SorobanRpc.Server(rpcUrl);
  let stopped = false;
  let startLedger = options.startLedger;
  let timeoutHandle: ReturnType<typeof setTimeout> | null = null;

  async function poll(): Promise<void> {
    if (stopped) return;

    try {
      // On the very first poll, resolve the current ledger as a start point.
      if (startLedger === undefined) {
        const health = await server.getLatestLedger();
        // Use the latest ledger minus a small buffer to avoid missing events
        // that were emitted just before we started.
        startLedger = Math.max(1, health.sequence - 1);
      }

      const response = await server.getEvents({
        startLedger,
        filters: [
          {
            type: "contract",
            contractIds: [contractId],
          },
        ],
        limit: 200,
      });

      let maxLedger = startLedger;

      for (const rawEvent of response.events) {
        if (rawEvent.type !== "contract") continue;

        const topics = rawEvent.topic as string[] | undefined;
        const type = decodeEventType(topics);
        if (type !== eventType) continue;

        const data = decodeEventData(type, rawEvent.value as string | undefined) as unknown as T;
        const topic = decodeSecondTopic(topics);

        const event: ContractEvent<T> = {
          type,
          ledger: rawEvent.ledger,
          txHash: rawEvent.txHash,
          topic,
          data,
          raw: rawEvent,
        };

        callback(event);

        if (rawEvent.ledger > maxLedger) {
          maxLedger = rawEvent.ledger;
        }
      }

      // Advance the cursor past the last seen ledger.
      startLedger = maxLedger + 1;
    } catch {
      // Swallow polling errors — the subscription stays alive and retries on
      // the next interval.  Callers who need error observability should wrap
      // in their own try/catch or use a monitoring layer.
    }

    if (!stopped) {
      timeoutHandle = setTimeout(() => void poll(), pollIntervalMs);
    }
  }

  // Kick off the first poll.
  timeoutHandle = setTimeout(() => void poll(), 0);

  return function unsubscribe(): void {
    stopped = true;
    if (timeoutHandle !== null) {
      clearTimeout(timeoutHandle);
      timeoutHandle = null;
    }
  };
}
