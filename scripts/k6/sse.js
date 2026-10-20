// SSE load scenario for GET /bounties/stream (#1120).
//
// Ramps up to SSE_CLIENTS long-lived stream subscribers. While they are all
// connected, a trigger scenario publishes TRIGGER_RPS bounty events per second.
// The script measures:
//
//   sse_event_latency        ms from publishing an event to a client receiving it
//   sse_connect_time         ms until the stream's response headers arrive
//   sse_events_delivered     bounty_updated events received across all clients
//   sse_dropped_connections  streams that ended before their hold window
//   sse_rejected_connections streams refused with 429 (connection caps, #1118)
//   sse_connection_ok        share of streams that stayed open the full window
//
// Needs a k6 binary with the xk6-sse extension: build one with
// scripts/k6/build-k6-sse.sh. Usage and baseline numbers:
// scripts/k6/README.md and docs/benchmarks.md.
import sse from "k6/x/sse";
import { check } from "k6";
import exec from "k6/execution";
import { Counter, Rate, Trend } from "k6/metrics";
import {
  API_PREFIX,
  BASE_URL,
  DURATION,
  RAMP,
  SSE_CLIENTS,
  SSE_HOLD_SECONDS,
  TRIGGER_RPS,
  THRESHOLDS,
} from "./lib/config.js";
import { sentAtFromEventData, triggerBountyEvent } from "./lib/events.js";

const eventLatency = new Trend("sse_event_latency", true);
const connectTime = new Trend("sse_connect_time", true);
const eventsDelivered = new Counter("sse_events_delivered");
const eventsTriggered = new Counter("sse_events_triggered");
const droppedConnections = new Counter("sse_dropped_connections");
const rejectedConnections = new Counter("sse_rejected_connections");
const connectionOk = new Rate("sse_connection_ok");

// Extra time on top of the hold window before the HTTP client gives up. The
// stream is normally closed on the first event or keep-alive after the
// window ends, and the server sends a keep-alive every 15 s by default.
const CLOSE_GRACE_SECONDS = 20;

export const options = {
  scenarios: {
    sse_clients: {
      executor: "ramping-vus",
      exec: "sseClient",
      startVUs: 0,
      stages: [
        { target: SSE_CLIENTS, duration: RAMP },
        { target: SSE_CLIENTS, duration: DURATION },
        { target: 0, duration: "10s" },
      ],
      gracefulRampDown: `${SSE_HOLD_SECONDS + CLOSE_GRACE_SECONDS}s`,
    },
    // Starts once the ramp-up is done, so latency is measured at full fan-out.
    sse_trigger: {
      executor: "constant-arrival-rate",
      exec: "trigger",
      rate: TRIGGER_RPS,
      timeUnit: "1s",
      duration: DURATION,
      startTime: RAMP,
      preAllocatedVUs: 2,
      maxVUs: 20,
    },
  },
  thresholds: THRESHOLDS.sse,
  summaryTrendStats: ["avg", "med", "p(90)", "p(95)", "p(99)", "max"],
};

export function sseClient() {
  const started = Date.now();
  const deadline = started + SSE_HOLD_SECONDS * 1000;
  let closedByUs = false;
  let streamError = null;

  const res = sse.open(
    `${BASE_URL}${API_PREFIX}/bounties/stream`,
    {
      method: "GET",
      headers: { Accept: "text/event-stream", "Cache-Control": "no-cache" },
      timeout: `${SSE_HOLD_SECONDS + CLOSE_GRACE_SECONDS}s`,
      tags: { name: "GET /bounties/stream" },
    },
    (client) => {
      client.on("open", () => {
        connectTime.add(Date.now() - started);
      });

      // Keep-alive comments also arrive here (empty name), which lets idle
      // streams notice the end of their hold window.
      client.on("event", (event) => {
        if (event.name === "bounty_updated") {
          eventsDelivered.add(1);
          const sentAt = sentAtFromEventData(event.data);
          if (sentAt !== null) eventLatency.add(Date.now() - sentAt);
        }
        if (Date.now() >= deadline) {
          closedByUs = true;
          client.close();
        }
      });

      // The extension stops reading after an error, so close explicitly.
      client.on("error", (e) => {
        if (!closedByUs) streamError = typeof e.error === "function" ? e.error() : String(e);
        client.close();
      });
    },
  );

  const status = res ? res.status : 0;
  if (status === 429) {
    rejectedConnections.add(1);
  }
  const ok = status === 200 && closedByUs && streamError === null;
  if (status === 200 && !ok) {
    droppedConnections.add(1);
    console.warn(
      `stream dropped after ${Date.now() - started} ms: ${streamError || "server closed the stream"}`,
    );
  }
  connectionOk.add(ok);

  check(res, {
    "sse: status 200": (r) => r && r.status === 200,
    "sse: held for the full window": () => ok,
  });
}

export function trigger() {
  const res = triggerBountyEvent(exec.vu.idInTest, exec.scenario.iterationInTest);
  eventsTriggered.add(1);
  check(res, { "trigger: status 200": (r) => r.status === 200 });
}
