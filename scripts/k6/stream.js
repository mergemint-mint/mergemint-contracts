// Standalone "stream" scenario. Usage: k6 run scripts/k6/stream.js (see README.md).
import { options as buildOptions } from "./lib/scenarios.js";

export { stream } from "./lib/scenarios.js";

export const options = buildOptions(["stream"]);
