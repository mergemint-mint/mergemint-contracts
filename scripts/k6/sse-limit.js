// SSE connection-cap scenario for GET /bounties/stream (#1118).
//
// Opens SSE_LIMIT_ATTEMPTS streams at once from this machine (one IP) and
// holds them open. With the backend's per-IP cap set to SSE_EXPECTED_CAP
// (SSE_MAX_CONNECTIONS_PER_IP, default 32), exactly that many streams should
// be accepted and the rest refused with 429 + Retry-After. Needs k6 with
// xk6-sse (scripts/k6/build-k6-sse.sh). Usage: scripts/k6/README.md.
import sse from "k6/x/sse";
import { check } from "k6";
import { Counter } from "k6/metrics";
import {
  API_PREFIX,
  BASE_URL,
  SSE_LIMIT_ATTEMPTS,
  SSE_LIMIT_HOLD_SECONDS,
  THRESHOLDS,
} from "./lib/config.js";

const accepted = new Counter("sse_limit_accepted");
const rejected = new Counter("sse_limit_rejected");
const unexpected = new Counter("sse_limit_unexpected_status");

export const options = {
  scenarios: {
    sse_limit: {
      executor: "per-vu-iterations",
      exec: "openAndHold",
      vus: SSE_LIMIT_ATTEMPTS,
      iterations: 1,
      maxDuration: `${SSE_LIMIT_HOLD_SECONDS + 60}s`,
    },
  },
  thresholds: THRESHOLDS.sseLimit,
};

export function openAndHold() {
  const deadline = Date.now() + SSE_LIMIT_HOLD_SECONDS * 1000;
  const res = sse.open(
    `${BASE_URL}${API_PREFIX}/bounties/stream`,
    {
      method: "GET",
      headers: { Accept: "text/event-stream" },
      // Keep-alives arrive every 15 s by default; allow time for one after the hold.
      timeout: `${SSE_LIMIT_HOLD_SECONDS + 20}s`,
      tags: { name: "GET /bounties/stream" },
    },
    (client) => {
      client.on("event", () => {
        if (Date.now() >= deadline) client.close();
      });
      client.on("error", () => client.close());
    },
  );

  const status = res ? res.status : 0;
  if (status === 200) accepted.add(1);
  else if (status === 429) rejected.add(1);
  else unexpected.add(1);

  check(res, {
    "limit: status is 200 or 429": (r) => r && (r.status === 200 || r.status === 429),
    "limit: 429 carries Retry-After": (r) =>
      !r || r.status !== 429 || Boolean(r.headers["Retry-After"]),
  });
}
