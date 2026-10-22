/** A detached child owns its process group; denied observation never proves absence. */
import type { ChildProcess } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
export interface ObservationError {
  operation: string;
  code: string;
  message: string;
}
type SendSignal = (pid: number, signal: NodeJS.Signals | 0) => boolean;
function errorRecord(operation: string, error: unknown): ObservationError {
  return {
    operation,
    code:
      error instanceof Error && "code" in error
        ? String(error.code)
        : "UNKNOWN",
    message: error instanceof Error ? error.message : "Unknown process error",
  };
}
export async function reapOwned(
  child: ChildProcess,
  graceMs = 10000,
  observationMs = 2000,
  send: SendSignal = process.kill,
) {
  const errors: ObservationError[] = [];
  const pid = child.pid;
  const reaped = () => child.exitCode !== null || child.signalCode !== null;
  const signal = (value: NodeJS.Signals) => {
    if (pid === undefined) return;
    try {
      send(-pid, value);
    } catch (error) {
      if (!(
        error instanceof Error &&
        "code" in error &&
        error.code === "ESRCH"
      ))
        errors.push(errorRecord("signalGroup", error));
    }
  };
  const observe = async (): Promise<boolean | null> => {
    if (pid === undefined) return null;
    const deadline = performance.now() + observationMs;
    for (;;) {
      let state: boolean | null = null;
      try {
        send(-pid, 0);
        state = true;
      } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "ESRCH")
          return false;
        errors.push(errorRecord("observeGroup", error));
      }
      if (performance.now() >= deadline) return state;
      await delay(Math.min(10, Math.max(0, deadline - performance.now())));
    }
  };
  const wait = async () => {
    const deadline = performance.now() + graceMs;
    while (!reaped() && performance.now() < deadline) await delay(10);
  };
  if (!reaped()) {
    signal("SIGTERM");
    await wait();
  }
  let present = await observe();
  const forced = present === true;
  if (forced) {
    signal("SIGKILL");
    await wait();
    present = await observe();
  }
  return {
    exitCode: child.exitCode,
    signal: child.signalCode,
    processReaped: reaped(),
    terminationConfirmed: present === false && reaped(),
    forcedGroupCleanup: forced,
    cleanupObservationErrors: errors,
  };
}
