import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const config = path.resolve(".gitleaks.toml");
// Deterministic public fixture data, never an issued credential.
const checksum = createHash("sha256")
  .update("public Cargo checksum fixture")
  .digest("hex");
const nonChecksum = createHash("sha512")
  .update("another public fixture")
  .digest("base64");
const generated =
  "artifacts/p06-exact-state-counter-diagnostic-v1/public-dependencies/example-1.0.0/.cargo-checksum.json";

function scan(file, content) {
  const directory = fs.mkdtempSync(
    path.join(os.tmpdir(), "everlast-secret-rule-"),
  );
  try {
    const target = path.join(directory, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
    const result = spawnSync(
      "gitleaks",
      [
        "dir",
        "--no-banner",
        "--no-color",
        "--redact=100",
        "--ignore-gitleaks-allow",
        "--config",
        config,
        directory,
      ],
      { encoding: "utf8" },
    );
    assert.ifError(result.error);
    assert.ok(
      [0, 1].includes(result.status),
      `Scanner execution failed: ${result.stderr}`,
    );
    return result.status;
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

const metadata = JSON.stringify({
  files: { "src/api.rs": checksum },
  package: checksum,
});

test("isolated generated public Cargo checksums do not count as credentials", () => {
  assert.equal(scan(generated, metadata), 0);
});

test("the same checksum-shaped value in application source remains detectable", () => {
  assert.equal(scan("src/credentials.rs", `const API_KEY = "${checksum}";`), 1);
});

test("checksum metadata outside the exact public dependency archive remains detectable", () => {
  assert.equal(scan("src/.cargo-checksum.json", metadata), 1);
});

test("a non-checksum credential shape in that archive remains detectable", () => {
  assert.equal(scan(generated, `const API_KEY = "${nonChecksum}";`), 1);
});
