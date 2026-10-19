import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, writeFileSync, rmSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import os from "node:os";
import { CONTRACT_ID } from "./single-press-runner.mjs";
import { PUBLIC_AUDIO_SHA256 } from "./single-press-playback.mjs";
import {
  executeSinglePressEntry,
  INSTALLED_EXECUTABLE,
} from "./run-single-press.mjs";

function fixture() {
  const directory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "single-press-entry-")),
  );
  const runId = "a".repeat(32);
  const sha = "1".repeat(64);
  const fingerprint = path.join(directory, "fingerprint.json");
  const log = path.join(directory, "build.log");
  writeFileSync(fingerprint, JSON.stringify({ features: '["default"]' }), {
    mode: 0o600,
  });
  writeFileSync(log, "explicit normal build log", { mode: 0o600 });
  const fileHash = (text) => createHash("sha256").update(text).digest("hex");
  const proof = {
    schemaVersion: 1,
    kind: "single-press-ready-preparation",
    contractId: CONTRACT_ID,
    runId,
    productPid: 1234,
    recipientPid: 2345,
    normalBuild: true,
    installed: true,
    mainWindowClosed: true,
    recipientPrepared: true,
    observedUnixMillis: Date.now(),
    normalBuildProof: {
      artifactSha256: sha,
      buildLogPath: log,
      buildLogSha256: fileHash("explicit normal build log"),
      cargoFingerprintPath: fingerprint,
      cargoFingerprintSha256: fileHash(
        JSON.stringify({ features: '["default"]' }),
      ),
    },
  };
  const preparationProof = path.join(directory, "proof.json");
  const config = {
    schemaVersion: 1,
    contractId: CONTRACT_ID,
    frozen: {
      contractId: CONTRACT_ID,
      runId,
      product: { processId: 1234, runId },
      recipient: { processId: 2345, processStartedUnixMillis: 1000 },
      artifactSha256: sha,
      modelSha256: "2".repeat(64),
      fixtureSha256: PUBLIC_AUDIO_SHA256,
      fixtureDurationMillis: 300000,
      normalBuild: true,
    },
    productReport: path.join(directory, "product.json"),
    recipientReport: path.join(directory, "recipient.json"),
    recipientDirectory: directory,
    reportDirectory: path.join(directory, "run"),
    bindings: {
      artifact: INSTALLED_EXECUTABLE,
      model: path.join(directory, "model"),
      fixture: path.join(directory, "audio"),
    },
    watchdog: {
      executable: path.join(directory, "helper"),
      executableSha256: "4".repeat(64),
      workDirectory: directory,
      newReportDirectory: path.join(directory, "watchdog"),
    },
    preparationProof,
  };
  const configPath = path.join(directory, "config.json");
  const publish = () => {
    writeFileSync(preparationProof, JSON.stringify(proof), { mode: 0o600 });
    writeFileSync(configPath, JSON.stringify(config), { mode: 0o600 });
  };
  publish();
  const calls = [];
  const dependencies = {
    verifyAudio: () => ({
      sha256: PUBLIC_AUDIO_SHA256,
      frames: 4800000,
      durationMillis: 300000,
    }),
    verifyBundle: async () => {
      calls.push("bundle");
      return true;
    },
    verifyBindings: async () => ({ verifyFrozen: async () => true }),
    preflight: async () => {
      calls.push("preflight");
      return { ready: true };
    },
    makeWatchdog: () => ({ preflight: async () => true }),
    makePlayback: () => {
      calls.push("playback-factory");
      return {};
    },
    makeFileAdapters: () => {
      calls.push("run-adapters");
      return { writeReport: async () => {} };
    },
    run: async () => {
      calls.push("run");
      return { state: "failed", gestureConsumed: false };
    },
  };
  return {
    directory,
    config,
    proof,
    configPath,
    publish,
    dependencies,
    calls,
    dispose: () => rmSync(directory, { recursive: true, force: true }),
  };
}

test("prepared but not installed proof must fail without signalling or running", async () => {
  const f = fixture();
  try {
    f.proof.installed = false;
    f.publish();
    await assert.rejects(
      executeSinglePressEntry(["--preflight", f.configPath], f.dependencies),
      /preparation-not-ready/,
    );
    assert.ok(!f.calls.includes("run"));
  } finally {
    f.dispose();
  }
});

test("preflight performs only read-only verification and never creates run adapters", async () => {
  const f = fixture();
  try {
    const result = await executeSinglePressEntry(
      ["--preflight", f.configPath],
      f.dependencies,
    );
    assert.equal(result.ready, true);
    assert.equal(result.gestureConsumed, false);
    assert.deepEqual(f.calls, ["bundle", "preflight"]);
    assert.equal(
      result.normalBuildEvidence.artifactSha256,
      f.config.frozen.artifactSha256,
    );
  } finally {
    f.dispose();
  }
});

test("explicit run wires playback, watchdog cleanup and runner only through controlled test seams", async () => {
  const f = fixture();
  try {
    let requestedCleanup = false;
    const watchdog = {
      preflight: async () => true,
      cleanup: async (frozen) => {
        assert.equal(frozen.runId, f.config.frozen.runId);
        requestedCleanup = true;
        return { clean: true };
      },
    };
    f.dependencies.makeWatchdog = () => watchdog;
    f.dependencies.makeFileAdapters = (options) => {
      assert.equal(options.watchdog, watchdog);
      return { ...options, writeReport: async () => {} };
    };
    f.dependencies.run = async (options) => {
      assert.equal((await options.cleanup(options.frozen)).clean, true);
      assert.ok(options.audio);
      return { state: "failed", gestureConsumed: false };
    };
    const result = await executeSinglePressEntry(
      ["--run", f.configPath],
      f.dependencies,
    );
    assert.equal(result.state, "failed");
    assert.equal(requestedCleanup, true);
    assert.ok(f.calls.includes("playback-factory"));
  } finally {
    f.dispose();
  }
});

test("arguments, unknown config fields and non-installed executable path are rejected", async () => {
  await assert.rejects(executeSinglePressEntry(["--run"]), /arguments-invalid/);
  await assert.rejects(
    executeSinglePressEntry(["--run", "relative.json"]),
    /arguments-invalid/,
  );
  const f = fixture();
  try {
    f.config.extra = "unexpected";
    f.publish();
    await assert.rejects(
      executeSinglePressEntry(["--preflight", f.configPath], f.dependencies),
      /entry-config-invalid/,
    );
    delete f.config.extra;
    f.config.bindings.artifact = path.join(f.directory, "other-binary");
    f.publish();
    await assert.rejects(
      executeSinglePressEntry(["--preflight", f.configPath], f.dependencies),
      /entry-installed-binding-invalid/,
    );
    assert.deepEqual(f.calls, []);
  } finally {
    f.dispose();
  }
});

test("old proof, wrong PID and open main window cannot authorize a signal", async () => {
  for (const field of ["time", "pid", "window"]) {
    const f = fixture();
    try {
      if (field === "time") f.proof.observedUnixMillis -= 30001;
      if (field === "pid") f.proof.productPid++;
      if (field === "window") f.proof.mainWindowClosed = false;
      f.publish();
      await assert.rejects(
        executeSinglePressEntry(["--preflight", f.configPath], f.dependencies),
        /preparation-not-ready/,
      );
      assert.deepEqual(f.calls, []);
    } finally {
      f.dispose();
    }
  }
});

test("default feature fingerprint and exact evidence hashes are mandatory", async () => {
  for (const mode of ["feature", "log", "binary"]) {
    const f = fixture();
    try {
      const proof = f.proof.normalBuildProof;
      if (mode === "feature") {
        const bytes = JSON.stringify({
          features: '["default","test-control"]',
        });
        writeFileSync(proof.cargoFingerprintPath, bytes);
        proof.cargoFingerprintSha256 = createHash("sha256")
          .update(bytes)
          .digest("hex");
      }
      if (mode === "log") writeFileSync(proof.buildLogPath, "changed build");
      if (mode === "binary") proof.artifactSha256 = "f".repeat(64);
      f.publish();
      await assert.rejects(
        executeSinglePressEntry(["--preflight", f.configPath], f.dependencies),
        /normal-build-/,
      );
      assert.deepEqual(f.calls, []);
    } finally {
      f.dispose();
    }
  }
});

test("configuration must be private and cannot be a symlink", async () => {
  const { chmodSync, symlinkSync } = await import("node:fs");
  const f = fixture();
  try {
    chmodSync(f.configPath, 0o644);
    await assert.rejects(
      executeSinglePressEntry(["--preflight", f.configPath], f.dependencies),
      /private-document-invalid/,
    );
    chmodSync(f.configPath, 0o600);
    const link = path.join(f.directory, "linked.json");
    symlinkSync(f.configPath, link);
    await assert.rejects(
      executeSinglePressEntry(["--preflight", link], f.dependencies),
    );
    assert.deepEqual(f.calls, []);
  } finally {
    f.dispose();
  }
});

test("signature, Info bundle identity, microphone entitlement and absent LSEnvironment are verified without launching", async () => {
  const { verifyInstalledBundle } = await import("./run-single-press.mjs");
  const f = fixture();
  try {
    for (const mode of [
      "ok",
      "signature",
      "id",
      "environment",
      "entitlement",
    ]) {
      const calls = [];
      const command = (program, args) => {
        calls.push([program, args]);
        if (program === "/usr/bin/codesign" && args[0] === "--verify") {
          if (mode === "signature")
            throw new Error("controlled invalid signature");
          return "";
        }
        if (program === "/usr/bin/codesign") return "<plist/>";
        if (args.at(-1) === "-")
          return JSON.stringify({
            "com.apple.security.device.audio-input": mode !== "entitlement",
          });
        return JSON.stringify({
          CFBundleIdentifier:
            mode === "id" ? "foreign.app" : "de.hoppworks.everlast-dictation",
          CFBundleExecutable: "everlast-dictation",
          NSMicrophoneUsageDescription: "Local recording",
          ...(mode === "environment" ? { LSEnvironment: {} } : {}),
        });
      };
      if (mode === "ok")
        assert.equal(verifyInstalledBundle(f.config, { command }), true);
      else assert.throws(() => verifyInstalledBundle(f.config, { command }));
      assert.ok(
        calls.every(([program]) =>
          ["/usr/bin/codesign", "/usr/bin/plutil"].includes(program),
        ),
      );
      assert.ok(calls[0][1].includes("--strict"));
    }
  } finally {
    f.dispose();
  }
});

test("failed readiness or an expired preparation assertion cannot enter run factories", async () => {
  for (const mode of ["readiness", "expired"]) {
    const f = fixture();
    try {
      let now = Date.now();
      f.dependencies.now = () => now;
      f.dependencies.preflight = async () => {
        if (mode === "expired") now += 31000;
        return { ready: mode !== "readiness" };
      };
      await assert.rejects(
        executeSinglePressEntry(["--run", f.configPath], f.dependencies),
        mode === "readiness"
          ? /entry-readiness-unconfirmed/
          : /preparation-not-ready/,
      );
      assert.ok(!f.calls.includes("playback-factory"));
      assert.ok(!f.calls.includes("run-adapters"));
      assert.ok(!f.calls.includes("run"));
    } finally {
      f.dispose();
    }
  }
});
