import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { sha256 } from "../../scripts/tooling/files.ts";
import { record } from "../../scripts/tooling/io.ts";
const root = fileURLToPath(new URL("../../", import.meta.url));
const fixture = fileURLToPath(
  new URL("../fixtures/native-contract/", import.meta.url),
);
function execute(command: string, args: string[], env = process.env) {
  const result = spawnSync(command, args, {
    cwd: root,
    env,
    encoding: "utf8",
    timeout: 30000,
    maxBuffer: 1024 * 1024,
  });
  assert.ifError(result.error);
  return result;
}
await test("actual public VAD function clears failed frames and recovers after allocation/compute failure", () => {
  const raw = readFileSync(
    path.join(root, "vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"),
    "utf8",
  );
  assert.ok(Buffer.byteLength(raw) <= 2 * 1024 * 1024);
  const starts = [...raw.matchAll(/^bool whisper_vad_detect_speech\(\n/gm)],
    ends = [...raw.matchAll(/^int whisper_vad_segments_n_segments\(/gm)];
  assert.equal(starts.length, 1);
  assert.equal(ends.length, 1);
  assert.ok(ends[0]!.index > starts[0]!.index);
  const body = raw.slice(starts[0]!.index, ends[0]!.index);
  assert.ok(body.endsWith("}\n\n") && Buffer.byteLength(body) <= 16384);
  const dir = mkdtempSync(path.join(os.tmpdir(), "vad-contract-"));
  try {
    const source = path.join(dir, "contract.cpp"),
      binary = path.join(dir, "contract");
    writeFileSync(
      source,
      readFileSync(path.join(fixture, "vad-boundaries.cpp"), "utf8") +
        "\n" +
        body +
        "\n" +
        readFileSync(path.join(fixture, "vad-contract.cpp"), "utf8"),
    );
    const compile = execute("clang++", [
      "-std=c++17",
      "-Wall",
      "-Wextra",
      "-Werror",
      source,
      "-o",
      binary,
    ]);
    assert.equal(compile.status, 0, compile.stderr);
    for (const scenario of ["success", "first", "middle", "allocation"]) {
      const result = execute(binary, [scenario]);
      assert.equal(
        result.status,
        0,
        `${scenario} ${sha256(body)} ${result.stdout}${result.stderr}`,
      );
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
await test("actual Rust wrapper reclaims both constructor paths while SHA-pinned original leaks", () => {
  const current = path.join(root, "vendor/whisper-rs/src/whisper_vad.rs"),
    raw = readFileSync(current, "utf8");
  const patterns = record(
    JSON.parse(
      readFileSync(path.join(fixture, "wrapper-reconstruction.json"), "utf8"),
    ),
  );
  assert.equal(typeof patterns.patched, "string");
  assert.equal(typeof patterns.baseline, "string");
  const patched = String(patterns.patched),
    baseline = String(patterns.baseline);
  assert.equal(raw.split(patched).length, 2);
  assert.equal(raw.split("use std::ffi::CString;").length, 2);
  const original = raw
    .replace(patched, baseline)
    .replace("use std::ffi::CString;", "use std::ffi::{c_char, CString};");
  assert.equal(
    sha256(original),
    "d8033831a1b5aff45e8cdb27970bae5d492d1c92d9bf5b5001b91f32fe57be4a",
  );
  const dir = mkdtempSync(path.join(os.tmpdir(), "wrapper-contract-"));
  try {
    const before = path.join(dir, "original.rs");
    writeFileSync(before, original);
    for (const [label, source, code] of [
      ["original", before, 1],
      ["patched", current, 0],
    ] as const) {
      const binary = path.join(dir, label),
        compile = execute(
          "rustc",
          [
            "--edition=2021",
            path.join(
              root,
              "crates/macos-integration/tests/fixtures/whisper_vad_lifetime.rs",
            ),
            "-o",
            binary,
          ],
          { ...process.env, WHISPER_VAD_SOURCE: source },
        );
      assert.equal(compile.status, 0, compile.stderr);
      const result = execute(binary, []);
      assert.equal(result.status, code, result.stderr);
      const value = record(JSON.parse(result.stdout));
      assert.equal(value.validPaths, 200);
      assert.equal(value.nativeDrops, 100);
      assert.equal(value.unexpectedResults, 0);
      assert.equal(value.passed, code === 0);
      if (code === 1) {
        assert.equal(value.successFreed, 0);
        assert.equal(value.failureFreed, 0);
      }
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
