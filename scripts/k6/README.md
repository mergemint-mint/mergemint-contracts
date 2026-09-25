# k6 load scenarios

[k6](https://grafana.com/docs/k6/latest/) scenarios for the MergeMint backend
(`mergemint-backend/`). They cover the four route groups that see real traffic.
You can run each one alone, or run all four at once as a mixed load.

| Scenario | Script      | Route(s)                                                  | Load model                                    |
| -------- | ----------- | --------------------------------------------------------- | --------------------------------------------- |
| list     | `list.js`   | `GET /api/v1/bounties?limit=N` (+ one `cursor` page)      | open, `LIST_RPS` req/s (default 30)           |
| filter   | `filter.js` | `GET /api/v1/bounties/assignee/{address}?limit=N`         | open, `FILTER_RPS` req/s (default 15)         |
| stream   | `stream.js` | `GET /api/v1/bounties/stream` (Server-Sent Events)        | closed, `STREAM_VUS` subscribers (default 25) |
| tx       | `tx.js`     | `POST /tx/self-claim`, `POST /tx/resolve-dispute` (50/50) | open, `TX_RPS` req/s (default 5)              |
| mixed    | `mixed.js`  | all of the above, concurrently                            | all of the above                              |

The default mix is roughly what we expect a busy frontend to generate. Reads
dominate. Filtered reads happen about half as often as plain lists. There are
some long-lived dashboard subscribers and only a trickle of transaction
requests.

## Running

Install k6 ([instructions](https://grafana.com/docs/k6/latest/set-up/install-k6/)),
or have Docker available. Then start the backend and run a scenario:

```bash
# Backend (listens on :8080)
(cd mergemint-backend && cargo run --release)

# Mixed load (default) against http://localhost:8080
scripts/k6/run.sh

# A single scenario, shorter, with a different target
BASE_URL=http://staging.example:8080 DURATION=30s scripts/k6/run.sh filter

# Extra arguments go straight to `k6 run`
scripts/k6/run.sh mixed --summary-export=k6-summary.json
```

The root `docker-compose.yml` also exposes the backend on `:8080`. At the time
of writing, though, its build contexts (`./backend`, `./frontend`) have no
Dockerfile, so run the backend directly as shown above until that is fixed.

`run.sh` uses a local `k6` binary if it finds one, and otherwise falls back to
the `grafana/k6` Docker image. You can also call k6 directly:
`k6 run -e LIST_RPS=100 scripts/k6/mixed.js`.

k6 exits with code **99** when any threshold fails, so the scripts can gate CI.

### Configuration

| Variable              | Default                 | Meaning                                                         |
| --------------------- | ----------------------- | --------------------------------------------------------------- |
| `BASE_URL`            | `http://localhost:8080` | Backend origin                                                  |
| `API_PREFIX`          | `/api/v1`               | Prefix for the bounty read routes                               |
| `DURATION`            | `2m`                    | Steady-state duration (after ramp-up)                           |
| `RAMP`                | `30s`                   | Ramp-up duration                                                |
| `LIST_RPS`            | `30`                    | list requests per second                                        |
| `FILTER_RPS`          | `15`                    | filter requests per second                                      |
| `TX_RPS`              | `5`                     | tx requests per second                                          |
| `STREAM_VUS`          | `25`                    | concurrent SSE subscribers                                      |
| `STREAM_HOLD_SECONDS` | `20`                    | how long each subscriber holds a connection before reconnecting |
| `PAGE_LIMIT`          | `20`                    | `limit` used by the list scenario                               |
| `ASSIGNEES`           | 3 sample G… addresses   | comma-separated addresses for filter/tx payloads                |
| `BOUNTY_IDS`          | 2 sample IDs            | comma-separated bounty IDs for tx payloads                      |

To exercise the success paths instead of the "not found" paths, set
`ASSIGNEES` and `BOUNTY_IDS` to records that exist in the backend you are
testing.

## Thresholds

Every scenario has its own thresholds, defined in `lib/config.js`. A scenario
only enforces its own thresholds, so a slow stream never fails the list run.

| Scenario | Metric                | Threshold                  | Why                                                                                         |
| -------- | --------------------- | -------------------------- | ------------------------------------------------------------------------------------------- |
| list     | `http_req_duration`   | p95 < 300 ms, p99 < 800 ms | Loads the landing page. It has to feel instant, and it is a simple indexed read.            |
| list     | `http_req_failed`     | < 1 %                      | Any non-2xx on a plain list is a bug.                                                       |
| filter   | `http_req_duration`   | p95 < 400 ms, p99 < 1 s    | Joins through the `assignees` table and allows `limit` up to 100, so it gets more headroom. |
| filter   | `http_req_failed`     | < 1 %                      | Unknown addresses return an empty page, not an error.                                       |
| stream   | `sse_time_to_headers` | p95 < 500 ms               | Time until the SSE response headers arrive, which is how fast subscribers get attached.     |
| stream   | `sse_stream_ok`       | > 99 %                     | The connection stayed open for the whole hold window and received at least a keep-alive.    |
| tx       | `http_req_duration`   | p95 < 500 ms, p99 < 1.5 s  | Builds XDR. Later this includes Horizon round-trips, so it gets the largest budget.         |
| tx       | `http_req_failed`     | < 1 %                      | Only 5xx and transport errors count. 400/404/409/422 are expected business outcomes.        |
| all      | `checks`              | > 99 %                     | Response-shape assertions (status, JSON body, `xdr` present on success).                    |

These are starting budgets for a single backend instance on a developer
machine. Tighten them once there is a baseline from the target environment,
and record the reasons in this table.

### Notes on the stream scenario

k6 has no built-in SSE client. Each stream iteration opens a normal
`GET` and holds it for `STREAM_HOLD_SECONDS`. When the hold window runs out,
k6 reports a `request timeout`, and for this route that is the **success**
case: it means the server kept the stream open. As a result:

- The global `http_req_failed` and `http_req_duration` values in the summary
  include these held-open requests. Read the per-scenario values
  (`{scenario:list}` etc.) and the `sse_*` metrics instead.
- `STREAM_HOLD_SECONDS` must be longer than the server keep-alive interval
  (axum default: 15 s). Otherwise an idle but healthy stream can close before
  it has received any bytes.
- `sse_events_received` counts `bounty_updated` events. It only goes above
  zero if something triggers bounty updates while the test runs.

## Current backend status

When these scenarios were written, `mergemint-backend/src/main.rs` mounted
only the `/tx/*` routes. The bounty read routes are implemented in
`routes/bounties.rs`, but they are not yet registered in `routes/mod.rs` or in
the router. Until they are wired up, **list, filter and stream fail their
thresholds with 404s**, and only `tx.js` passes against the real binary. The
scenarios target the documented API, so they will start passing without
changes once those routes are mounted.
