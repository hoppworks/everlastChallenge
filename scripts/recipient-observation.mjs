import { setTimeout as pause } from "node:timers/promises";

// Diagnostic observation only: creates no capture, target approval or recording.
// Actor Idle may precede the recipient's next independent 250ms status tick.
export async function awaitRecipientDelivery({
  read,
  recipientPid,
  afterSequence,
  terminalObservedUnixMillis,
  timeoutMs = 2000,
  now = Date.now,
  wait = pause,
}) {
  if (
    typeof read !== "function" ||
    !Number.isSafeInteger(recipientPid) ||
    recipientPid <= 0 ||
    !Number.isSafeInteger(afterSequence) ||
    afterSequence < 0 ||
    !Number.isSafeInteger(terminalObservedUnixMillis) ||
    !Number.isSafeInteger(timeoutMs) ||
    timeoutMs < 1 ||
    timeoutMs > 5000
  )
    throw new Error("Invalid bounded recipient observation request");
  const deadline = now() + timeoutMs;
  while (true) {
    const value = await read();
    const current = now();
    if (
      value?.schemaVersion !== 1 ||
      value.kind !== "everlast-own-native-recipient-status" ||
      value.processId !== recipientPid ||
      value.running !== true ||
      value.status !== "observed" ||
      value.publicExpectedSentenceId !== "fleurs-de-dev-1517" ||
      !Number.isSafeInteger(value.sequence) ||
      !Number.isSafeInteger(value.observedUnixMillis)
    )
      return { confirmed: false, reason: "invalid-or-changed-recipient" };
    if (
      current <= deadline &&
      value.sequence > afterSequence &&
      value.observedUnixMillis >= terminalObservedUnixMillis &&
      value.observedUnixMillis <= current + 100 &&
      current - value.observedUnixMillis <= 2000 &&
      value.expectedSentenceMatches === true &&
      value.expectedSentenceOccurrences === 1 &&
      value.shortcutCharacterPresent === false
    )
      return {
        confirmed: true,
        recipientPid,
        sequence: value.sequence,
        observedUnixMillis: value.observedUnixMillis,
        reason: "fresh-exact-public-sentence-once",
      };
    if (current >= deadline)
      return { confirmed: false, reason: "observation-timeout" };
    await wait(Math.min(25, deadline - current));
  }
}
