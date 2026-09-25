// Shared configuration for the k6 load scenarios.
//
// Every value can be overridden with `-e NAME=value` on the k6 command line
// (or an environment variable when using scripts/k6/run.sh).

function envInt(name, fallback) {
  const raw = __ENV[name];
  if (raw === undefined || raw === "") return fallback;
  const n = parseInt(raw, 10);
  if (Number.isNaN(n)) throw new Error(`${name} must be an integer, got "${raw}"`);
  return n;
}

function envList(name, fallback) {
  const raw = __ENV[name];
  if (!raw) return fallback;
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}

export const BASE_URL = (__ENV.BASE_URL || "http://localhost:8080").replace(/\/$/, "");
export const API_PREFIX = __ENV.API_PREFIX || "/api/v1";

// Load shape. Rates are requests (iterations) per second at steady state.
export const DURATION = __ENV.DURATION || "2m";
export const RAMP = __ENV.RAMP || "30s";
export const LIST_RPS = envInt("LIST_RPS", 30);
export const FILTER_RPS = envInt("FILTER_RPS", 15);
export const TX_RPS = envInt("TX_RPS", 5);
export const STREAM_VUS = envInt("STREAM_VUS", 25);
// How long each SSE client holds its connection open before reconnecting.
// Must exceed the server keep-alive interval (axum default: 15s) so that every
// healthy connection receives at least one byte.
export const STREAM_HOLD_SECONDS = envInt("STREAM_HOLD_SECONDS", 20);
export const PAGE_LIMIT = envInt("PAGE_LIMIT", 20);

// Test data. Point these at seeded records to exercise the success paths; the
// defaults are syntactically valid Stellar addresses with no data behind them.
export const ASSIGNEES = envList("ASSIGNEES", [
  "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7",
  "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
  "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEYVXM3XOJMDS674JZ",
]);
export const BOUNTY_IDS = envList("BOUNTY_IDS", ["load-test-bounty-1", "load-test-bounty-2"]);

// ── Thresholds ───────────────────────────────────────────────────────────────
//
// Keyed by scenario name so each entry script only enforces the thresholds of
// the scenarios it runs. The rationale for every number is in README.md.
export const THRESHOLDS = {
  list: {
    "http_req_duration{scenario:list}": ["p(95)<300", "p(99)<800"],
    "http_req_failed{scenario:list}": ["rate<0.01"],
    "checks{scenario:list}": ["rate>0.99"],
  },
  filter: {
    "http_req_duration{scenario:filter}": ["p(95)<400", "p(99)<1000"],
    "http_req_failed{scenario:filter}": ["rate<0.01"],
    "checks{scenario:filter}": ["rate>0.99"],
  },
  stream: {
    sse_time_to_headers: ["p(95)<500"],
    sse_stream_ok: ["rate>0.99"],
    "checks{scenario:stream}": ["rate>0.99"],
  },
  tx: {
    "http_req_duration{scenario:tx}": ["p(95)<500", "p(99)<1500"],
    "http_req_failed{scenario:tx}": ["rate<0.01"],
    "checks{scenario:tx}": ["rate>0.99"],
  },
};

export function thresholdsFor(names) {
  return Object.assign({}, ...names.map((n) => THRESHOLDS[n]));
}
