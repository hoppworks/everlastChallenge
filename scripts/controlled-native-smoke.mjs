// Developer-only, one public sentence; importing never records or plays audio.
import { awaitRecipientDelivery } from "./recipient-observation.mjs";
import { requestControl } from "./test-control.mjs";
import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  constants,
  openSync,
  fstatSync,
  readFileSync,
  readSync,
  closeSync,
  writeFileSync,
  renameSync,
  mkdirSync,
  realpathSync,
  lstatSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
export const PUBLIC_FIXTURE = Object.freeze({
  id: "fleurs-de-dev-1517",
  sha256: "b892cf9dc80c437f0a78c6f261d83dda196252e31a1e415032981472d3ea9137",
  durationMs: 10560,
});
const idle = (s) =>
  s?.phase === "Idle" &&
  s.sessionId === null &&
  s.controlledSessionId === null &&
  !s.controlledStartPending;

/** Caller prepares private endpoint/recipient and supplies bounded operations.
 * No process launch, activation, permission request, or physical key is implied.
 */
export async function runControlledNativeSmoke({
  controlDir,
  recipientPid,
  endpointStartedAt,
  fixture,
  command,
  readRecipient,
  play,
  writeReport,
  now = Date.now,
  sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  signal,
}) {
  const report = {
    schemaVersion: 1,
    kind: "controlled-native-public-smoke-v1",
    passed: false,
    physicalHotkey: false,
    microphoneRecording: false,
    sessionId: null,
    fixture: PUBLIC_FIXTURE,
    recipientPid,
    cleanupConfirmed: false,
    status: "running",
    events: [],
  };
  const began = now();
  let deadline = began + 40000;
  let cleaning = false;
  const call = async (op, session) => {
    if (signal?.aborted && !cleaning) throw new Error("interrupted");
    const remaining = Math.floor(deadline - now());
    if (remaining < 10) throw new Error("deadline");
    const reply = await command({
      controlDir,
      op,
      ...(session === undefined ? {} : { session }),
      timeoutMs: Math.min(3000, remaining),
    });
    report.events.push({
      op,
      session: session ?? null,
      observedUnixMillis: now(),
      ok: reply?.ok === true,
      snapshot: reply?.snapshot ?? null,
    });
    if (!reply?.ok) {
      const error = new Error("command-rejected");
      error.rejected = true;
      throw error;
    }
    return reply.snapshot;
  };
  const poll = async (predicate, timeoutMs) => {
    const end = Math.min(deadline, now() + timeoutMs);
    for (;;) {
      const value = await call("status");
      if (now() <= end && predicate(value)) return value;
      if (now() >= end) throw new Error("bounded-observation-expired");
      await sleep(Math.min(25, end - now()));
    }
  };
  let acknowledged = false;
  let stage = "preflight";
  try {
    if (
      !Number.isSafeInteger(recipientPid) ||
      recipientPid <= 0 ||
      !Number.isSafeInteger(endpointStartedAt) ||
      endpointStartedAt > began ||
      began - endpointStartedAt > 12 * 60 * 1000
    )
      throw new Error("fresh-endpoint-and-recipient-required");
    if (
      fixture?.id !== PUBLIC_FIXTURE.id ||
      fixture.sha256 !== PUBLIC_FIXTURE.sha256 ||
      fixture.durationMs !== PUBLIC_FIXTURE.durationMs ||
      typeof fixture.path !== "string"
    )
      throw new Error("fixed-public-fixture-required");
    const initial = await call("status");
    if (
      !idle(initial) ||
      !initial.setupReady ||
      !initial.microphoneGranted ||
      !initial.accessibilityGranted
    )
      throw new Error("not-ready-idle");
    const recipient = await readRecipient();
    if (
      recipient?.schemaVersion !== 1 ||
      recipient.kind !== "everlast-own-native-recipient-status" ||
      recipient.processId !== recipientPid ||
      recipient.running !== true ||
      recipient.status !== "observed" ||
      recipient.publicExpectedSentenceId !== PUBLIC_FIXTURE.id ||
      !Number.isSafeInteger(recipient.sequence) ||
      recipient.sequence < 0 ||
      !Number.isSafeInteger(recipient.observedUnixMillis) ||
      now() - recipient.observedUnixMillis < -100 ||
      now() - recipient.observedUnixMillis > 2000 ||
      recipient.frontmostIsSelf !== true ||
      recipient.applicationActive !== true ||
      recipient.windowKey !== true ||
      !["multiline", "singleline"].includes(recipient.focusedField) ||
      recipient.expectedSentenceOccurrences !== 0 ||
      recipient.expectedSentenceMatches !== false ||
      recipient.shortcutCharacterPresent !== false
    )
      throw new Error("fresh-empty-own-recipient-required");
    report.initialRecipient = {
      sequence: recipient.sequence,
      observedUnixMillis: recipient.observedUnixMillis,
      processId: recipientPid,
    };
    await writeReport(structuredClone(report));
    stage = "start";
    report.startAttempted = true;
    await writeReport(structuredClone(report));
    await call("start-host");
    acknowledged = true;
    stage = "recording";
    const recording = await poll(
      (s) =>
        s.phase === "Recording" &&
        Number.isSafeInteger(s.controlledSessionId) &&
        s.controlledSessionId > 0 &&
        s.controlledSessionId === s.sessionId,
      5000,
    );
    report.sessionId = recording.controlledSessionId;
    report.microphoneRecording = true;
    report.recordingObservedUnixMillis = now();
    stage = "playback";
    if (now() + fixture.durationMs + 1500 > deadline)
      throw new Error("insufficient-playback-budget");
    await writeReport(structuredClone(report));
    report.playbackStartedUnixMillis = now();
    await play(fixture.path, { timeoutMs: fixture.durationMs + 1000, signal });
    report.playbackFinishedUnixMillis = now();
    await sleep(500);
    stage = "stop";
    await call("stop", report.sessionId);
    stage = "terminal";
    report.terminal = await poll(idle, 20000);
    report.terminalObservedUnixMillis = now();
    await writeReport(structuredClone(report));
    stage = "recipient";
    report.recipientConfirmation = await awaitRecipientDelivery({
      read: readRecipient,
      recipientPid,
      afterSequence: recipient.sequence,
      terminalObservedUnixMillis: report.terminalObservedUnixMillis,
      now,
      wait: sleep,
      timeoutMs: 2000,
    });
    report.cleanupConfirmed = idle(report.terminal);
    report.passed =
      report.cleanupConfirmed &&
      report.terminal.outcome === "sent" &&
      report.terminal.publicSentenceMatches &&
      report.terminal.actionError === false &&
      report.terminal.transcriptCharacters > 0 &&
      report.recipientConfirmation.confirmed;
    report.status = report.passed ? "passed" : "failed";
  } catch (error) {
    report.passed = false;
    report.unknownStart = stage === "start" && !acknowledged && !error.rejected;
    report.status = ["preflight", "start"].includes(stage)
      ? "blocked"
      : "failed";
    report.failureStage = stage;
    report.error = /^[a-z][a-z0-9-]{0,80}$/.test(error?.message ?? "")
      ? error.message
      : `${stage}-failed`;
  } finally {
    cleaning = true;
    deadline = Math.min(began + 45000, now() + 5000);
    try {
      const current = await call("status");
      report.cleanupConfirmed = idle(current);
      if (
        !report.cleanupConfirmed &&
        acknowledged &&
        report.sessionId !== null &&
        current.sessionId === report.sessionId &&
        current.controlledSessionId === report.sessionId
      ) {
        await call("cancel", report.sessionId);
        report.cleanupConfirmed = idle(
          await poll(idle, Math.max(10, deadline - now())),
        );
      }
    } catch {
      report.cleanupConfirmed = false;
    }
    if (!report.cleanupConfirmed) report.passed = false;
  }
  if (!report.cleanupConfirmed && report.status === "passed")
    report.status = "failed";
  report.finishedAtUnixMillis = now();
  report.elapsedMs = now() - began;
  await writeReport(structuredClone(report));
  return report;
}

function ownedBytes(filename, maxBytes) {
  if (!path.isAbsolute(filename))
    throw new Error("absolute-owned-file-required");
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const info = fstatSync(fd);
    if (
      !info.isFile() ||
      info.uid !== process.getuid() ||
      info.size > maxBytes ||
      (info.mode & 0o077) !== 0
    )
      throw new Error("private-owned-regular-file-required");
    const bytes = Buffer.alloc(maxBytes + 1);
    let length = 0;
    while (length < bytes.length) {
      const count = readSync(fd, bytes, length, bytes.length - length, null);
      if (!count) break;
      length += count;
    }
    if (length > maxBytes) throw new Error("owned-file-size-limit");
    return bytes.subarray(0, length);
  } finally {
    closeSync(fd);
  }
}

export function loadPublicFixture(filename) {
  const bytes = ownedBytes(filename, 512 * 1024);
  if (digest(bytes) !== PUBLIC_FIXTURE.sha256)
    throw new Error("pinned-public-fixture-hash-required");
  return { ...PUBLIC_FIXTURE, path: filename };
}

function playPublicAudio(filename, { timeoutMs, signal }) {
  return new Promise((resolve, reject) => {
    const child = spawn("/usr/bin/afplay", [filename], {
      stdio: "ignore",
      shell: false,
    });
    let failed = false;
    const stop = () => {
      failed = true;
      child.kill("SIGKILL");
    };
    const timer = setTimeout(stop, timeoutMs);
    signal?.addEventListener("abort", stop, { once: true });
    if (signal?.aborted) stop();
    child.once("error", () => {
      failed = true;
    });
    child.once("close", (code) => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", stop);
      if (code === 0 && !failed) resolve();
      else reject(new Error("public-playback-failed"));
    });
  });
}

export async function main(args) {
  const names = [
    "--control-dir",
    "--recipient-report",
    "--fixture",
    "--output-dir",
    "--endpoint-started-at",
  ];
  const options = {};
  if (args.length !== names.length * 2)
    throw new Error(
      "Usage: controlled-native-smoke --control-dir ABS --recipient-report ABS --fixture ABS --output-dir NEW_ABS --endpoint-started-at ISO",
    );
  for (let i = 0; i < args.length; i += 2) {
    if (
      !names.includes(args[i]) ||
      Object.hasOwn(options, args[i]) ||
      !args[i + 1]
    )
      throw new Error("Usage: explicit smoke options required");
    options[args[i]] = args[i + 1];
  }
  for (const name of names.filter((name) => name !== "--endpoint-started-at"))
    if (!path.isAbsolute(options[name]))
      throw new Error("Usage: absolute paths required");
  const endpointStartedAt = Date.parse(options["--endpoint-started-at"]);
  if (!Number.isSafeInteger(endpointStartedAt))
    throw new Error("Usage: endpoint start timestamp required");
  const controlDir = options["--control-dir"];
  const info = lstatSync(controlDir);
  if (
    !info.isDirectory() ||
    info.isSymbolicLink() ||
    realpathSync(controlDir) !== controlDir ||
    info.uid !== process.getuid() ||
    (info.mode & 0o077) !== 0
  )
    throw new Error("private-canonical-control-directory-required");
  const configBytes = ownedBytes(path.join(controlDir, "config.json"), 256);
  const config = JSON.parse(configBytes);
  if (
    Object.keys(config).length !== 1 ||
    !Number.isSafeInteger(config.recipientPid) ||
    config.recipientPid <= 0
  )
    throw new Error("fixed-own-recipient-config-required");
  const fixture = loadPublicFixture(options["--fixture"]);
  const output = options["--output-dir"];
  if (realpathSync(path.dirname(output)) !== path.dirname(output))
    throw new Error("canonical-output-parent-required");
  mkdirSync(output, { mode: 0o700 }); // Existing evidence is never overwritten.
  const bindings = {
    configSha256: digest(configBytes),
    fixtureSha256: fixture.sha256,
    fixturePath: fixture.path,
    controlDir,
    endpointStartedAt,
    recipientReport: options["--recipient-report"],
    sourceHashes: {},
  };
  for (const filename of [
    "./controlled-native-smoke.mjs",
    "./test-control.mjs",
    "./recipient-observation.mjs",
  ])
    bindings.sourceHashes[filename] = digest(
      readFileSync(new URL(filename, import.meta.url)),
    );
  let observations = 0;
  const controller = new AbortController();
  const abort = () => controller.abort();
  process.on("SIGINT", abort);
  process.on("SIGTERM", abort);
  const writeReport = async (report) => {
    const temporary = path.join(output, `${randomUUID()}.tmp`);
    writeFileSync(
      temporary,
      JSON.stringify({ ...report, bindings }, null, 2) + "\n",
      { flag: "wx", mode: 0o600 },
    );
    renameSync(temporary, path.join(output, "result.json"));
  };
  try {
    return await runControlledNativeSmoke({
      controlDir,
      recipientPid: config.recipientPid,
      endpointStartedAt,
      fixture,
      command: requestControl,
      play: playPublicAudio,
      writeReport,
      signal: controller.signal,
      readRecipient: async () => {
        // Kernel liveness is additional to report PID/freshness and server AX identity.
        process.kill(config.recipientPid, 0);
        const bytes = ownedBytes(options["--recipient-report"], 65536);
        writeFileSync(
          path.join(
            output,
            `recipient-${String(++observations).padStart(4, "0")}.json`,
          ),
          bytes,
          { flag: "wx", mode: 0o600 },
        );
        return JSON.parse(bytes);
      },
    });
  } finally {
    process.off("SIGINT", abort);
    process.off("SIGTERM", abort);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const result = await main(process.argv.slice(2));
    console.log(
      JSON.stringify({
        passed: result.passed,
        status: result.status,
        cleanupConfirmed: result.cleanupConfirmed,
        physicalHotkey: false,
      }),
    );
    process.exitCode = result.passed ? 0 : 1;
  } catch {
    console.error(
      "Controlled public smoke blocked; no start retry, permission request or user input. Inspect prepared paths and private evidence.",
    );
    process.exitCode = 1;
  }
}
