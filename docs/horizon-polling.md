# Stellar Horizon Event Polling for MergeMint

This guide shows how to poll the Stellar network for MergeMint contract events using the Soroban RPC `getEvents` endpoint. It covers filtering by contract address, parsing event topics and data payloads, and handling pagination for continuous indexer operation.

See [event-schema.md](./event-schema.md) for the full event payload specification.

## Prerequisites

```bash
npm install @stellar/stellar-sdk
```

## Endpoint

MergeMint events are emitted as Soroban contract events. The correct endpoint is the Soroban RPC `getEvents` method, not the Horizon effects endpoint. Horizon does not index Soroban contract events.

```
POST https://soroban-testnet.stellar.org
Content-Type: application/json
{ "jsonrpc": "2.0", "id": 1, "method": "getEvents", "params": { ... } }
```

## Filtering for bounty_created events

```ts
import { SorobanRpc, xdr, scValToNative } from "@stellar/stellar-sdk";

const CONTRACT_ID = "CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const RPC_URL = "https://soroban-testnet.stellar.org";

const server = new SorobanRpc.Server(RPC_URL, { timeout: 30000 });

interface BountyCreatedEvent {
  bountyId: string;
  creator: string;
  rewardAmount: bigint;
  ledger: number;
  txHash: string;
}

async function fetchBountyCreatedEvents(
  startLedger: number
): Promise<BountyCreatedEvent[]> {
  const response = await server.getEvents({
    startLedger,
    filters: [
      {
        type: "contract",
        contractIds: [CONTRACT_ID],
        topics: [
          ["*", "*"], // topic1 = event name symbol, topic2 = creator address
        ],
      },
    ],
    limit: 100,
  });

  const events: BountyCreatedEvent[] = [];

  for (const event of response.events) {
    const topic0 = scValToNative(
      xdr.ScVal.fromXDR(event.topic[0], "base64")
    ) as string;

    if (topic0 !== "bounty_created") continue;

    const creator = scValToNative(
      xdr.ScVal.fromXDR(event.topic[1], "base64")
    ) as string;

    const data = scValToNative(
      xdr.ScVal.fromXDR(event.value, "base64")
    ) as [Buffer, string];

    const bountyId = Buffer.from(data[0]).toString("hex");
    const rewardAmount = BigInt(data[1]);

    events.push({
      bountyId,
      creator,
      rewardAmount,
      ledger: event.ledger,
      txHash: event.txHash,
    });
  }

  return events;
}
```

## Parsing all MergeMint events

```ts
type EventType =
  | "bounty_created"
  | "bounty_claimed"
  | "bounty_completed"
  | "bounty_disputed"
  | "bounty_cancelled"
  | "bounty_expired"
  | "reward_paid"
  | "approval_recorded"
  | "dispute_resolved";

interface ParsedEvent {
  type: EventType;
  ledger: number;
  txHash: string;
  data: Record<string, unknown>;
}

function parseEvent(event: SorobanRpc.Api.EventResponse): ParsedEvent | null {
  const topics = event.topic.map((t) =>
    scValToNative(xdr.ScVal.fromXDR(t, "base64"))
  );
  const value = scValToNative(xdr.ScVal.fromXDR(event.value, "base64"));

  const eventType = topics[0] as EventType;
  const actor = topics[1] as string;

  const base = { ledger: event.ledger, txHash: event.txHash };

  switch (eventType) {
    case "bounty_created": {
      const [bountyIdBuf, rewardAmount] = value as [Buffer, string];
      return {
        type: eventType,
        ...base,
        data: {
          bountyId: Buffer.from(bountyIdBuf).toString("hex"),
          creator: actor,
          rewardAmount: BigInt(rewardAmount),
        },
      };
    }

    case "bounty_claimed":
    case "bounty_completed":
    case "bounty_disputed":
    case "bounty_cancelled":
    case "bounty_expired": {
      return {
        type: eventType,
        ...base,
        data: {
          bountyId: Buffer.from(value as Buffer).toString("hex"),
          actor,
        },
      };
    }

    case "reward_paid": {
      const [bountyIdBuf, amount] = value as [Buffer, string];
      return {
        type: eventType,
        ...base,
        data: {
          bountyId: Buffer.from(bountyIdBuf).toString("hex"),
          contributor: actor,
          amount: BigInt(amount),
        },
      };
    }

    case "approval_recorded": {
      const [bountyIdBuf, currentCount] = value as [Buffer, number];
      return {
        type: eventType,
        ...base,
        data: {
          bountyId: Buffer.from(bountyIdBuf).toString("hex"),
          verifier: actor,
          approvalCount: currentCount,
        },
      };
    }

    case "dispute_resolved": {
      const [bountyIdBuf, resolution] = value as [Buffer, string];
      return {
        type: eventType,
        ...base,
        data: {
          bountyId: Buffer.from(bountyIdBuf).toString("hex"),
          arbitrator: actor,
          resolution,
        },
      };
    }

    default:
      return null;
  }
}
```

## Cursor-based pagination for continuous polling

The `getEvents` response includes a `cursor` on each event. Use the last cursor as the starting point for the next poll to avoid replaying events.

```ts
const POLL_INTERVAL_MS = 5000;

async function startIndexer(fromLedger: number) {
  let cursor: string | null = null;

  while (true) {
    const params: SorobanRpc.Server.GetEventsRequest = cursor
      ? { startLedger: fromLedger, filters: [buildFilter()], limit: 200 }
      : { startLedger: fromLedger, filters: [buildFilter()], limit: 200 };

    // When resuming from cursor, use the pagination cursor approach.
    // The Soroban RPC paginates using the `cursor` field within the request.
    const response = await server.getEvents(params);

    for (const raw of response.events) {
      const parsed = parseEvent(raw);
      if (parsed) {
        await handleEvent(parsed);
      }
      cursor = raw.pagingToken;
    }

    // Advance the start ledger to the latest seen to avoid refetching.
    if (response.events.length > 0) {
      fromLedger = response.events[response.events.length - 1].ledger + 1;
    }

    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }
}

function buildFilter(): SorobanRpc.Server.EventFilter {
  return {
    type: "contract",
    contractIds: [CONTRACT_ID],
  };
}

async function handleEvent(event: ParsedEvent) {
  console.log(`[${event.ledger}] ${event.type}`, event.data);
  // Persist to your database here.
}
```

## Finding the starting ledger

To begin indexing from contract deployment rather than ledger 0, fetch the current ledger and work backwards, or store the deployment ledger in your indexer config.

```ts
async function getLatestLedger(): Promise<number> {
  const latest = await server.getLatestLedger();
  return latest.sequence;
}
```

## Testnet contract address

Deploy the MergeMint contract to testnet using the instructions in [getting-started.md](./getting-started.md) and replace `CONTRACT_ID` above with the resulting contract address.

## Timeouts

Without an explicit timeout, a slow or unresponsive RPC node will cause `server.getEvents()` to hang indefinitely, blocking the entire indexer loop. Always pass a `timeout` (in milliseconds) to the `SorobanRpc.Server` constructor:

```ts
// 30-second timeout on every request — prevents the indexer loop from
// hanging when the RPC node is slow or temporarily unreachable.
const server = new SorobanRpc.Server(RPC_URL, { timeout: 30000 });
```

The `timeout` option applies to every HTTP request made by the server instance. If the RPC node does not respond within the configured window the SDK will reject the promise with a network error, which your retry/back-off logic can then handle rather than waiting forever.

For production deployments, pair the timeout with a catch-and-retry loop around `server.getEvents()`:

```ts
async function fetchWithRetry(
  params: SorobanRpc.Server.GetEventsRequest,
  retries = 3
) {
  for (let attempt = 1; attempt <= retries; attempt++) {
    try {
      return await server.getEvents(params);
    } catch (err) {
      if (attempt === retries) throw err;
      const backoff = attempt * 2000;
      console.warn(`RPC request failed (attempt ${attempt}), retrying in ${backoff}ms`, err);
      await new Promise((resolve) => setTimeout(resolve, backoff));
    }
  }
}
```

## Notes

- Soroban RPC is the correct endpoint for contract events. Horizon `/effects` does not include Soroban contract event data.
- Events are only available for a limited window of ledgers on shared RPC nodes. For production indexers, run your own RPC node or use an archival node.
- The `limit` parameter caps results per request. For high-throughput periods, iterate until `response.events.length < limit` before sleeping.
