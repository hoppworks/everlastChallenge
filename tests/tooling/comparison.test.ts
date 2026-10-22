import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { sha256 } from "../../scripts/tooling/files.ts";
import { test } from "node:test";
import { compare } from "../../scripts/compare-mlx-quality.ts";
await test("quality comparison requires a current pair-bound semantic review even for an automatic improvement", () => {
  const cases = [
    {
      id: "a",
      language: "de",
      class: "clean",
      split: "acceptance",
      reference: "eins zwei",
    },
  ];
  const before = [{ id: "a", status: "ok", text: "eins" }],
    after = [{ id: "a", status: "ok", text: "eins zwei" }];
  const report = compare(cases, before, after);
  assert.equal(report.automaticGatePassed, true);
  assert.equal(report.qualified, false);
  const pairSha256 = report.reviewsRequired[0]!.pairSha256;
  assert.equal(
    compare(cases, before, after, {
      a: { pairSha256, accepted: true, note: "Both words verified" },
    }).qualified,
    true,
  );
  assert.equal(
    compare(cases, before, after, {
      a: { pairSha256: "stale", accepted: true, note: "old" },
    }).qualified,
    false,
  );
  assert.equal(compare(cases, after, before).automaticGatePassed, false);
});
await test("unchanged comparison CLI still binds every exact input", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "comparison-cli-"));
  try {
    const manifest = JSON.stringify({
        schemaVersion: 1,
        unicodeVersion: "13.0.0",
        cases: [
          {
            id: "a",
            language: "de",
            class: "clean",
            split: "acceptance",
            reference: "eins",
          },
        ],
      }),
      results = JSON.stringify([{ id: "a", status: "ok", text: "eins" }]);
    const m = path.join(dir, "manifest.json"),
      r = path.join(dir, "results.json"),
      out = path.join(dir, "report.json");
    writeFileSync(m, manifest);
    writeFileSync(r, results);
    const run = spawnSync(
      process.execPath,
      [
        "--import",
        "tsx",
        fileURLToPath(
          new URL("../../scripts/compare-mlx-quality.ts", import.meta.url),
        ),
        m,
        r,
        r,
        "--output",
        out,
      ],
      { encoding: "utf8", timeout: 10000, maxBuffer: 65536 },
    );
    assert.equal(run.status, 0, run.stderr);
    const report = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(report.qualified, true);
    assert.deepEqual(report.inputs, {
      manifest: sha256(manifest),
      baseline: sha256(results),
      candidate: sha256(results),
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
