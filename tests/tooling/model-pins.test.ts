import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { chunkDigests } from "../../scripts/generate-model-chunk-hashes.ts";
await test("chunk boundaries and short final chunk retain the whole-file trust anchor", async () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "pins-"));
  try {
    const file = path.join(dir, "weights");
    const data = Buffer.from("abcdefghi");
    writeFileSync(file, data);
    const sha256 = createHash("sha256").update(data).digest("hex");
    const actual = await chunkDigests(file, { bytes: 9, sha256 }, 4);
    assert.deepEqual(
      actual.map((x) => x.toString("hex")),
      ["abcd", "efgh", "i"].map((x) =>
        createHash("sha256").update(x).digest("hex"),
      ),
    );
    await assert.rejects(
      chunkDigests(file, { bytes: 9, sha256: "0".repeat(64) }, 4),
      /hash/,
    );
    await assert.rejects(chunkDigests(file, { bytes: 8, sha256 }, 4), /size/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
