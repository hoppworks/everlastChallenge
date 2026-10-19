// Developer-only host start measurement. Importing never records or dispatches.
import { createHash } from "node:crypto";
import { readFileSync, lstatSync, realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { requestControl } from "./test-control.mjs";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const CENTRAL = path.join(
  ROOT,
  "tests/fixtures/corpus/central-reference-v1.json",
);
const SHA = /^[a-f0-9]{64}$/;
const count = (v) => Number.isSafeInteger(v) && v >= 0;
const failureCode = (error) =>
  /^[a-z0-9-]{1,80}$/.test(error?.message ?? "")
    ? error.message
    : "campaign-operation-failed";
function checkedCases(cases) {
  if (!Array.isArray(cases) || cases.length !== 6)
    throw new Error("six-prebound-central-cases-required");
  const expected = ["de-5", "de-10", "de-15", "en-5", "en-10", "en-15"];
  if (
    new Set(cases.map((c) => c.id)).size !== 6 ||
    cases.some(
      (c, i) =>
        `${c.language}-${c.seconds}` !== expected[i] ||
        !SHA.test(c.audioSha256),
    )
  )
    throw new Error("central-case-binding-invalid");
  return cases.map((c) => ({
    id: c.id,
    language: c.language,
    seconds: c.seconds,
    audioSha256: c.audioSha256,
  }));
}
export function loadCentralStartCases() {
  const bytes = readFileSync(CENTRAL);
  if (bytes.length > 256 * 1024) throw new Error("manifest-bound");
  const manifest = JSON.parse(bytes);
  const cases = manifest.cases.map((c) => ({
    id: c.id,
    language: c.language,
    seconds: c.audio.frames / 16000,
    audioSha256: c.audio.sha256,
  }));
  checkedCases(cases);
  for (const c of manifest.cases) {
    if (
      c.class !== "clean" ||
      c.audio.sampleRate !== 16000 ||
      !c.tags.includes(`central-${c.audio.frames / 16000}s`)
    )
      throw new Error("central-case-evidence-invalid");
    const filename = path.resolve(ROOT, c.audio.path);
    if (!filename.startsWith(`${ROOT}/artifacts/corpus/audio/`))
      throw new Error("central-case-path-invalid");
    const info = lstatSync(filename);
    if (
      !info.isFile() ||
      info.isSymbolicLink() ||
      realpathSync(filename) !== filename ||
      info.size > 1024 * 1024
    )
      throw new Error("central-audio-bound");
    if (
      createHash("sha256").update(readFileSync(filename)).digest("hex") !==
      c.audio.sha256
    )
      throw new Error("central-audio-hash-mismatch");
  }
  return {
    cases,
    manifestSha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

/** Exactly one warmup and 30 starts per prebound future-input case. No audio is
 * played and no case duration/language changes the actual default-mic start.
 * Caller supplies existing authorized developer process/controlDir and durable
 * writeReport; this function never launches a GUI, changes rights or presses keys.
 */
export async function runHostStartCampaign({
  controlDir,
  cases = loadCentralStartCases().cases,
  command = requestControl,
  now = () => performance.now(),
  sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  writeReport,
} = {}) {
  if (typeof writeReport !== "function")
    throw new Error("explicit-report-writer-required");
  const bound = checkedCases(cases);
  const report = {
    schemaVersion: 1,
    kind: "controlled-host-start-campaign-v1",
    passed: false,
    physicalHotkey: false,
    exactCpalLatency: false,
    futureInputOnly: true,
    actualInput:
      "system-default microphone; immediately cancelled after confirmed start",
    actualConfiguration:
      "unchanged ready product settings; no per-case language/device reconfiguration",
    maximumObservationWaitMillis: 5000,
    requiredPerCase: 30,
    warmups: 1,
    trials: [],
    cases: bound.map((c) => ({ ...c, medianUpperMillis: null })),
    cleanupConfirmed: false,
    error: null,
  };
  let owned = null;
  let startAcknowledged = false;
  const call = async (op, session) => {
    const result = await command({
      controlDir,
      op,
      ...(session === undefined ? {} : { session }),
    });
    if (!result?.ok) throw new Error("command-rejected");
    return result.snapshot;
  };
  const wait = async (action, predicate, code) => {
    const deadline = now() + 5000;
    do {
      const value = await action();
      if (predicate(value)) return value;
      await sleep(25);
    } while (now() < deadline);
    throw new Error(code);
  };
  const status = () => call("status");
  const idle = (s) =>
    s.phase === "Idle" &&
    s.sessionId === null &&
    s.controlledSessionId === null &&
    !s.controlledStartPending;
  const observe = async () => {
    const o = await call("host-observation");
    if (
      o.kind !== "controlled-host-start-observation-v1" ||
      o.physicalHotkey !== false ||
      o.exactCpalLatency !== false ||
      !count(o.recordCount) ||
      o.recordCount > 181 ||
      o.counterError ||
      o.observerContentionOrPoison ||
      o.overflowAttempts !== 0
    )
      throw new Error("observer-invalid");
    return o;
  };
  const cleanup = async () => {
    const current = await status();
    if (idle(current)) return true;
    // Unknown/rejected requests and endpoint-global SIDs grant no authority.
    // Only a positively acknowledged dispatch plus its exact completed observer
    // ordinal can freeze a SID owned by this campaign.
    if (
      !startAcknowledged ||
      owned === null ||
      current.sessionId !== owned ||
      current.controlledSessionId !== owned
    )
      return false;
    await call("cancel", owned);
    await wait(status, idle, "cleanup-idle-unconfirmed");
    owned = null;
    return true;
  };
  try {
    const initial = await status();
    if (
      !idle(initial) ||
      !initial.setupReady ||
      !initial.microphoneGranted ||
      !initial.accessibilityGranted
    )
      throw new Error("not-ready-idle");
    const baseline = await observe();
    if (baseline.recordCount !== 0 || baseline.pendingOrdinal !== null)
      throw new Error("observer-not-fresh");
    await writeReport(structuredClone(report));
    const sequence = [
      { caseIndex: 0, warmup: true },
      ...bound.flatMap((_, caseIndex) =>
        Array.from({ length: 30 }, () => ({ caseIndex, warmup: false })),
      ),
    ];
    for (const item of sequence) {
      const trial = {
        ordinal: report.trials.length + 1,
        caseId: bound[item.caseIndex].id,
        warmup: item.warmup,
        sessionId: null,
        observedUpperNanos: null,
        ackOffsetNanos: null,
        error: null,
      };
      report.trials.push(trial);
      try {
        const before = await status();
        if (!idle(before)) throw new Error("trial-not-idle");
        startAcknowledged = false;
        await call("start-host"); // never retry a start, including unknown command outcome
        startAcknowledged = true;
        const recording = await wait(
          status,
          (s) =>
            s.phase === "Recording" &&
            s.controlledSessionId > 0 &&
            s.sessionId === s.controlledSessionId,
          "owned-recording-unconfirmed",
        );
        const candidate = recording.controlledSessionId;
        const observation = await wait(
          observe,
          (o) => o.recordCount >= trial.ordinal,
          "host-observation-missing",
        );
        const r = observation.lastRecord;
        if (
          observation.recordCount !== trial.ordinal ||
          observation.pendingOrdinal !== null ||
          r?.ordinal !== trial.ordinal ||
          r.session !== candidate ||
          r.reason !== null ||
          !count(r.observedUpperNanos) ||
          !count(r.ackOffsetNanos)
        )
          throw new Error("host-record-not-matching");
        owned = candidate;
        trial.sessionId = owned;
        trial.observedUpperNanos = r.observedUpperNanos;
        trial.ackOffsetNanos = r.ackOffsetNanos;
        await call("cancel", owned);
        await wait(status, idle, "trial-cleanup-unconfirmed");
        owned = null;
        startAcknowledged = false;
      } catch (error) {
        trial.error = failureCode(error);
        throw error;
      }
      await writeReport(structuredClone(report));
    }
    for (const c of report.cases) {
      const values = report.trials
        .filter((t) => !t.warmup && t.caseId === c.id)
        .map((t) => t.observedUpperNanos / 1e6)
        .sort((a, b) => a - b);
      if (values.length !== 30) throw new Error("trial-count-mismatch");
      c.medianUpperMillis = (values[14] + values[15]) / 2;
    }
    report.passed = report.cases.every((c) => c.medianUpperMillis <= 250);
    if (!report.passed) report.error = "start-median-exceeds-250ms";
  } catch (error) {
    report.error = failureCode(error);
    report.passed = false;
  } finally {
    try {
      report.cleanupConfirmed = await cleanup();
    } catch {
      report.cleanupConfirmed = false;
      report.passed = false;
      report.error ??= "cleanup-unconfirmed";
    }
    if (!report.cleanupConfirmed) {
      report.passed = false;
      report.error ??= "cleanup-unconfirmed";
    }
    await writeReport(structuredClone(report));
  }
  return report;
}
