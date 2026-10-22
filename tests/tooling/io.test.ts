import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import os from "node:os";
import { parseJson, writeNew, readBytes } from "../../scripts/tooling/io.ts";
import { main } from "../../scripts/quality.ts";
await test("strict JSON refuses duplicate decoded keys and non-finite or malformed input", () => {
  for (const value of [
    '{"a":1,"\\u0061":2}',
    "1e999",
    "NaN",
    "[1,]",
    '{"x":1,}',
    "01",
    '{"a":{"b":1,"b":2}}',
  ])
    assert.throws(() => parseJson(value));
  assert.deepEqual(parseJson('{"a":[1,true,null,"雪"]}'), {
    a: [1, true, null, "雪"],
  });
});
await test("atomic report output preserves existing files and symlinks", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "quality-ts-"));
  try {
    const output = path.join(dir, "out");
    writeNew(output, { ok: true });
    assert.throws(() => writeNew(output, { bad: true }));
    assert.deepEqual(JSON.parse(readFileSync(output, "utf8")), { ok: true });
    const link = path.join(dir, "link");
    symlinkSync(output, link);
    assert.throws(() => writeNew(link, { bad: true }));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
await test("quality CLI writes a numeric report and never replaces it", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "quality-cli-"));
  try {
    const m = path.join(dir, "m.json"),
      r = path.join(dir, "r.json"),
      o = path.join(dir, "o.json");
    writeFileSync(
      m,
      JSON.stringify({
        schemaVersion: 1,
        unicodeVersion: "13.0.0",
        cases: [
          {
            id: "a",
            language: "de",
            class: "clean",
            split: "acceptance",
            reference: "one two",
          },
        ],
      }),
    );
    writeFileSync(r, JSON.stringify([{ id: "a", status: "ok", text: "one" }]));
    assert.equal(main([m, r, "--output", o]), 0);
    assert.equal(JSON.parse(readFileSync(o, "utf8")).cases[0].edits, 1);
    assert.equal(main([m, r, "--output", o]), 2);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
await test("bounded input refuses oversized files and directories before parsing", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "bounded-input-"));
  try {
    const file = path.join(dir, "large");
    writeFileSync(file, "12345");
    assert.throws(() => readBytes(file, 4));
    assert.throws(() => readBytes(dir, 100));
    assert.equal(readBytes(file, 5).toString(), "12345");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
