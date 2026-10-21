import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
test("dependency gate rejects an orphan file and an unused declared dependency", () => {
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), "everlast-knip-"));
  try {
    fs.mkdirSync(path.join(fixture, "src"));
    fs.writeFileSync(
      path.join(fixture, "package.json"),
      JSON.stringify({
        name: "guardrail-fixture",
        private: true,
        dependencies: { react: "19.3.1" },
      }),
    );
    fs.writeFileSync(
      path.join(fixture, "knip.json"),
      JSON.stringify({
        entry: ["src/main.ts"],
        project: ["src/**/*.ts"],
      }),
    );
    fs.writeFileSync(
      path.join(fixture, "src/main.ts"),
      'console.log("fixture");',
    );
    fs.writeFileSync(
      path.join(fixture, "src/orphan.ts"),
      "export const orphan = 1;",
    );
    const run = () =>
      spawnSync(
        process.execPath,
        [
          path.join(root, "node_modules/knip/bin/knip.js"),
          "--directory",
          fixture,
          "--no-progress",
          "--reporter",
          "json",
        ],
        { encoding: "utf8", timeout: 15000 },
      );
    const failed = run();
    assert.equal(failed.status, 1, failed.stderr);
    assert.match(failed.stdout, /orphan\.ts/);
    assert.match(failed.stdout, /react/);
    fs.unlinkSync(path.join(fixture, "src/orphan.ts"));
    fs.writeFileSync(
      path.join(fixture, "package.json"),
      '{"name":"guardrail-fixture","private":true}',
    );
    const passed = run();
    assert.equal(passed.status, 0, passed.stdout + passed.stderr);
  } finally {
    fs.rmSync(fixture, { recursive: true, force: true });
  }
});
