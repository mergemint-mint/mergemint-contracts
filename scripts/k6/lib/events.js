// Helpers for producing and timing bounty events in the SSE scenarios.
//
// `triggerBountyEvent` calls POST /bounties/{id}/claim, which publishes the
// bounty id on the same broadcast channel the indexer writes to, so every
// connected GET /bounties/stream client receives a `bounty_updated` event.
// The id carries the send time (`k6sse-<epoch ms>-<vu>-<iter>`), so a client
// can work out delivery latency from the event alone. The trigger and the
// clients run in the same k6 process and share one clock, so no clock sync
// is needed.

import http from "k6/http";
import { API_PREFIX, BASE_URL } from "./config.js";

const ID_PREFIX = "k6sse";

/** Publish one bounty_updated event. Returns the k6 HTTP response. */
export function triggerBountyEvent(vu, iter) {
  const id = `${ID_PREFIX}-${Date.now()}-${vu}-${iter}`;
  return http.post(`${BASE_URL}${API_PREFIX}/bounties/${id}/claim`, null, {
    tags: { name: "POST /bounties/{id}/claim (trigger)" },
  });
}

/**
 * Send time (epoch ms) encoded in a `bounty_updated` event payload, or null
 * when the event was not produced by `triggerBountyEvent`.
 */
export function sentAtFromEventData(data) {
  let bountyId;
  try {
    bountyId = JSON.parse(data).bountyId;
  } catch {
    return null;
  }
  if (typeof bountyId !== "string" || !bountyId.startsWith(`${ID_PREFIX}-`)) return null;
  const sentAt = parseInt(bountyId.split("-")[1], 10);
  return Number.isNaN(sentAt) ? null : sentAt;
}
