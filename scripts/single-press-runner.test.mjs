import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { CONTRACT_ID, createGestureLedger } from "./single-press-runner.mjs";

test("gesture consumption survives a new ledger adapter and different run directory", async () => {
  const dir = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-ledger-")),
  );
  try {
    createGestureLedger(path.join(dir, "ledger")).consume({
      contractId: CONTRACT_ID,
      runId: "a".repeat(32),
    });
    const child = await import("node:child_process");
    const script = `import {createGestureLedger,CONTRACT_ID} from ${JSON.stringify(new URL("./single-press-runner.mjs", import.meta.url).href)};try {createGestureLedger(${JSON.stringify(path.join(dir, "ledger"))}).consume({contractId:CONTRACT_ID,runId:"${"b".repeat(32)}"});process.exit(7)} catch {process.exit(0)}`;
    const result = child.spawnSync(process.execPath, [
      "--input-type=module",
      "-e",
      script,
    ]);
    assert.equal(
      result.status,
      0,
      "another process/run must not regain the gesture",
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

function fixture(overrides = {}, startAt = 2000) {
  let elapsed = 0;
  let signalState = null;
  const runId = "a".repeat(32);
  const events = [];
  const text = "public fixture";
  const sha = "3".repeat(64);
  const frozen = {
    contractId: CONTRACT_ID,
    runId,
    product: { processId: 4321, runId },
    recipient: { processId: 5432, processStartedUnixMillis: 1000 },
    artifactSha256: "1".repeat(64),
    modelSha256: "2".repeat(64),
    fixtureSha256: sha,
    fixtureDurationMillis: 300000,
    normalBuild: true,
  };
  const now = () => 1800000000000 + elapsed;
  const options = {
    frozen,
    clock: {
      now,
      monotonic: () => elapsed,
      sleep: async (ms) => {
        elapsed += ms;
      },
    },
    pollMillis: 1000,
    ledger: { consume: () => events.push("consume") },
    verifyFrozen: async () => true,
    watchdog: {
      preflight: async () => true,
      arm: async () => events.push("arm"),
      disarm: async () => events.push("disarm"),
      abort: async () => {
        events.push("abort");
        return { clean: true };
      },
    },
    signal: async ({ state }) => {
      signalState = state;
      events.push(`signal:${state}`);
    },
    audio: {
      start: async () => events.push("audio:start"),
      stop: async () => events.push("audio:stop"),
    },
    cleanup: async () => {
      events.push("cleanup");
      return { clean: true };
    },
    writeReport: async () => {},
    observe: async () => {
      const recording = elapsed >= startAt && elapsed < startAt + 300000;
      const final = elapsed >= startAt + 301000;
      return {
        product: {
          schemaVersion: 1,
          kind: "everlast-one-press-product-status",
          contractId: CONTRACT_ID,
          runId,
          sequence: elapsed + 1,
          observedUnixMillis: now(),
          monotonicSinceLaunchMillis: elapsed + 1000,
          processId: 4321,
          setupReady: true,
          microphoneGranted: true,
          accessibilityGranted: true,
          running: true,
          phase: recording
            ? "Recording"
            : final
              ? "Idle"
              : elapsed >= startAt + 300000
                ? "Finalizing"
                : "Idle",
          sessionId: elapsed >= startAt && !final ? 1 : null,
          outcome: final ? "sent" : null,
          maximumWarning: elapsed >= startAt + 270000,
          maximumReached: elapsed >= startAt + 300000,
          elapsedMillis: recording ? elapsed - startAt : 0,
          transcriptUtf8Bytes: final ? Buffer.byteLength(text) : 0,
          transcriptSha256: final ? sha : null,
          observerState: "observed",
        },
        recipient: {
          schemaVersion: 1,
          kind: "everlast-own-native-recipient-status",
          sequence: elapsed + 1,
          observedUnixMillis: now(),
          monotonicSinceLaunchNanos: elapsed * 1000000,
          processId: 5432,
          processStartedUnixMillis: 1000,
          running: true,
          status: "observed",
          applicationActive: true,
          frontmostIsSelf: true,
          windowKey: true,
          accessibilityFocused: true,
          focusedField: "multiline",
          fieldUtf8Bytes: final ? Buffer.byteLength(text) : 0,
          fieldSha256: final ? sha : null,
          totalUtf8Bytes: final ? Buffer.byteLength(text) : 0,
          shortcutCharacterPresent: false,
          signalState,
          signalRunId: signalState ? runId : null,
        },
      };
    },
    ...overrides,
  };
  return {
    options,
    events,
    get elapsed() {
      return elapsed;
    },
  };
}

test("one gesture runs real observed start before audio and requires warning/autostop/exact receipt", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const { options, events } = fixture();
  const result = await runSinglePress(options);
  assert.equal(result.state, "passed");
  assert.ok(events.indexOf("consume") < events.indexOf("signal:ready"));
  assert.ok(events.indexOf("signal:ready") < events.indexOf("audio:start"));
  assert.equal(
    events.filter((event) => event === "signal:ready").length >= 1,
    true,
  );
  assert.equal(result.transportConfirmed, true);
  assert.equal(result.qualityQualified, false);
  assert.equal(result.observations.warning.elapsedMillis, 270000);
  assert.ok(result.observations.stop.elapsedSinceStartMillis >= 300000);
  assert.equal(events.filter((event) => event === "audio:start").length, 1);
  assert.ok(events.includes("cleanup"));
});

test("preflight focus or frozen-identity failure never consumes or prompts", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  for (const mode of ["focus", "binding", "watchdog"]) {
    const f = fixture();
    const original = f.options.observe;
    if (mode === "focus")
      f.options.observe = async () => {
        const pair = await original();
        pair.recipient.frontmostIsSelf = false;
        return pair;
      };
    if (mode === "binding") f.options.verifyFrozen = async () => false;
    if (mode === "watchdog") f.options.watchdog.preflight = async () => false;
    const result = await runSinglePress(f.options);
    assert.equal(result.state, "failed");
    assert.equal(result.gestureConsumed, false);
    assert.ok(!f.events.includes("consume"));
    assert.ok(!f.events.includes("signal:ready"));
    assert.ok(!f.events.includes("audio:start"));
  }
});

test("stale, switched identity, bad warning, duplicate receipt and shortcut characters fail and clean only through injected ownership", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  for (const mode of [
    "stale",
    "identity",
    "warning",
    "duplicate",
    "shortcut",
    "digest",
    "session",
  ]) {
    const f = fixture();
    const original = f.options.observe;
    f.options.observe = async () => {
      const pair = await original();
      if (f.elapsed >= 5000 && mode === "stale")
        pair.product.observedUnixMillis -= 2001;
      if (f.elapsed >= 5000 && mode === "identity")
        pair.recipient.processStartedUnixMillis++;
      if (mode === "warning") pair.product.maximumWarning = false;
      if (f.elapsed >= 303000 && mode === "duplicate")
        pair.recipient.totalUtf8Bytes *= 2;
      if (f.elapsed >= 5000 && mode === "shortcut")
        pair.recipient.shortcutCharacterPresent = true;
      if (f.elapsed >= 303000 && mode === "digest")
        pair.recipient.fieldSha256 = "f".repeat(64);
      if (f.elapsed >= 5000 && mode === "session") pair.product.sessionId = 7;
      return pair;
    };
    const result = await runSinglePress(f.options);
    assert.equal(result.state, "failed", mode);
    assert.equal(result.gestureConsumed, true, mode);
    assert.equal(f.events.filter((event) => event === "consume").length, 1);
    assert.ok(f.events.includes("cleanup"), mode);
    assert.ok(f.events.includes("audio:stop"), mode);
    assert.ok(f.events.includes("abort"), mode);
    assert.equal(f.events.includes("disarm"), false, mode);
  }
});

test("recording and result watchdogs never ask for a second gesture", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  for (const mode of ["recording", "result", "start"]) {
    const f = fixture();
    const original = f.options.observe;
    f.options.observe = async () => {
      const pair = await original();
      if (mode === "start") {
        pair.product.phase = "Idle";
        pair.product.sessionId = null;
      } else if (mode === "recording" && f.elapsed >= 302000) {
        pair.product.phase = "Recording";
        pair.product.sessionId = 1;
        pair.product.elapsedMillis = f.elapsed - 2000;
        pair.product.transcriptUtf8Bytes = 0;
        pair.product.transcriptSha256 = null;
        pair.recipient.fieldUtf8Bytes = pair.recipient.totalUtf8Bytes = 0;
        pair.recipient.fieldSha256 = null;
      } else if (mode === "result" && f.elapsed >= 303000) {
        pair.product.phase = "Finalizing";
        pair.product.sessionId = 1;
      }
      return pair;
    };
    const result = await runSinglePress(f.options);
    assert.equal(result.state, "failed", mode);
    assert.equal(
      result.failure,
      mode === "recording"
        ? "recording-watchdog"
        : mode === "result"
          ? "result-timeout"
          : "start-timeout",
    );
    assert.equal(f.events.filter((event) => event === "consume").length, 1);
    assert.ok(f.events.includes("cleanup"));
    assert.ok(f.elapsed <= 424000);
  }
});

test("terminal product snapshot waits for newer matching recipient readback", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const original = f.options.observe;
  f.options.observe = async () => {
    const pair = await original();
    if (f.elapsed >= 303000 && f.elapsed < 305000) {
      pair.recipient.fieldUtf8Bytes = pair.recipient.totalUtf8Bytes = 0;
      pair.recipient.fieldSha256 = null;
    }
    return pair;
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.state, "passed");
  assert.ok(f.elapsed >= 305000);
});

test("real Accessibility focus and signal acknowledgement precede audio", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const original = f.options.observe;
  f.options.observe = async () => {
    const pair = await original();
    pair.recipient.accessibilityFocused = false;
    return pair;
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.state, "failed");
  assert.equal(result.gestureConsumed, false);
  assert.ok(!f.events.includes("audio:start"));
});

test("signal-file adapter replaces only its own inode and exposes no arbitrary prompt text", async () => {
  const { chmodSync, readFileSync, renameSync, writeFileSync } =
    await import("node:fs");
  const { createRecipientSignalAdapter, readPrivateJson } =
    await import("./single-press-runner.mjs");
  const dir = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-signal-")),
  );
  chmodSync(dir, 0o700);
  try {
    const signal = createRecipientSignalAdapter(dir, "a".repeat(32));
    await signal({
      schemaVersion: 1,
      contractId: CONTRACT_ID,
      runId: "a".repeat(32),
      state: "ready",
      observedUnixMillis: 1000,
      text: "must not be persisted",
    });
    const report = readPrivateJson(path.join(dir, "signal.json"));
    assert.equal(report.state, "ready");
    assert.equal(Object.hasOwn(report, "text"), false);
    await signal({ ...report, state: "running", observedUnixMillis: 2000 });
    renameSync(path.join(dir, "signal.json"), path.join(dir, "previous.json"));
    writeFileSync(path.join(dir, "signal.json"), "foreign", { mode: 0o600 });
    await assert.rejects(
      signal({ ...report, state: "verifying", observedUnixMillis: 3000 }),
      /output-file-replaced/,
    );
    assert.equal(
      readFileSync(path.join(dir, "signal.json"), "utf8"),
      "foreign",
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("private report reader rejects symlinks, wrong permissions, oversize and invalid UTF8", async () => {
  const { chmodSync, symlinkSync, unlinkSync, writeFileSync } =
    await import("node:fs");
  const { readPrivateJson } = await import("./single-press-runner.mjs");
  const dir = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-report-")),
  );
  chmodSync(dir, 0o700);
  const filename = path.join(dir, "snapshot.json");
  try {
    writeFileSync(filename, "{}", { mode: 0o644 });
    assert.throws(() => readPrivateJson(filename));
    chmodSync(filename, 0o600);
    assert.deepEqual(readPrivateJson(filename), {});
    writeFileSync(filename, Buffer.from([0xff]));
    assert.throws(() => readPrivateJson(filename));
    writeFileSync(filename, "x".repeat(65537));
    assert.throws(() => readPrivateJson(filename));
    unlinkSync(filename);
    writeFileSync(path.join(dir, "target.json"), "{}", { mode: 0o600 });
    symlinkSync(path.join(dir, "target.json"), filename);
    assert.throws(() => readPrivateJson(filename));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("prompt failure permanently retains consumed gesture across process restart", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const dir = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-failed-")),
  );
  try {
    const f = fixture();
    f.options.ledger = createGestureLedger(path.join(dir, "ledger"));
    f.options.signal = async ({ state }) => {
      if (state === "ready") throw new Error("display failed");
    };
    const result = await runSinglePress(f.options);
    assert.equal(result.state, "failed");
    assert.equal(result.gestureConsumed, true);
    assert.throws(
      () =>
        createGestureLedger(path.join(dir, "ledger")).consume({
          contractId: CONTRACT_ID,
          runId: "b".repeat(32),
        }),
      /already-consumed/,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("ready acknowledgement may arrive after first Recording snapshot without starting audio early", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const original = f.options.observe;
  f.options.observe = async () => {
    const pair = await original();
    if (f.elapsed < 3000) {
      pair.recipient.signalState = null;
      pair.recipient.signalRunId = null;
    }
    return pair;
  };
  f.options.audio.start = async () => {
    assert.ok(f.elapsed >= 3000);
    f.events.push("audio:start");
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.state, "passed");
  assert.equal(result.observations.start.elapsedMillis, 1000);
});

test("a terminal transcript mutation during readback cannot pass", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const original = f.options.observe;
  f.options.observe = async () => {
    const pair = await original();
    if (f.elapsed >= 303000) {
      pair.recipient.fieldUtf8Bytes = pair.recipient.totalUtf8Bytes = 0;
      pair.recipient.fieldSha256 = null;
    }
    if (f.elapsed >= 304000) pair.product.transcriptSha256 = "f".repeat(64);
    return pair;
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.state, "failed");
  assert.equal(result.failure, "terminal-result-changed");
});

test("cleanup failure prevents a passed verdict even after exact receipt", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture({ cleanup: async () => ({ clean: false }) });
  const result = await runSinglePress(f.options);
  assert.equal(result.transportConfirmed, true);
  assert.equal(result.state, "failed");
  assert.equal(result.failure, "cleanup-unconfirmed");
  assert.equal(f.events.includes("disarm"), false);
});

test("throwing cleanup leaves the independent safety guard armed", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture({
    cleanup: async () => {
      throw new Error("cleanup failed");
    },
  });
  const result = await runSinglePress(f.options);
  assert.equal(result.state, "failed");
  assert.equal(result.failure, "cleanup-failed");
  assert.equal(f.events.includes("arm"), true);
  assert.equal(f.events.includes("disarm"), false);
});

test("observers near their lifetime limit cannot consume the sole gesture", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  for (const kind of ["product", "recipient"]) {
    const f = fixture();
    const original = f.options.observe;
    f.options.observe = async () => {
      const pair = await original();
      if (kind === "product") pair.product.monotonicSinceLaunchMillis = 800000;
      else pair.recipient.monotonicSinceLaunchNanos = 1700000000000;
      return pair;
    };
    const result = await runSinglePress(f.options);
    assert.equal(result.state, "failed");
    assert.equal(result.failure, "observation-budget-insufficient");
    assert.equal(result.gestureConsumed, false);
    assert.equal(f.events.includes("signal:ready"), false);
  }
});

test("read-only preflight shares readiness gates without consuming, signalling or arming", async () => {
  const { preflightSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const result = await preflightSinglePress(f.options);
  assert.equal(result.ready, true);
  assert.equal(result.observations.product.phase, "Idle");
  assert.deepEqual(f.events, []);
  const bad = fixture();
  const original = bad.options.observe;
  bad.options.observe = async () => {
    const pair = await original();
    pair.recipient.frontmostIsSelf = false;
    return pair;
  };
  await assert.rejects(
    preflightSinglePress(bad.options),
    /recipient-focus-lost/,
  );
  assert.deepEqual(bad.events, []);
});

test("transient focus loss while Idle waits without another consumed gesture", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture({}, 8000);
  const observe = f.options.observe;
  f.options.observe = async () => {
    const pair = await observe();
    if (f.elapsed >= 2000 && f.elapsed < 6000)
      pair.recipient.frontmostIsSelf = false;
    return pair;
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.state, "passed");
  assert.equal(f.events.filter((event) => event === "consume").length, 1);
  assert.ok(f.events.includes("signal:preparing"));
  assert.ok(
    f.events.lastIndexOf("signal:ready") > f.events.indexOf("signal:preparing"),
  );
  assert.ok(f.events.includes("audio:start"));
});

test("start observed without focus fails before audio and aborts even an Idle-clean product", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const observe = f.options.observe;
  f.options.observe = async () => {
    const pair = await observe();
    if (pair.product.phase === "Recording")
      pair.recipient.frontmostIsSelf = false;
    return pair;
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.failure, "recipient-focus-lost");
  assert.equal(f.events.includes("audio:start"), false);
  assert.ok(f.events.includes("abort"));
  assert.equal(f.events.includes("disarm"), false);
});

test("unfocused Idle waiting retains the original deadline and rejects stale identity", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  for (const mode of ["deadline", "stale", "identity"]) {
    const f = fixture({}, 200000);
    const observe = f.options.observe;
    f.options.observe = async () => {
      const pair = await observe();
      if (f.elapsed >= 2000) {
        pair.recipient.frontmostIsSelf = false;
        if (mode === "stale") pair.recipient.observedUnixMillis -= 2001;
        if (mode === "identity") pair.recipient.processId++;
      }
      return pair;
    };
    const result = await runSinglePress(f.options);
    assert.equal(result.state, "failed", mode);
    assert.equal(
      result.failure,
      mode === "deadline"
        ? "start-timeout"
        : mode === "stale"
          ? "stale-observation"
          : "invalid-recipient-observation",
    );
    assert.equal(f.events.filter((event) => event === "consume").length, 1);
    assert.equal(f.events.includes("audio:start"), false);
    assert.ok(f.events.includes("abort"));
    assert.ok(f.elapsed <= 122000);
  }
});

test("failed final signal after the prompt still aborts the bound product", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  const signal = f.options.signal;
  f.options.signal = async (value) => {
    if (value.state === "passed") throw new Error("unavailable recipient");
    return signal(value);
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.failure, "signal-failed");
  assert.ok(f.events.includes("abort"));
  assert.equal(f.events.includes("disarm"), false);
});

test("disarm failure revokes a published success and requests bound abort", async () => {
  const { runSinglePress } = await import("./single-press-runner.mjs");
  const f = fixture();
  f.options.watchdog.disarm = async () => {
    throw new Error("disarm failed");
  };
  const result = await runSinglePress(f.options);
  assert.equal(result.failure, "watchdog-cleanup-failed");
  assert.ok(f.events.includes("abort"));
  assert.equal(
    f.events.filter((event) => event.startsWith("signal:")).at(-1),
    "signal:failed",
  );
});
