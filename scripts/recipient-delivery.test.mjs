import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { observeFiles } from "./recipient-delivery.mjs";
test("file entry point binds actor completion and fresh receiver observation", async () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "everlast-delivery-"));
  try {
    const attempt = path.join(dir, "attempt.json");
    const report = path.join(dir, "report.json");
    const terminal = Date.now();
    writeFileSync(
      attempt,
      JSON.stringify({
        recordingStarted: true,
        recipientPid: process.pid,
        endedAtUtc: new Date(terminal).toISOString(),
        recipient: { sequence: 10 },
        terminal: {
          phase: "Idle",
          outcome: "sent",
          publicSentenceMatches: true,
        },
      }),
    );
    writeFileSync(
      report,
      JSON.stringify({
        schemaVersion: 1,
        kind: "everlast-own-native-recipient-status",
        processId: process.pid,
        sequence: 11,
        observedUnixMillis: terminal,
        running: true,
        status: "observed",
        publicExpectedSentenceId: "fleurs-de-dev-1517",
        expectedSentenceMatches: true,
        expectedSentenceOccurrences: 1,
        shortcutCharacterPresent: false,
      }),
    );
    const result = await observeFiles(report, attempt);
    assert.equal(result.confirmed, true);
    assert.equal(result.afterSequence, 10);
    assert.equal(result.terminalObservedUnixMillis, terminal);
    assert.match(result.attemptSha256, /^[a-f0-9]{64}$/);
    assert.match(result.helperSha256, /^[a-f0-9]{64}$/);
    writeFileSync(attempt, JSON.stringify({ recordingStarted: false }));
    await assert.rejects(
      observeFiles(report, attempt),
      /Controlled completion required/,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
