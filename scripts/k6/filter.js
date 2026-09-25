// Standalone "filter" scenario. Usage: k6 run scripts/k6/filter.js (see README.md).
import { options as buildOptions } from "./lib/scenarios.js";

export { filter } from "./lib/scenarios.js";

export const options = buildOptions(["filter"]);
