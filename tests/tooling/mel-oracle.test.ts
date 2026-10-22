import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { generate } from "../../scripts/generate-whisper-mel-oracle.ts";
import { record } from "../../scripts/tooling/io.ts";
await test("TypeScript direct DFT exactly reproduces frozen Python oracle arrays", () => {
  const old = record(
    JSON.parse(
      readFileSync(
        new URL("../fixtures/mlx/whisper-mel-oracle-v1.json", import.meta.url),
        "utf8",
      ),
    ),
  );
  const actual = record(
    generate(
      fileURLToPath(
        new URL("../fixtures/mlx/mel_filters.npz", import.meta.url),
      ),
    ),
  );
  assert.deepEqual(actual.cases, old.cases);
  assert.equal(actual.filterSha256, old.filterSha256);
});
