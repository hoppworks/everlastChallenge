import test from "node:test";
import assert from "node:assert/strict";
import {
  runControlledNativeSmoke,
  loadPublicFixture,
  main,
} from "./controlled-native-smoke.mjs";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

function simulation(overrides = {}) {
  let clock = 100000;
  let phase = "Idle";
  let started = false;
  const calls = [];
  const reports = [];
  const snapshot = () => ({
    setupReady: true,
    microphoneGranted: true,
    accessibilityGranted: true,
    phase,
    sessionId: phase === "Recording" ? 7 : null,
    controlledSessionId: phase === "Recording" ? 7 : null,
    controlledStartPending: false,
    outcome: started && phase === "Idle" ? "sent" : null,
    publicSentenceMatches: started && phase === "Idle",
    transcriptCharacters: started ? 130 : 0,
    actionError: false,
  });
  const recipient = () => ({
    schemaVersion: 1,
    kind: "everlast-own-native-recipient-status",
    processId: process.pid,
    sequence: started && phase === "Idle" ? 2 : 1,
    observedUnixMillis: clock,
    running: true,
    status: "observed",
    publicExpectedSentenceId: "fleurs-de-dev-1517",
    expectedSentenceMatches: started && phase === "Idle",
    expectedSentenceOccurrences: started && phase === "Idle" ? 1 : 0,
    shortcutCharacterPresent: false,
    applicationActive: true,
    frontmostIsSelf: true,
    windowKey: true,
    focusedField: "multiline",
  });
  return {
    calls,
    reports,
    options: {
      controlDir: "/private/test-control",
      recipientPid: process.pid,
      endpointStartedAt: 99000,
      fixture: {
        path: "/public.wav",
        id: "fleurs-de-dev-1517",
        sha256:
          "b892cf9dc80c437f0a78c6f261d83dda196252e31a1e415032981472d3ea9137",
        durationMs: 10560,
      },
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
      command: async ({ op, session }) => {
        calls.push([op, session]);
        if (op === "start-host") {
          started = true;
          phase = "Recording";
        }
        if (op === "stop" || op === "cancel") phase = "Idle";
        return { ok: true, snapshot: snapshot() };
      },
      readRecipient: async () => recipient(),
      play: async () => {
        calls.push(["play"]);
        clock += 10560;
      },
      writeReport: async (report) => {
        reports.push(structuredClone(report));
      },
      ...overrides,
    },
  };
}

test("one controlled start plays only the pinned public fixture, stops owned SID and confirms fresh recipient", async () => {
  const s = simulation();
  const result = await runControlledNativeSmoke(s.options);
  assert.equal(result.passed, true);
  assert.equal(result.physicalHotkey, false);
  assert.equal(result.microphoneRecording, true);
  assert.equal(result.cleanupConfirmed, true);
  assert.equal(result.sessionId, 7);
  assert.deepEqual(
    s.calls.filter(([op]) =>
      ["start-host", "play", "stop", "cancel"].includes(op),
    ),
    [["start-host", undefined], ["play"], ["stop", 7]],
  );
  assert.equal(result.recipientConfirmation.confirmed, true);
  assert.equal(s.reports.at(-1).passed, true);
});

test("unknown start reply never retries, adopts a SID or cancels an unacknowledged session", async () => {
  const s = simulation();
  const original = s.options.command;
  s.options.command = async (request) => {
    const reply = await original(request);
    if (request.op === "start-host") throw new Error("lost-reply");
    return reply;
  };
  const result = await runControlledNativeSmoke(s.options);
  assert.equal(result.status, "blocked");
  assert.equal(result.unknownStart, true);
  assert.equal(result.cleanupConfirmed, false);
  assert.equal(result.sessionId, null);
  assert.equal(s.calls.filter(([op]) => op === "start-host").length, 1);
  assert.equal(
    s.calls.some(([op]) => ["cancel", "stop", "play"].includes(op)),
    false,
  );
});

test("waits boundedly for actual Recording and terminal Idle; playback failure cancels only its known session", async () => {
  const s = simulation();
  const original = s.options.command;
  let afterStart = 0;
  s.options.command = async (request) => {
    const reply = await original(request);
    if (request.op === "start-host") afterStart = 1;
    else if (request.op === "status" && afterStart++ === 1)
      return { ok: true, snapshot: { ...reply.snapshot, phase: "Starting" } };
    return reply;
  };
  const result = await runControlledNativeSmoke(s.options);
  assert.equal(result.passed, true);
  const failed = simulation({
    play: async () => {
      throw new Error("playback failed");
    },
  });
  const failedResult = await runControlledNativeSmoke(failed.options);
  assert.equal(failedResult.passed, false);
  assert.equal(failedResult.cleanupConfirmed, true);
  assert.deepEqual(
    failed.calls.filter(([op]) => op === "cancel"),
    [["cancel", 7]],
  );
});

test("stale endpoint, wrong public fixture and stale/nonempty recipient block before any start", async () => {
  for (const mutate of [
    (s) => {
      s.options.endpointStartedAt = -1000000;
    },
    (s) => {
      s.options.fixture = { ...s.options.fixture, sha256: "0".repeat(64) };
    },
    (s) => {
      const read = s.options.readRecipient;
      s.options.readRecipient = async () => ({
        ...(await read()),
        observedUnixMillis: 1,
      });
    },
    (s) => {
      const read = s.options.readRecipient;
      s.options.readRecipient = async () => ({
        ...(await read()),
        expectedSentenceOccurrences: 1,
      });
    },
  ]) {
    const s = simulation();
    mutate(s);
    const result = await runControlledNativeSmoke(s.options);
    assert.equal(result.status, "blocked");
    assert.equal(result.passed, false);
    assert.equal(
      s.calls.some(([op]) => op === "start-host"),
      false,
    );
  }
});

test("never cancels a changed foreign SID and never confirms stale recipient success", async () => {
  const s = simulation({
    play: async () => {
      throw new Error("failed");
    },
  });
  const original = s.options.command;
  s.options.command = async (request) => {
    const reply = await original(request);
    if (
      request.op === "status" &&
      s.calls.filter(([op]) => op === "status").length >= 3
    )
      return {
        ok: true,
        snapshot: {
          ...reply.snapshot,
          sessionId: 99,
          controlledSessionId: null,
        },
      };
    return reply;
  };
  const result = await runControlledNativeSmoke(s.options);
  assert.equal(result.cleanupConfirmed, false);
  assert.equal(
    s.calls.some(([op]) => op === "cancel"),
    false,
  );
  const stale = simulation();
  const read = stale.options.readRecipient;
  stale.options.readRecipient = async () => ({
    ...(await read()),
    sequence: 1,
  });
  const staleResult = await runControlledNativeSmoke(stale.options);
  assert.equal(staleResult.passed, false);
  assert.equal(staleResult.recipientConfirmation.confirmed, false);
});

test("file entry rejects unpinned audio and unknown CLI options before any native action", async (t) => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "native-smoke-test-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const audio = path.join(dir, "not-public.wav");
  await writeFile(audio, "private or unverified audio", { mode: 0o600 });
  assert.throws(() => loadPublicFixture(audio), /public-fixture/);
  await assert.rejects(main(["--arbitrary-target", "123"]), /Usage/);
});

test("terminal timeout and interruption both end boundedly and cancel only the acknowledged SID", async () => {
  const s = simulation();
  const original = s.options.command;
  let stopped = false,
    cancelled = false;
  s.options.command = async (request) => {
    const reply = await original(request);
    if (request.op === "stop") stopped = true;
    if (request.op === "cancel") cancelled = true;
    if (request.op === "status" && stopped && !cancelled)
      return {
        ok: true,
        snapshot: {
          ...reply.snapshot,
          phase: "Finalizing",
          sessionId: 7,
          controlledSessionId: 7,
        },
      };
    return reply;
  };
  const result = await runControlledNativeSmoke(s.options);
  assert.equal(result.passed, false);
  assert.equal(result.cleanupConfirmed, true);
  assert.ok(result.elapsedMs <= 45000);
  assert.deepEqual(
    s.calls.filter(([op]) => op === "cancel"),
    [["cancel", 7]],
  );
  const controller = new AbortController();
  const interrupted = simulation({
    signal: controller.signal,
    play: async () => {
      controller.abort();
      throw new Error("interrupted");
    },
  });
  const aborted = await runControlledNativeSmoke(interrupted.options);
  assert.equal(aborted.passed, false);
  assert.equal(aborted.cleanupConfirmed, true);
  assert.deepEqual(
    interrupted.calls.filter(([op]) => op === "cancel"),
    [["cancel", 7]],
  );
});
