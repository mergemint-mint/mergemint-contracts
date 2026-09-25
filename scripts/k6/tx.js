// Standalone "tx" scenario. Usage: k6 run scripts/k6/tx.js (see README.md).
import { options as buildOptions } from "./lib/scenarios.js";

export { tx } from "./lib/scenarios.js";

export const options = buildOptions(["tx"]);
