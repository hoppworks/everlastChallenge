/** Explicit-command benchmark owner, with bounded lifetime and power observations. */
import { spawn, spawnSync } from "node:child_process";
import { closeSync, openSync } from "node:fs";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { check } from "./tooling/audio-data.ts";
import { writeNew } from "./tooling/io.ts";
import { reapOwned } from "./tooling/owned-probe.ts";
export interface Power {
  utc: string;
  source: "AC Power" | "Battery Power";
  policy: string;
  batteryReadout: string;
}
export function powerSnapshot(): Power {
  const read = (arg: string) => {
    const result = spawnSync("/usr/bin/pmset", ["-g", arg], {
      timeout: 5000,
      maxBuffer: 16384,
      encoding: "utf8",
    });
    check(!result.error && result.status === 0, "Power observation failed");
    return result.stdout;
  };
  const batteryReadout = read("batt"),
    policy = read("custom"),
    match = /^Now drawing from '(AC Power|Battery Power)'/.exec(batteryReadout);
  check(match && policy.trim(), "Unrecognized power source/policy");
  return {
    utc: new Date().toISOString(),
    source: match[1] as Power["source"],
    policy,
    batteryReadout,
  };
}
export function samePower(rows: Power[], requirePair = true): void {
  const first = rows[0];
  check(
    first &&
      rows.length >= (requirePair ? 2 : 1) &&
      ["AC Power", "Battery Power"].includes(first.source) &&
      first.policy.trim() &&
      rows.every(
        (row) => row.source === first.source && row.policy === first.policy,
      ),
    "Missing or changed power observations",
  );
}
interface Options {
  command: string[];
  folder: string;
  revision: string;
  timeoutMs?: number;
  sampleIntervalMs?: number;
  signal?: AbortSignal;
  power?: () => Power;
  graceMs?: number;
  observationMs?: number;
}
export async function owned(options: Options) {
  const { command, folder, revision, signal } = options;
  const timeout = options.timeoutMs ?? 6000000,
    interval = options.sampleIntervalMs ?? 5000;
  check(
    command.length > 0 &&
      command[0]!.length > 0 &&
      command.every((x) => typeof x === "string") &&
      timeout > 0 &&
      Number.isFinite(timeout) &&
      interval > 0 &&
      Number.isFinite(interval),
    "Invalid command or time bound",
  );
  check(!signal?.aborted, "Cancelled before admission");
  const power = options.power ?? powerSnapshot,
    observations = [power()];
  samePower(observations, false);
  const fd = openSync(path.join(folder, "run.log"), "wx", 0o600);
  let child: ReturnType<typeof spawn> | undefined;
  let launchError: string | null = null,
    operationError: string | null = null,
    watchdogExpired = false;
  try {
    check(!signal?.aborted, "Cancelled before launch");
    child = spawn(command[0]!, command.slice(1), {
      cwd: path.resolve(fileURLToPath(new URL("..", import.meta.url))),
      stdio: ["ignore", fd, fd],
      detached: true,
      env: {
        ...process.env,
        EVERLAST_BENCHMARK_REVISION: revision,
        EVERLAST_MLX_SEGMENT_DIAGNOSTICS: "1",
      },
    });
    child.on("error", (error) => {
      launchError = error.message;
    });
    writeNew(path.join(folder, "started.json"), {
      command,
      wrapperPid: child.pid ?? null,
      beforePower: observations[0],
      sourceRevision: revision,
    });
    const deadline = performance.now() + timeout;
    let next = performance.now() + interval;
    while (
      child.exitCode === null &&
      child.signalCode === null &&
      !launchError &&
      !signal?.aborted
    ) {
      if (performance.now() >= deadline) {
        watchdogExpired = true;
        break;
      }
      await delay(Math.min(50, Math.max(0, deadline - performance.now())));
      if (performance.now() >= next) {
        observations.push(power());
        samePower(observations, false);
        next = performance.now() + interval;
      }
    }
  } catch (error) {
    operationError =
      error instanceof Error ? error.message : "Probe operation failed";
  } finally {
    closeSync(fd);
  }
  const cleanup = child
    ? await reapOwned(child, options.graceMs, options.observationMs)
    : {
        exitCode: null,
        signal: null,
        processReaped: false,
        terminationConfirmed: false,
        forcedGroupCleanup: false,
        cleanupObservationErrors: [],
      };
  if (cleanup.terminationConfirmed) {
    try {
      observations.push(power());
      samePower(observations);
    } catch (error) {
      operationError =
        error instanceof Error
          ? error.message
          : "Final power observation failed";
    }
  }
  const qualified =
    cleanup.exitCode === 0 &&
    cleanup.terminationConfirmed &&
    !cleanup.forcedGroupCleanup &&
    !signal?.aborted &&
    !watchdogExpired &&
    !launchError &&
    !operationError;
  const report = {
    ...cleanup,
    qualified,
    command,
    sourceRevision: revision,
    startedUtc: observations[0]!.utc,
    finishedUtc: new Date().toISOString(),
    cancelled: signal?.aborted ?? false,
    watchdogExpired,
    launchError,
    operationError,
    powerObservations: observations,
  };
  writeNew(path.join(folder, "owner-completion.json"), report);
  return report;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const args = process.argv.slice(2),
    separator = args.indexOf("--"),
    folder = args[0],
    revision = args[1];
  if (separator !== 2 || !folder || !revision || !args[3]) {
    console.error(
      "Usage: controlled-probe.ts NEW_EXISTING_FOLDER REVISION -- COMMAND [ARGS...]",
    );
    process.exitCode = 2;
  } else {
    const controller = new AbortController(),
      stop = () => controller.abort();
    process.on("SIGINT", stop);
    process.on("SIGTERM", stop);
    owned({
      folder,
      revision,
      command: args.slice(3),
      signal: controller.signal,
    })
      .then((result) => {
        if (!result.qualified) process.exitCode = 1;
      })
      .catch(() => {
        console.error("Probe admission or receipt failed");
        process.exitCode = 2;
      })
      .finally(() => {
        process.off("SIGINT", stop);
        process.off("SIGTERM", stop);
      });
  }
}
