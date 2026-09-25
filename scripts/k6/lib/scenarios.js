// Scenario definitions and the request logic each one runs.
//
// Entry scripts (list.js, filter.js, stream.js, tx.js, mixed.js) pick the
// scenarios they need via `options(...)` and re-export the matching exec
// functions, so the same code path is measured standalone and under mixed load.

import http from "k6/http";
import { check, sleep } from "k6";
import { Counter, Rate, Trend } from "k6/metrics";
import {
  API_PREFIX,
  ASSIGNEES,
  BASE_URL,
  BOUNTY_IDS,
  DURATION,
  FILTER_RPS,
  LIST_RPS,
  PAGE_LIMIT,
  RAMP,
  STREAM_HOLD_SECONDS,
  STREAM_VUS,
  TX_RPS,
  thresholdsFor,
} from "./config.js";

const sseTimeToHeaders = new Trend("sse_time_to_headers", true);
const sseStreamOk = new Rate("sse_stream_ok");
const sseEvents = new Counter("sse_events_received");

function pick(list) {
  return list[Math.floor(Math.random() * list.length)];
}

function isJson(res) {
  try {
    res.json();
    return true;
  } catch {
    return false;
  }
}

// Open-model executor: requests keep arriving at the target rate even when the
// backend slows down, which is how real traffic behaves.
function arrivalRate(exec, rps) {
  return {
    executor: "ramping-arrival-rate",
    exec,
    startRate: 0,
    timeUnit: "1s",
    preAllocatedVUs: Math.max(5, rps * 2),
    maxVUs: Math.max(20, rps * 10),
    stages: [
      { target: rps, duration: RAMP },
      { target: rps, duration: DURATION },
      { target: 0, duration: "10s" },
    ],
  };
}

const SCENARIOS = {
  list: () => arrivalRate("list", LIST_RPS),
  filter: () => arrivalRate("filter", FILTER_RPS),
  tx: () => arrivalRate("tx", TX_RPS),
  // Closed model: a fixed population of long-lived subscribers.
  stream: () => ({
    executor: "ramping-vus",
    exec: "stream",
    startVUs: 0,
    stages: [
      { target: STREAM_VUS, duration: RAMP },
      { target: STREAM_VUS, duration: DURATION },
      { target: 0, duration: "10s" },
    ],
    gracefulRampDown: `${STREAM_HOLD_SECONDS + 5}s`,
  }),
};

export function options(names) {
  const scenarios = {};
  for (const name of names) scenarios[name] = SCENARIOS[name]();
  return {
    scenarios,
    thresholds: thresholdsFor(names),
    summaryTrendStats: ["avg", "med", "p(90)", "p(95)", "p(99)", "max"],
  };
}

// ── list: GET /bounties, following one cursor page when available ────────────

export function list() {
  const res = http.get(`${BASE_URL}${API_PREFIX}/bounties?limit=${PAGE_LIMIT}`, {
    tags: { name: "GET /bounties" },
  });
  check(res, {
    "list: status 200": (r) => r.status === 200,
    "list: JSON body": isJson,
  });

  const cursor = res.status === 200 && isJson(res) ? res.json("next_cursor") : null;
  if (cursor) {
    const next = http.get(
      `${BASE_URL}${API_PREFIX}/bounties?limit=${PAGE_LIMIT}&cursor=${encodeURIComponent(cursor)}`,
      { tags: { name: "GET /bounties?cursor" } },
    );
    check(next, { "list: next page 200": (r) => r.status === 200 });
  }
}

// ── filter: GET /bounties/assignee/{address} with varying page sizes ─────────

export function filter() {
  const address = pick(ASSIGNEES);
  const limit = pick([10, PAGE_LIMIT, 50, 100]);
  const res = http.get(`${BASE_URL}${API_PREFIX}/bounties/assignee/${address}?limit=${limit}`, {
    tags: { name: "GET /bounties/assignee/{address}" },
  });
  check(res, {
    "filter: status 200": (r) => r.status === 200,
    "filter: JSON body": isJson,
  });
}

// ── stream: GET /bounties/stream (SSE) ───────────────────────────────────────
//
// k6 has no native SSE client, so each iteration holds a plain GET open for
// STREAM_HOLD_SECONDS. A healthy stream never completes on its own, so the
// request ending in k6's "request timeout" (error code 1050) with bytes
// received is the success case. `timings.waiting` still records time to
// response headers. Plain http_req_* metrics are meaningless for this route,
// which is why its thresholds use the custom sse_* metrics instead.

const REQUEST_TIMEOUT = 1050;

export function stream() {
  const res = http.get(`${BASE_URL}${API_PREFIX}/bounties/stream`, {
    headers: { Accept: "text/event-stream", "Cache-Control": "no-cache" },
    timeout: `${STREAM_HOLD_SECONDS}s`,
    tags: { name: "GET /bounties/stream" },
  });

  const body = typeof res.body === "string" ? res.body : "";
  const heldOpen = res.error_code === REQUEST_TIMEOUT || res.status === 200;
  const gotBytes = body.length > 0;
  const ok = heldOpen && gotBytes;

  sseStreamOk.add(ok);
  if (gotBytes) sseTimeToHeaders.add(res.timings.waiting);
  sseEvents.add((body.match(/^event: bounty_updated$/gm) || []).length);

  check(res, {
    "stream: connection held open": () => heldOpen,
    "stream: received keep-alive or event": () => gotBytes,
  });

  if (!ok) sleep(1); // back off like a real EventSource client would
}

// ── tx: POST /tx/self-claim and /tx/resolve-dispute ─────────────────────────
//
// Without seeded bounties these return 404/400. Those are correct, handled
// responses, so they count as successes; only 5xx and transport errors count
// towards http_req_failed.

const TX_EXPECTED = http.expectedStatuses(200, 400, 404, 409, 422);

export function tx() {
  const bountyId = pick(BOUNTY_IDS);
  const params = {
    headers: { "Content-Type": "application/json" },
    responseCallback: TX_EXPECTED,
  };

  let res;
  if (Math.random() < 0.5) {
    res = http.post(
      `${BASE_URL}/tx/self-claim`,
      JSON.stringify({ bounty_id: bountyId, claimant: pick(ASSIGNEES) }),
      Object.assign({ tags: { name: "POST /tx/self-claim" } }, params),
    );
  } else {
    res = http.post(
      `${BASE_URL}/tx/resolve-dispute`,
      JSON.stringify({ bounty_id: bountyId, arbitrator: pick(ASSIGNEES), winner: pick(ASSIGNEES) }),
      Object.assign({ tags: { name: "POST /tx/resolve-dispute" } }, params),
    );
  }

  check(res, {
    "tx: handled (2xx/4xx, not 5xx)": (r) => r.status >= 200 && r.status < 500,
    "tx: JSON body": isJson,
    "tx: 200 includes xdr": (r) => r.status !== 200 || Boolean(r.json("xdr")),
  });
}
