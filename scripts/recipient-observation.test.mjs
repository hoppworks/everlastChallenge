import test from "node:test";
import assert from "node:assert/strict";
import { awaitRecipientDelivery } from "./recipient-observation.mjs";
const sample = (changes = {}) => ({
  schemaVersion: 1,
  kind: "everlast-own-native-recipient-status",
  processId: 42,
  sequence: 11,
  observedUnixMillis: 1000,
  running: true,
  status: "observed",
  publicExpectedSentenceId: "fleurs-de-dev-1517",
  expectedSentenceMatches: true,
  expectedSentenceOccurrences: 1,
  shortcutCharacterPresent: false,
  ...changes,
});
const simulate = async (snapshots) => {
  let clock = 1000;
  let reads = 0;
  const result = await awaitRecipientDelivery({
    read: () => snapshots[Math.min(reads++, snapshots.length - 1)],
    recipientPid: 42,
    afterSequence: 10,
    terminalObservedUnixMillis: 1000,
    timeoutMs: 100,
    now: () => clock,
    wait: async (ms) => {
      clock += ms;
    },
  });
  return { result, reads };
};
test("actor completion does not reject the previous recipient tick", async () => {
  const { result, reads } = await simulate([
    sample({
      sequence: 10,
      observedUnixMillis: 990,
      expectedSentenceMatches: false,
    }),
    sample({ sequence: 11, observedUnixMillis: 1025 }),
  ]);
  assert.equal(result.confirmed, true);
  assert.equal(reads, 2);
});
test("old success cannot confirm a new delivery", async () => {
  const { result } = await simulate([
    sample({ sequence: 10, observedUnixMillis: 990 }),
  ]);
  assert.equal(result.confirmed, false);
});
test("wrong text, duplicates and shortcut characters never confirm", async () => {
  for (const change of [
    { expectedSentenceMatches: false },
    { expectedSentenceOccurrences: 2 },
    { shortcutCharacterPresent: true },
    { observedUnixMillis: 999 },
    { observedUnixMillis: 2000 },
  ]) {
    assert.equal((await simulate([sample(change)])).result.confirmed, false);
  }
});
test("changed recipient, stopped owner and read failure cannot confirm", async () => {
  for (const change of [
    { processId: 99 },
    { running: false },
    { schemaVersion: 2 },
  ]) {
    assert.equal((await simulate([sample(change)])).result.confirmed, false);
  }
  await assert.rejects(
    awaitRecipientDelivery({
      read: () => {
        throw new Error("read failed");
      },
      recipientPid: 42,
      afterSequence: 10,
      terminalObservedUnixMillis: 1000,
    }),
    /read failed/,
  );
});
