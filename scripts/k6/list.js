// Standalone "list" scenario. Usage: k6 run scripts/k6/list.js (see README.md).
import { options as buildOptions } from "./lib/scenarios.js";

export { list } from "./lib/scenarios.js";

export const options = buildOptions(["list"]);
