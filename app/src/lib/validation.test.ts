// app/src/lib/validation.test.ts
//
// Asserts app/src/lib/validation.ts agrees with mergemint-backend's
// validation.rs on the reward-amount and description-length rules, using the
// shared fixture at fixtures/validation-parity.json (root of the repo).
//
// Run with: node --experimental-strip-types src/lib/validation.test.ts
// (wired up as the "test" script in app/package.json). No test framework
// dependency is introduced — Node's built-in TypeScript type-stripping and
// `node:assert` are sufficient for this small parity check.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  isValidRewardAmount,
  isValidDescriptionLength,
} from "./validation";

interface Case {
  value: string;
  valid: boolean;
  reason: string;
}

interface Fixture {
  rewardAmount: Case[];
  descriptionLength: Case[];
}

const here = path.dirname(fileURLToPath(import.meta.url));
const fixturePath = path.join(here, "../../../fixtures/validation-parity.json");
const fixture: Fixture = JSON.parse(readFileSync(fixturePath, "utf-8"));

let failures = 0;

function check(label: string, cases: Case[], validator: (value: string) => boolean) {
  for (const { value, valid, reason } of cases) {
    const actual = validator(value);
    try {
      assert.equal(
        actual,
        valid,
        `${label} ${JSON.stringify(value)} expected valid=${valid} (${reason}), frontend disagreed`
      );
    } catch (err) {
      failures += 1;
      console.error((err as Error).message);
    }
  }
}

check("reward amount", fixture.rewardAmount, isValidRewardAmount);
check("description", fixture.descriptionLength, isValidDescriptionLength);

if (failures > 0) {
  console.error(`${failures} validation-parity case(s) failed.`);
  process.exit(1);
}

console.log(
  `validation parity OK: ${fixture.rewardAmount.length} reward-amount case(s), ${fixture.descriptionLength.length} description-length case(s).`
);
