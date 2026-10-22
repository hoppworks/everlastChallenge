import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { once } from "node:events";
import { test } from "node:test";
import {
  owned,
  samePower,
  type Power,
} from "../../scripts/controlled-probe.ts";
import { reapOwned } from "../../scripts/tooling/owned-probe.ts";
const power = (): Power => ({
  utc: new Date().toISOString(),
  source: "AC Power",
  policy: "fixed",
  batteryReadout: "AC",
});
await test("same-power gate rejects missing and changed observations", () => {
  assert.throws(() => samePower([power()]));
  assert.throws(() =>
    samePower([power(), { ...power(), source: "Battery Power" }]),
  );
  samePower([power(), power()]);
});
await test("owned timeout confirms cleanup and never qualifies the run", async () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "owner-test-"));
  try {
    const result = await owned({
      folder: dir,
      revision: "test",
      command: [
        process.execPath,
        "-e",
        "process.on('SIGTERM',()=>{});setInterval(()=>{},1000)",
      ],
      timeoutMs: 250,
      sampleIntervalMs: 50,
      graceMs: 50,
      observationMs: 20,
      power,
    });
    assert.equal(result.watchdogExpired, true);
    assert.equal(result.qualified, false);
    assert.equal(result.terminationConfirmed, true);
    assert.match(
      readFileSync(path.join(dir, "owner-completion.json"), "utf8"),
      /watchdogExpired/,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
await test("permission-denied group observation cannot confirm absence", async () => {
  const child = spawn(process.execPath, ["-e", ""], {
    detached: true,
    stdio: "ignore",
  });
  await once(child, "exit");
  const denied = () => {
    throw Object.assign(new Error("denied"), { code: "EPERM" });
  };
  const result = await reapOwned(child, 10, 10, denied);
  assert.equal(result.processReaped, true);
  assert.equal(result.terminationConfirmed, false);
  assert.ok(result.cleanupObservationErrors.length);
});
await test("successful child gets a post-reap power observation; output is not overwritten", async () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "owner-success-"));
  try {
    const result = await owned({
      folder: dir,
      revision: "test",
      command: [process.execPath, "-e", ""],
      power,
      observationMs: 10,
    });
    assert.equal(result.qualified, true);
    assert.ok(result.powerObservations.length >= 2);
    await assert.rejects(
      owned({
        folder: dir,
        revision: "test",
        command: [process.execPath, "-e", ""],
        power,
      }),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

await test("forced cleanup waits for the stubborn child's readiness", async () => {
  const child = spawn(
    process.execPath,
    [
      "-e",
      "process.on('SIGTERM',()=>{});process.stdout.write('ready');setInterval(()=>{},1000)",
    ],
    { detached: true, stdio: ["ignore", "pipe", "ignore"] },
  );
  assert.ok(child.stdout);
  child.on("error", (error) => child.stdout?.destroy(error));
  let cleaned = false;
  try {
    const [data] = await once(child.stdout, "data", {
      signal: AbortSignal.timeout(5000),
    });
    assert.equal(String(data), "ready");
    const result = await reapOwned(child, 50, 20);
    cleaned = result.terminationConfirmed;
    assert.equal(result.forcedGroupCleanup, true);
    assert.equal(result.terminationConfirmed, true);
    assert.equal(result.signal, "SIGKILL");
  } finally {
    if (!cleaned) await reapOwned(child, 100, 50);
  }
});
