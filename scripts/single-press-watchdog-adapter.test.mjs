import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  realpathSync,
  renameSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
  existsSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import path from "node:path";
import os from "node:os";
import { createWatchdogAdapter } from "./single-press-watchdog-adapter.mjs";

function fixture() {
  const directory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-watchdog-adapter-")),
  );
  const workDirectory = path.join(directory, "work");
  const productDirectory = path.join(directory, "product");
  mkdirSync(workDirectory, { mode: 0o700 });
  mkdirSync(productDirectory, { mode: 0o700 });
  const runId = "a".repeat(32);
  writeFileSync(path.join(productDirectory, "owner"), runId, { mode: 0o600 });
  const executable = path.join(directory, "controlled-helper");
  writeFileSync(executable, "never executed", { mode: 0o700 });
  const executableSha256 = createHash("sha256")
    .update("never executed")
    .digest("hex");
  const productReport = path.join(productDirectory, "snapshot.json");
  const newReportDirectory = path.join(workDirectory, "guard");
  let elapsed = 0;
  let sequence = 0;
  let productSequence = 0;
  const now = () => 1800000000000 + elapsed;
  let phase = "Idle";
  let stale = false;
  let wrongProduct = false;
  let helperState = "waiting-for-start";
  let cleanupConfirmed = true;
  let unknownReason = false;
  let noAcknowledgement = false;
  let childClosed = false;
  let helperStarted = false;
  const calls = [];
  const child = new EventEmitter();
  child.pid = 7654321;
  child.end = (code) => {
    childClosed = true;
    child.emit("close", code, null);
  };
  const frozen = {
    contractId: "single-press-2026-10-19",
    runId,
    product: { processId: 424242, runId },
  };
  function product() {
    writeFileSync(
      productReport,
      JSON.stringify({
        schemaVersion: 1,
        kind: "everlast-one-press-product-status",
        contractId: frozen.contractId,
        runId,
        sequence: ++productSequence,
        observedUnixMillis: now() - (stale ? 2001 : 0),
        processId: wrongProduct ? 424243 : 424242,
        observerState: "observed",
        running: true,
        phase,
        sessionId: phase === "Idle" ? null : 1,
      }),
      { mode: 0o600 },
    );
  }
  function helper() {
    if (!helperStarted) return;
    const terminal = ["disarmed", "aborted", "failed"].includes(helperState);
    writeFileSync(
      path.join(newReportDirectory, "snapshot.json"),
      JSON.stringify({
        schemaVersion: 1,
        kind: "everlast-single-press-watchdog",
        contractId: frozen.contractId,
        runId,
        sequence: ++sequence,
        observedUnixMillis: now(),
        processId: child.pid,
        parentPid: process.pid,
        productPid: 424242,
        state: helperState,
        armed: !terminal,
        startObserved: phase !== "Idle",
        cleanupAttempted: terminal && helperState !== "disarmed",
        cleanupConfirmed:
          terminal && helperState !== "disarmed" && cleanupConfirmed,
        reason:
          helperState === "aborted"
            ? "requestedAbort"
            : helperState === "failed"
              ? unknownReason
                ? "unrecognized"
                : "observationInvalid"
              : null,
      }),
      { mode: 0o600 },
    );
  }
  function tick() {
    product();
    if (!helperStarted || childClosed) return;
    if (
      !noAcknowledgement &&
      existsSync(path.join(newReportDirectory, "abort.json"))
    ) {
      helperState = "aborted";
      helper();
      child.end(1);
      return;
    }
    if (
      !noAcknowledgement &&
      existsSync(path.join(newReportDirectory, "disarm.json"))
    ) {
      helperState = "disarmed";
      helper();
      child.end(0);
      return;
    }
    helper();
  }
  product();
  const options = {
    executable,
    executableSha256,
    productReport,
    workDirectory,
    newReportDirectory,
    clock: {
      now,
      monotonic: () => elapsed,
      sleep: async (ms) => {
        elapsed += ms;
        tick();
      },
    },
    spawnHelper: (program, args, options) => {
      calls.push({ program, args, options });
      mkdirSync(newReportDirectory, { mode: 0o700 });
      writeFileSync(path.join(newReportDirectory, "owner"), runId, {
        mode: 0o600,
      });
      helperStarted = true;
      helper();
      queueMicrotask(() => child.emit("spawn"));
      return child;
    },
  };
  const api = createWatchdogAdapter(options);
  return {
    api,
    options,
    frozen,
    calls,
    child,
    newReportDirectory,
    workDirectory,
    productReport,
    now,
    setPhase(value) {
      phase = value;
      tick();
    },
    setStale(value) {
      stale = value;
      tick();
    },
    setWrongProduct(value) {
      wrongProduct = value;
      tick();
    },
    setCleanup(value) {
      cleanupConfirmed = value;
    },
    setNoAcknowledgement() {
      noAcknowledgement = true;
    },
    failUnknown() {
      unknownReason = true;
      helperState = "failed";
      helper();
      child.end(1);
    },
    dispose() {
      rmSync(directory, { recursive: true, force: true });
    },
  };
}

test("preflight and arm bind actual configuration and independent spawn before disarm", async () => {
  const f = fixture();
  try {
    assert.equal(await f.api.preflight(f.frozen), true);
    assert.equal((await f.api.arm(f.frozen)).armed, true);
    assert.equal(f.calls[0].options.detached, true);
    assert.equal(f.calls[0].options.shell, false);
    const config = JSON.parse(readFileSync(f.calls[0].args[1], "utf8"));
    assert.equal(config.parentPid, process.pid);
    assert.equal(config.productPid, f.frozen.product.processId);
    assert.equal((await f.api.cleanup(f.frozen)).clean, true);
    assert.equal((await f.api.disarm(f.frozen)).disarmed, true);
  } finally {
    f.dispose();
  }
});

test("unknown terminal reason cannot prove cleanup", async () => {
  const f = fixture();
  try {
    await f.api.arm(f.frozen);
    f.setPhase("Recording");
    f.failUnknown();
    assert.equal((await f.api.cleanup(f.frozen)).clean, false);
  } finally {
    f.dispose();
  }
});

test("preflight rejects helper hash mismatch, stale product and wrong product identity without spawning", async () => {
  for (const mode of ["hash", "stale", "identity"]) {
    const f = fixture();
    try {
      if (mode === "hash") f.options.executableSha256 = "f".repeat(64);
      if (mode === "stale") f.setStale(true);
      if (mode === "identity") f.setWrongProduct(true);
      const api = mode === "hash" ? createWatchdogAdapter(f.options) : f.api;
      assert.equal(await api.preflight(f.frozen), false, mode);
      assert.equal(f.calls.length, 0);
    } finally {
      f.dispose();
    }
  }
});

test("non-idle cleanup requests abort and requires confirmed terminal exit", async () => {
  const f = fixture();
  try {
    await f.api.arm(f.frozen);
    f.setPhase("Recording");
    assert.equal((await f.api.cleanup(f.frozen)).clean, true);
    assert.deepEqual(
      JSON.parse(
        readFileSync(path.join(f.newReportDirectory, "abort.json"), "utf8"),
      ),
      { runId: f.frozen.runId },
    );
    assert.equal((await f.api.cleanup(f.frozen)).clean, true);
  } finally {
    f.dispose();
  }
});

test("reported failed cleanup and helper loss never become clean", async () => {
  for (const mode of ["false-cleanup", "helper-loss", "stale-helper-loss"]) {
    const f = fixture();
    try {
      await f.api.arm(f.frozen);
      f.setPhase("Recording");
      if (mode === "false-cleanup") f.setCleanup(false);
      else {
        if (mode === "stale-helper-loss") f.setStale(true);
        f.child.end(2);
      }
      assert.equal((await f.api.cleanup(f.frozen)).clean, false, mode);
    } finally {
      f.dispose();
    }
  }
});

test("disarm refuses a live recording and missing acknowledgement is bounded", async () => {
  const f = fixture();
  try {
    await f.api.arm(f.frozen);
    f.setPhase("Recording");
    await assert.rejects(f.api.disarm(f.frozen), /watchdog-disarm-not-idle/);
    assert.equal(
      existsSync(path.join(f.newReportDirectory, "disarm.json")),
      false,
    );
    f.setPhase("Idle");
    f.setNoAcknowledgement();
    const started = f.now();
    await assert.rejects(f.api.disarm(f.frozen), /watchdog-ack-timeout/);
    assert.ok(f.now() - started <= 5000);
  } finally {
    f.dispose();
  }
});

test("wrong watchdog process identity cannot supply a terminal cleanup proof", async () => {
  const f = fixture();
  try {
    await f.api.arm(f.frozen);
    f.setPhase("Recording");
    const filename = path.join(f.newReportDirectory, "snapshot.json");
    const report = JSON.parse(readFileSync(filename, "utf8"));
    Object.assign(report, {
      sequence: report.sequence + 1,
      state: "failed",
      armed: false,
      cleanupAttempted: true,
      cleanupConfirmed: true,
      reason: "observationInvalid",
      processId: report.processId + 1,
    });
    writeFileSync(filename, JSON.stringify(report), { mode: 0o600 });
    f.child.end(1);
    assert.equal((await f.api.cleanup(f.frozen)).clean, false);
  } finally {
    f.dispose();
  }
});

test("existing output directory or foreign owner marker cannot be reused", async () => {
  const f = fixture();
  try {
    mkdirSync(f.newReportDirectory, { mode: 0o700 });
    assert.equal(await f.api.preflight(f.frozen), false);
    assert.equal(f.calls.length, 0);
  } finally {
    f.dispose();
  }
  const other = fixture();
  try {
    writeFileSync(
      path.join(path.dirname(other.productReport), "owner"),
      "b".repeat(32),
      { mode: 0o600 },
    );
    assert.equal(await other.api.preflight(other.frozen), false);
  } finally {
    other.dispose();
  }
});

test(
  "opt-in real native helper arms and disarms around an owned benign Node process",
  {
    skip: process.env.EVERLAST_RUN_WATCHDOG_ADAPTER_NATIVE !== "1",
    timeout: 15000,
  },
  async () => {
    const { spawn } = await import("node:child_process");
    const { once } = await import("node:events");
    const directory = realpathSync(
      mkdtempSync(path.join(os.tmpdir(), "one-press-native-adapter-")),
    );
    const work = path.join(directory, "work");
    const reports = path.join(directory, "product");
    mkdirSync(work, { mode: 0o700 });
    mkdirSync(reports, { mode: 0o700 });
    const runId = "c".repeat(32);
    writeFileSync(path.join(reports, "owner"), runId, { mode: 0o600 });
    const product = spawn(
      process.execPath,
      ["-e", "setInterval(()=>{},1000)"],
      { stdio: "ignore" },
    );
    let helper;
    let timer;
    try {
      await once(product, "spawn");
      let sequence = 0;
      const productReport = path.join(reports, "snapshot.json");
      const publish = () => {
        const pending = path.join(reports, "pending.json");
        writeFileSync(
          pending,
          JSON.stringify({
            schemaVersion: 1,
            kind: "everlast-one-press-product-status",
            contractId: "single-press-2026-10-19",
            runId,
            sequence: ++sequence,
            observedUnixMillis: Date.now(),
            processId: product.pid,
            observerState: "observed",
            running: true,
            phase: "Idle",
            sessionId: null,
          }),
          { mode: 0o600, flag: "wx" },
        );
        renameSync(pending, productReport);
      };
      publish();
      timer = setInterval(publish, 100);
      const executable = path.resolve(
        "target/debug/examples/single_press_watchdog",
      );
      const executableSha256 = createHash("sha256")
        .update(readFileSync(executable))
        .digest("hex");
      const api = createWatchdogAdapter({
        executable,
        executableSha256,
        productReport,
        workDirectory: work,
        newReportDirectory: path.join(work, "guard"),
        spawnHelper: (...args) => {
          helper = spawn(...args);
          return helper;
        },
      });
      const frozen = {
        contractId: "single-press-2026-10-19",
        runId,
        product: { processId: product.pid, runId },
      };
      assert.equal(await api.preflight(frozen), true);
      assert.equal((await api.arm(frozen)).armed, true);
      assert.equal((await api.cleanup(frozen)).clean, true);
      assert.equal((await api.disarm(frozen)).disarmed, true);
    } finally {
      clearInterval(timer);
      if (helper && helper.exitCode === null && helper.signalCode === null) {
        helper.kill("SIGTERM");
        await once(helper, "close");
      }
      if (product.exitCode === null && product.signalCode === null) {
        product.kill("SIGTERM");
        await once(product, "close");
      }
      rmSync(directory, { recursive: true, force: true });
    }
  },
);
