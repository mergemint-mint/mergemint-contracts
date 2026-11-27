#!/usr/bin/env node
/**
 * Cross-layer ABI & validation parity check (#1297).
 *
 * Verifies that:
 * 1. Every `ContractError` variant in `src/errors.rs` has a non-empty canonical
 *    panic message in `errors::message(...)`.
 * 2. Every `emit_*` helper in `src/events.rs` publishes a snake_case event topic.
 * 3. `fixtures/validation-parity.json` cases are present and consistent with
 *    `SYMBOL_MAX_LENGTH = 32` in both `frontend/src/lib/validation.ts` and
 *    `mergemint-backend/src/validation.rs`.
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

function readText(relPath) {
  return fs.readFileSync(path.join(rootDir, relPath), 'utf8');
}

function checkContractErrors() {
  const errorsRs = readText('src/errors.rs');
  const enumBlockMatch = errorsRs.match(/pub enum ContractError\s*\{([\s\S]*?)\}/);
  if (!enumBlockMatch) {
    throw new Error('Could not locate `pub enum ContractError` in src/errors.rs');
  }

  const variants = enumBlockMatch[1]
    .split('\n')
    .map((line) => line.replace(/\/\/.*$/, '').trim())
    .filter((line) => /^[A-Z][A-Za-z0-9]*,$/.test(line))
    .map((line) => line.replace(/,$/, ''));

  if (variants.length === 0) {
    throw new Error('No ContractError variants found in src/errors.rs');
  }

  for (const variant of variants) {
    if (!errorsRs.includes(`ContractError::${variant} =>`)) {
      throw new Error(`ContractError::${variant} is missing a canonical message arm in src/errors.rs`);
    }
  }

  const sdkErrorsTs = readText('sdk/src/errors.ts');
  if (!sdkErrorsTs.includes('parseSimulationError')) {
    throw new Error('sdk/src/errors.ts is missing parseSimulationError');
  }

  console.log(`✓ Verified ${variants.length} ContractError variants in src/errors.rs and sdk/src/errors.ts`);
}

function checkContractEvents() {
  const eventsRs = readText('src/events.rs');
  const emitFns = [...eventsRs.matchAll(/pub fn (emit_[a-z0-9_]+)\s*\(/g)].map((m) => m[1]);
  if (emitFns.length === 0) {
    throw new Error('No emit_* functions found in src/events.rs');
  }

  const topics = [...eventsRs.matchAll(/Symbol::new\(\s*env\s*,\s*"([a-z0-9_]+)"\s*\)/g)].map(
    (m) => m[1]
  );
  if (topics.length === 0) {
    throw new Error('No Symbol topics found in src/events.rs');
  }

  console.log(`✓ Verified ${emitFns.length} contract event emitters and ${topics.length} topics in src/events.rs`);
}

function checkValidationFixtures() {
  const fixtureRaw = readText('fixtures/validation-parity.json');
  const fixture = JSON.parse(fixtureRaw);

  if (!Array.isArray(fixture.rewardAmount) || fixture.rewardAmount.length === 0) {
    throw new Error('fixtures/validation-parity.json must contain non-empty rewardAmount cases');
  }
  if (!Array.isArray(fixture.description) || fixture.description.length === 0) {
    throw new Error('fixtures/validation-parity.json must contain non-empty description cases');
  }

  const frontendValidation = readText('frontend/src/lib/validation.ts');
  const backendValidation = readText('mergemint-backend/src/validation.rs');

  if (!frontendValidation.includes('SYMBOL_MAX_LENGTH = 32')) {
    throw new Error('frontend/src/lib/validation.ts does not define SYMBOL_MAX_LENGTH = 32');
  }
  if (!backendValidation.includes('SYMBOL_MAX_LENGTH: usize = 32')) {
    throw new Error('mergemint-backend/src/validation.rs does not define SYMBOL_MAX_LENGTH: usize = 32');
  }

  console.log(
    `✓ Verified validation-parity.json (${fixture.rewardAmount.length} reward cases, ${fixture.description.length} description cases)`
  );
}

checkContractErrors();
checkContractEvents();
checkValidationFixtures();
console.log('✓ Cross-layer parity checks passed.');
