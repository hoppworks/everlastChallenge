import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { normalize, evaluate } from "../../scripts/tooling/quality.ts";
import { record } from "../../scripts/tooling/io.ts";
const fixture = record(
  JSON.parse(
    readFileSync(
      new URL("../fixtures/quality-unicode13.json", import.meta.url),
      "utf8",
    ),
  ),
);
await test("normalization preserves frozen Unicode-13 outputs across contextual case and future code points", () => {
  assert.ok(Array.isArray(fixture.normalization));
  for (const raw of fixture.normalization) {
    const row = record(raw);
    assert.deepEqual(normalize(row.input), row.expected);
  }
});
await test("aggregate and per-case metrics preserve the prior evaluator including failed and absent results", () =>
  assert.deepEqual(evaluate(fixture.cases, fixture.results), fixture.expected));
