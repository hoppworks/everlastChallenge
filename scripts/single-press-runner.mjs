import {
  constants,
  closeSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { createHash, randomUUID } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const CONTRACT_ID = "single-press-2026-10-19";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const GESTURE_LEDGER_DIRECTORY = path.join(
  root,
  "artifacts",
  CONTRACT_ID,
);
const validRunId = (value) =>
  typeof value === "string" && /^[a-f0-9]{32}$/.test(value);
const hash = (value) =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const integer = (value) => Number.isSafeInteger(value) && value >= 0;
const positive = (value) => integer(value) && value > 0;
function reject(code) {
  throw Object.assign(new Error(code), { code });
}
function privateDirectory(directory, create = false) {
  if (!path.isAbsolute(directory) || path.normalize(directory) !== directory)
    reject("invalid-directory");
  const parent = path.dirname(directory);
  if (realpathSync(parent) !== parent) reject("symlink-directory");
  if (create) {
    try {
      mkdirSync(directory, { mode: 0o700 });
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
    }
  }
  const info = lstatSync(directory);
  if (
    !info.isDirectory() ||
    info.isSymbolicLink() ||
    info.uid !== process.getuid() ||
    (info.mode & 0o777) !== 0o700
  )
    reject("unsafe-directory");
}
function syncDirectory(directory) {
  const fd = openSync(directory, constants.O_RDONLY);
  try {
    fsyncSync(fd);
  } finally {
    closeSync(fd);
  }
}
export function createGestureLedger(directory = GESTURE_LEDGER_DIRECTORY) {
  privateDirectory(directory, true);
  const filename = path.join(directory, "gesture-consumed.json");
  return {
    consume(metadata) {
      if (metadata.contractId !== CONTRACT_ID || !validRunId(metadata.runId))
        reject("invalid-ledger-metadata");
      privateDirectory(directory);
      let fd;
      try {
        fd = openSync(
          filename,
          constants.O_WRONLY |
            constants.O_CREAT |
            constants.O_EXCL |
            constants.O_NOFOLLOW,
          0o600,
        );
      } catch (error) {
        if (error.code === "EEXIST" || error.code === "ELOOP")
          reject("gesture-already-consumed");
        throw error;
      }
      // Even a partial/crashed write consumes the budget. Never unlink or replace it.
      try {
        writeFileSync(
          fd,
          JSON.stringify({
            schemaVersion: 1,
            contractId: CONTRACT_ID,
            runId: metadata.runId,
            consumedUnixMillis: metadata.consumedUnixMillis ?? Date.now(),
          }) + "\n",
        );
        fsyncSync(fd);
      } finally {
        closeSync(fd);
      }
      syncDirectory(directory);
    },
  };
}

function validateFrozen(frozen) {
  if (
    frozen?.contractId !== CONTRACT_ID ||
    !validRunId(frozen.runId) ||
    !positive(frozen.product?.processId) ||
    frozen.product.runId !== frozen.runId ||
    !positive(frozen.recipient?.processId) ||
    frozen.product.processId === frozen.recipient.processId ||
    !positive(frozen.recipient.processStartedUnixMillis) ||
    ![frozen.artifactSha256, frozen.modelSha256, frozen.fixtureSha256].every(
      hash,
    ) ||
    frozen.fixtureDurationMillis !== 300000 ||
    frozen.normalBuild !== true
  )
    reject("invalid-frozen-identity");
}
function fresh(value, now) {
  return (
    integer(value?.sequence) &&
    positive(value.observedUnixMillis) &&
    value.observedUnixMillis <= now + 100 &&
    now - value.observedUnixMillis <= 2000
  );
}
function recipientFocused(r) {
  return (
    r.applicationActive &&
    r.frontmostIsSelf &&
    r.windowKey &&
    r.accessibilityFocused &&
    r.focusedField === "multiline"
  );
}
function validatePair(pair, frozen, now, previous, waiting = false) {
  const p = pair?.product;
  const r = pair?.recipient;
  if (!fresh(p, now) || !fresh(r, now)) reject("stale-observation");
  if (
    p.schemaVersion !== 1 ||
    p.kind !== "everlast-one-press-product-status" ||
    p.contractId !== CONTRACT_ID ||
    p.runId !== frozen.runId ||
    p.processId !== frozen.product.processId ||
    p.observerState !== "observed" ||
    p.running !== true ||
    !integer(p.monotonicSinceLaunchMillis) ||
    ![
      "Idle",
      "Starting",
      "Recording",
      "Finalizing",
      "Delivering",
      "Cancelling",
    ].includes(p.phase) ||
    !(p.sessionId === null || positive(p.sessionId)) ||
    !(
      p.outcome === null ||
      ["local", "empty", "sent", "cancelled", "held", "failed"].includes(
        p.outcome,
      )
    ) ||
    ![
      p.setupReady,
      p.microphoneGranted,
      p.accessibilityGranted,
      p.maximumWarning,
      p.maximumReached,
    ].every((value) => typeof value === "boolean") ||
    !integer(p.elapsedMillis) ||
    !integer(p.transcriptUtf8Bytes) ||
    p.transcriptUtf8Bytes > 262144 ||
    (p.transcriptUtf8Bytes === 0
      ? p.transcriptSha256 !== null
      : !hash(p.transcriptSha256))
  )
    reject("invalid-product-observation");
  if (
    r.schemaVersion !== 1 ||
    r.kind !== "everlast-own-native-recipient-status" ||
    r.processId !== frozen.recipient.processId ||
    r.processStartedUnixMillis !== frozen.recipient.processStartedUnixMillis ||
    r.running !== true ||
    r.status !== "observed" ||
    !integer(r.monotonicSinceLaunchNanos) ||
    ![
      r.applicationActive,
      r.frontmostIsSelf,
      r.windowKey,
      r.accessibilityFocused,
      r.shortcutCharacterPresent,
    ].every((value) => typeof value === "boolean") ||
    !["multiline", "singleline", "none"].includes(r.focusedField) ||
    !integer(r.fieldUtf8Bytes) ||
    !integer(r.totalUtf8Bytes) ||
    r.fieldUtf8Bytes > 262144 ||
    r.totalUtf8Bytes > 524288 ||
    (r.fieldUtf8Bytes === 0 ? r.fieldSha256 !== null : !hash(r.fieldSha256))
  )
    reject("invalid-recipient-observation");
  if (!p.setupReady || !p.microphoneGranted || !p.accessibilityGranted)
    reject("product-not-ready");
  if (!(waiting && p.phase === "Idle") && !recipientFocused(r))
    reject("recipient-focus-lost");
  if (r.shortcutCharacterPresent) reject("shortcut-character-present");
  if (previous) {
    for (const [key, clockKey] of [
      ["product", "monotonicSinceLaunchMillis"],
      ["recipient", "monotonicSinceLaunchNanos"],
    ]) {
      if (
        pair[key].sequence < previous[key].sequence ||
        pair[key][clockKey] < previous[key][clockKey] ||
        pair[key].observedUnixMillis < previous[key].observedUnixMillis
      )
        reject("observation-regressed");
      if (
        pair[key].sequence === previous[key].sequence &&
        JSON.stringify(pair[key]) !== JSON.stringify(previous[key])
      )
        reject("observation-sequence-reused");
    }
  }
  return pair;
}
// Reserve 600 seconds for wait (120), capture guard (330), result (120),
// receipt (5) and cleanup. Exporters are bounded at 900/1800 seconds.
function requireObservationBudget(pair) {
  if (
    pair.product.monotonicSinceLaunchMillis > 300000 ||
    pair.recipient.monotonicSinceLaunchNanos > 1200000000000
  )
    reject("observation-budget-insufficient");
}
const defaultClock = {
  now: () => Date.now(),
  monotonic: () => performance.now(),
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
};

/** Readiness-only seam: no gesture, signal, audio, report write or helper arm. */
export async function preflightSinglePress({
  frozen,
  observe,
  verifyFrozen,
  watchdog,
  clock = defaultClock,
  pollMillis = 250,
}) {
  validateFrozen(frozen);
  if (
    ![
      observe,
      verifyFrozen,
      watchdog?.preflight,
      clock?.now,
      clock?.sleep,
    ].every((value) => typeof value === "function") ||
    !integer(pollMillis) ||
    pollMillis < 10 ||
    pollMillis > 1000
  )
    reject("missing-adapter");
  if (
    (await verifyFrozen(frozen)) !== true ||
    (await watchdog.preflight(frozen)) !== true
  )
    reject("preflight-unverified");
  const first = validatePair(await observe(), frozen, clock.now());
  requireObservationBudget(first);
  if (
    first.product.phase !== "Idle" ||
    first.recipient.totalUtf8Bytes !== 0 ||
    first.recipient.fieldUtf8Bytes !== 0
  )
    reject("preflight-not-idle-empty");
  await clock.sleep(pollMillis);
  const second = validatePair(await observe(), frozen, clock.now(), first);
  if (
    second.product.phase !== "Idle" ||
    second.recipient.totalUtf8Bytes !== 0 ||
    second.recipient.fieldUtf8Bytes !== 0 ||
    second.product.sequence <= first.product.sequence ||
    second.recipient.sequence <= first.recipient.sequence
  )
    reject("preflight-unstable");
  requireObservationBudget(second);
  return { ready: true, observations: second };
}

/** No default microphone, keyboard, prompt, playback or process-control action exists.
 * Integration must provide verified own-component adapters and an independent watchdog.
 * audio.start resolves after player spawn/admission, never after the full playback.
 */
export async function runSinglePress(options) {
  const {
    frozen,
    ledger,
    observe,
    signal,
    audio,
    cleanup,
    writeReport,
    verifyFrozen,
    watchdog,
    clock = defaultClock,
    pollMillis = 250,
  } = options;
  validateFrozen(frozen);
  if (
    ![
      observe,
      signal,
      cleanup,
      writeReport,
      verifyFrozen,
      ledger?.consume,
      audio?.start,
      audio?.stop,
      watchdog?.preflight,
      watchdog?.arm,
      watchdog?.disarm,
      watchdog?.abort,
    ].every((value) => typeof value === "function") ||
    !integer(pollMillis) ||
    pollMillis < 10 ||
    pollMillis > 1000
  )
    reject("missing-adapter");
  const report = {
    schemaVersion: 1,
    kind: "everlast-single-press-run",
    contractId: CONTRACT_ID,
    runId: frozen.runId,
    state: "preparing",
    gestureConsumed: false,
    transportConfirmed: false,
    qualityQualified: false,
    frozen: {
      contractId: CONTRACT_ID,
      runId: frozen.runId,
      product: {
        processId: frozen.product.processId,
        runId: frozen.product.runId,
      },
      recipient: {
        processId: frozen.recipient.processId,
        processStartedUnixMillis: frozen.recipient.processStartedUnixMillis,
      },
      artifactSha256: frozen.artifactSha256,
      modelSha256: frozen.modelSha256,
      fixtureSha256: frozen.fixtureSha256,
      fixtureDurationMillis: frozen.fixtureDurationMillis,
      normalBuild: frozen.normalBuild,
    },
    observations: {},
    limits: [
      "Recipient equality proves transport only, not ASR quality.",
      "Observed transition times are bounds, not invented native event timestamps.",
    ],
  };
  let previous;
  let armed = false;
  let failed = null;
  const emit = async () => {
    report.observedUnixMillis = clock.now();
    await writeReport(structuredClone(report));
  };
  const send = async (state) =>
    signal({
      schemaVersion: 1,
      contractId: CONTRACT_ID,
      runId: frozen.runId,
      state,
      observedUnixMillis: clock.now(),
    });
  const read = async (waiting = false) => {
    previous = validatePair(
      await observe(),
      frozen,
      clock.now(),
      previous,
      waiting,
    );
    return previous;
  };
  const pause = async () => clock.sleep(pollMillis);
  try {
    await emit();
    const preflight = await preflightSinglePress({
      frozen,
      observe,
      verifyFrozen,
      watchdog,
      clock,
      pollMillis,
    });
    previous = preflight.observations;
    report.state = "ready";
    report.observations.ready = { observedUnixMillis: clock.now() };
    await emit();
    if ((await verifyFrozen(frozen)) !== true)
      reject("frozen-identity-changed");
    await watchdog.arm(frozen);
    armed = true;
    const immediate = await read();
    requireObservationBudget(immediate);
    if (
      immediate.product.phase !== "Idle" ||
      immediate.recipient.totalUtf8Bytes !== 0
    )
      reject("preflight-changed-before-prompt");
    // Persist before the first ready label can be displayed. Failure never refunds it.
    await ledger.consume({
      contractId: CONTRACT_ID,
      runId: frozen.runId,
      consumedUnixMillis: clock.now(),
    });
    report.gestureConsumed = true;
    report.state = "waiting-for-start";
    await emit();
    await send("ready");
    const waitingAt = clock.monotonic();
    let startedAt;
    let session;
    let promptAcknowledged = false;
    while (startedAt === undefined) {
      await pause();
      const { product: p, recipient: r } = await read(true);
      const ready = recipientFocused(r);
      await send(ready ? "ready" : "preparing"); // Same consumed gesture and deadline.
      if (!ready) promptAcknowledged = false;
      if (ready && r.signalState === "ready" && r.signalRunId === frozen.runId)
        promptAcknowledged = true;
      if (r.totalUtf8Bytes !== 0) reject("recipient-changed-before-start");
      if (clock.monotonic() - waitingAt > 120000) reject("start-timeout");
      if (["Starting", "Recording"].includes(p.phase)) {
        if (!positive(p.sessionId) || (session && session !== p.sessionId))
          reject("session-changed");
        session = p.sessionId;
        if (p.phase === "Recording" && promptAcknowledged) {
          if (p.elapsedMillis > 2000 || p.maximumReached || p.maximumWarning)
            reject("start-not-observed-cleanly");
          startedAt = clock.monotonic() - p.elapsedMillis;
          report.observations.start = {
            observedUnixMillis: p.observedUnixMillis,
            sessionId: session,
            elapsedMillis: p.elapsedMillis,
          };
        }
      } else if (p.phase !== "Idle") reject("unexpected-start-phase");
    }
    report.state = "running";
    await emit();
    await send("running");
    await audio.start(frozen);
    let stoppedAt;
    while (stoppedAt === undefined) {
      await pause();
      const { product: p, recipient: r } = await read();
      if (p.phase === "Recording" && r.totalUtf8Bytes !== 0)
        reject("unexpected-early-recipient-text");
      await send("running");
      const elapsed = clock.monotonic() - startedAt;
      if (
        elapsed > 330000 ||
        (p.phase === "Recording" && p.elapsedMillis > 330000)
      )
        reject("recording-watchdog");
      if (p.sessionId !== null && p.sessionId !== session)
        reject("session-changed");
      if (p.maximumWarning && !report.observations.warning) {
        if (
          p.phase !== "Recording" ||
          p.elapsedMillis < 269000 ||
          p.elapsedMillis > 275000
        )
          reject("warning-not-observed-on-time");
        report.observations.warning = {
          observedUnixMillis: p.observedUnixMillis,
          elapsedMillis: p.elapsedMillis,
        };
      }
      if (p.phase !== "Recording") {
        if (
          !["Finalizing", "Delivering", "Idle"].includes(p.phase) ||
          !p.maximumReached ||
          !report.observations.warning ||
          elapsed < 299000 ||
          elapsed > 305000
        )
          reject("automatic-stop-not-proven");
        stoppedAt = clock.monotonic();
        report.observations.stop = {
          observedUnixMillis: p.observedUnixMillis,
          elapsedSinceStartMillis: elapsed,
          maximumReached: true,
        };
      }
    }
    await audio.stop();
    report.state = "verifying";
    await emit();
    await send("verifying");
    let terminal = null;
    let terminalAt;
    while (true) {
      const { product: p, recipient: r } = previous;
      if (p.sessionId !== null && p.sessionId !== session)
        reject("session-changed");
      if (!p.maximumReached || !p.maximumWarning) reject("maximum-state-lost");
      if (p.phase === "Idle") {
        if (p.outcome !== "sent" || p.transcriptUtf8Bytes === 0)
          reject("terminal-result-failed");
        const identity = JSON.stringify([
          p.phase,
          p.sessionId,
          p.outcome,
          p.transcriptUtf8Bytes,
          p.transcriptSha256,
        ]);
        if (!terminal) {
          terminal = { identity, observedUnixMillis: p.observedUnixMillis };
          terminalAt = clock.monotonic();
        } else if (terminal.identity !== identity)
          reject("terminal-result-changed");
        if (
          r.observedUnixMillis >= terminal.observedUnixMillis &&
          r.fieldUtf8Bytes === p.transcriptUtf8Bytes &&
          r.totalUtf8Bytes === p.transcriptUtf8Bytes &&
          r.fieldSha256 === p.transcriptSha256 &&
          !r.shortcutCharacterPresent
        ) {
          if ((await verifyFrozen(frozen)) !== true)
            reject("frozen-identity-changed");
          report.transportConfirmed = true;
          report.observations.receipt = {
            observedUnixMillis: r.observedUnixMillis,
            transcriptUtf8Bytes: p.transcriptUtf8Bytes,
            transcriptSha256: p.transcriptSha256,
          };
          break;
        }
        if (clock.monotonic() - terminalAt >= 5000)
          reject("receipt-not-exactly-once");
      } else {
        if (terminal) reject("terminal-result-changed");
        if (!["Finalizing", "Delivering"].includes(p.phase))
          reject("unexpected-result-phase");
      }
      if (clock.monotonic() - stoppedAt > 120000) reject("result-timeout");
      await pause();
      await read();
      await send("verifying");
    }
  } catch (error) {
    failed =
      typeof error?.code === "string" && /^[a-z-]+$/.test(error.code)
        ? error.code
        : "adapter-failed";
  } finally {
    try {
      await audio.stop();
    } catch {
      failed ??= "audio-cleanup-failed";
    }
    let cleanupConfirmed = false;
    try {
      cleanupConfirmed = (await cleanup(frozen))?.clean === true;
      if (!cleanupConfirmed) failed ??= "cleanup-unconfirmed";
    } catch {
      failed ??= "cleanup-failed";
    }
    // Publish the verdict before releasing the independent guard. A failed signal
    // after the prompt must also revoke the product's ability to accept a late key.
    try {
      await send(failed ? "failed" : "passed");
    } catch {
      failed ??= "signal-failed";
    }
    if (armed && failed && report.gestureConsumed) {
      try {
        if ((await watchdog.abort(frozen))?.clean !== true)
          failed ??= "watchdog-cleanup-unconfirmed";
      } catch {
        failed ??= "watchdog-cleanup-failed";
      }
    } else if (armed && cleanupConfirmed) {
      try {
        await watchdog.disarm(frozen);
      } catch {
        failed ??= "watchdog-cleanup-failed";
        if (report.gestureConsumed) {
          try {
            await watchdog.abort(frozen);
          } catch {
            /* Guard remains armed. */
          }
        }
      }
    }
    // A disarm failure can revise an already published success. Revoke the
    // visible success as well; never leave a failed run labelled passed.
    if (failed) {
      try {
        await send("failed");
      } catch {
        /* The report retains the failure. */
      }
    }
    report.state = failed ? "failed" : "passed";
    report.failure = failed;
    await emit();
  }
  return report;
}

/** Bounded own-report reader. Never follows a symlink or blocks opening a FIFO. */
export function readPrivateJson(filename, maximumBytes = 65536) {
  privateDirectory(path.dirname(filename));
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const info = fstatSync(fd);
    if (
      !info.isFile() ||
      info.uid !== process.getuid() ||
      (info.mode & 0o777) !== 0o600 ||
      info.size > maximumBytes
    )
      reject("unsafe-report-file");
    const bytes = Buffer.alloc(maximumBytes + 1);
    let used = 0;
    while (used < bytes.length) {
      const count = readSync(fd, bytes, used, bytes.length - used, null);
      if (!count) break;
      used += count;
    }
    if (used > maximumBytes) reject("oversized-report");
    let value;
    try {
      value = JSON.parse(
        new TextDecoder("utf-8", { fatal: true }).decode(
          bytes.subarray(0, used),
        ),
      );
    } catch {
      reject("invalid-report-json");
    }
    if (!value || typeof value !== "object" || Array.isArray(value))
      reject("invalid-report-json");
    return value;
  } finally {
    closeSync(fd);
  }
}
function createPrivateWriter(directory, basename, maximumBytes) {
  privateDirectory(directory);
  const filename = path.join(directory, basename);
  let owned = null;
  return (value) => {
    privateDirectory(directory);
    try {
      const current = lstatSync(filename);
      if (
        !owned ||
        !current.isFile() ||
        current.uid !== process.getuid() ||
        (current.mode & 0o777) !== 0o600 ||
        current.dev !== owned.dev ||
        current.ino !== owned.ino
      )
        reject("output-file-replaced");
    } catch (error) {
      if (error.code !== "ENOENT" || owned) throw error;
    }
    const bytes = Buffer.from(JSON.stringify(value) + "\n");
    if (bytes.length > maximumBytes) reject("oversized-output");
    const temporary = path.join(directory, `.single-press-${randomUUID()}.tmp`);
    let fd;
    let failure;
    try {
      fd = openSync(
        temporary,
        constants.O_WRONLY |
          constants.O_CREAT |
          constants.O_EXCL |
          constants.O_NOFOLLOW,
        0o600,
      );
      writeFileSync(fd, bytes);
      fsyncSync(fd);
      const completed = fd;
      fd = undefined;
      closeSync(completed);
      renameSync(temporary, filename);
      owned = lstatSync(filename);
      syncDirectory(directory);
    } catch (error) {
      failure = error;
    } finally {
      if (fd !== undefined) {
        try {
          closeSync(fd);
        } catch (error) {
          failure ??= error;
        }
      }
      try {
        unlinkSync(temporary);
      } catch (error) {
        if (error.code !== "ENOENT") failure ??= error;
      }
    }
    if (failure) throw failure;
  };
}

export function createRecipientSignalAdapter(directory, runId) {
  if (!validRunId(runId)) reject("invalid-signal-identity");
  const write = createPrivateWriter(directory, "signal.json", 1024);
  return async (value) => {
    if (
      value.schemaVersion !== 1 ||
      value.contractId !== CONTRACT_ID ||
      value.runId !== runId ||
      !positive(value.observedUnixMillis) ||
      ![
        "preparing",
        "ready",
        "running",
        "verifying",
        "passed",
        "failed",
      ].includes(value.state)
    )
      reject("invalid-signal");
    write({
      schemaVersion: 1,
      contractId: CONTRACT_ID,
      runId,
      state: value.state,
      observedUnixMillis: value.observedUnixMillis,
    });
  };
}
function fileBinding(filename) {
  if (
    !path.isAbsolute(filename) ||
    realpathSync(path.dirname(filename)) !== path.dirname(filename)
  )
    reject("invalid-binding-path");
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const before = fstatSync(fd);
    if (
      !before.isFile() ||
      before.uid !== process.getuid() ||
      before.size > 1073741824
    )
      reject("invalid-binding-file");
    const digest = createHash("sha256");
    const bytes = Buffer.alloc(65536);
    let count;
    while ((count = readSync(fd, bytes, 0, bytes.length, null)) !== 0)
      digest.update(bytes.subarray(0, count));
    const after = fstatSync(fd);
    if (
      before.size !== after.size ||
      before.mtimeMs !== after.mtimeMs ||
      before.ino !== after.ino ||
      before.dev !== after.dev
    )
      reject("binding-changed-during-read");
    return {
      sha256: digest.digest("hex"),
      size: after.size,
      mtimeMs: after.mtimeMs,
      ino: after.ino,
      dev: after.dev,
    };
  } finally {
    closeSync(fd);
  }
}

/** Actual guarded filesystem adapters; audio/cleanup/watchdog remain explicit integrations.
 * Creating these adapters does not show a signal, start audio, or change the ledger.
 */
export function createFileAdapters({
  frozen,
  productReport,
  recipientReport,
  recipientDirectory,
  reportDirectory,
  bindings,
  audio,
  cleanup,
  watchdog,
}) {
  validateFrozen(frozen);
  if (
    !bindings ||
    Object.keys(bindings).sort().join() !== "artifact,fixture,model"
  )
    reject("missing-file-bindings");
  privateDirectory(reportDirectory, true);
  const write = createPrivateWriter(reportDirectory, "report.json", 65536);
  const bound = {};
  for (const key of ["artifact", "model", "fixture"]) {
    bound[key] = fileBinding(bindings[key]);
    if (bound[key].sha256 !== frozen[`${key}Sha256`])
      reject("binding-hash-mismatch");
  }
  return {
    frozen,
    ledger: createGestureLedger(),
    observe: async () => ({
      product: readPrivateJson(productReport),
      recipient: readPrivateJson(recipientReport),
    }),
    signal: createRecipientSignalAdapter(recipientDirectory, frozen.runId),
    audio,
    cleanup,
    watchdog,
    writeReport: async (report) => write(report),
    verifyFrozen: async () => {
      try {
        process.kill(frozen.product.processId, 0);
        process.kill(frozen.recipient.processId, 0);
      } catch {
        return false;
      }
      return ["artifact", "model", "fixture"].every((key) => {
        const info = lstatSync(bindings[key]);
        return (
          info.isFile() &&
          info.uid === process.getuid() &&
          info.dev === bound[key].dev &&
          info.ino === bound[key].ino &&
          info.size === bound[key].size &&
          info.mtimeMs === bound[key].mtimeMs
        );
      });
    },
  };
}
