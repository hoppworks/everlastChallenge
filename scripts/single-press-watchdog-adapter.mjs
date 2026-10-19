import {
  constants,
  closeSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  openSync,
  readSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import path from "node:path";

const CONTRACT = "single-press-2026-10-19";
const REASONS = [
  "waitStartTimeout",
  "recordingTimeout",
  "processingTimeout",
  "sessionChanged",
  "lifetimeExceeded",
  "parentLost",
  "productLost",
  "requestedAbort",
  "requestInvalid",
  "observationInvalid",
  "observationRegressed",
  "prematureDisarm",
];
const runIdValid = (value) =>
  typeof value === "string" && /^[a-f0-9]{32}$/.test(value);
const hashValid = (value) =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const positive = (value) => Number.isSafeInteger(value) && value > 0;
function reject(code) {
  throw Object.assign(new Error(code), { code });
}
function canonical(filename) {
  if (
    !path.isAbsolute(filename) ||
    path.normalize(filename) !== filename ||
    realpathSync(path.dirname(filename)) !== path.dirname(filename)
  )
    reject("watchdog-path-invalid");
}
function privateDirectory(directory) {
  canonical(directory);
  const info = lstatSync(directory);
  if (
    !info.isDirectory() ||
    info.uid !== process.getuid() ||
    (info.mode & 0o777) !== 0o700
  )
    reject("watchdog-directory-invalid");
  return info;
}
function readPrivate(filename, limit) {
  canonical(filename);
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
      info.size > limit
    )
      reject("watchdog-file-invalid");
    const bytes = Buffer.alloc(limit + 1);
    let used = 0;
    while (used < bytes.length) {
      const count = readSync(fd, bytes, used, bytes.length - used, null);
      if (!count) break;
      used += count;
    }
    if (used > limit) reject("watchdog-file-oversized");
    return {
      bytes: bytes.subarray(0, used),
      identity: { dev: info.dev, ino: info.ino },
    };
  } finally {
    closeSync(fd);
  }
}
function json(filename, limit = 4096) {
  try {
    return JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(
        readPrivate(filename, limit).bytes,
      ),
    );
  } catch {
    reject("watchdog-report-unreadable");
  }
}
function writeNew(filename, value) {
  privateDirectory(path.dirname(filename));
  const fd = openSync(
    filename,
    constants.O_WRONLY |
      constants.O_CREAT |
      constants.O_EXCL |
      constants.O_NOFOLLOW,
    0o600,
  );
  try {
    writeFileSync(fd, JSON.stringify(value) + "\n");
    fsyncSync(fd);
  } finally {
    closeSync(fd);
  }
  const directory = openSync(path.dirname(filename), constants.O_RDONLY);
  try {
    fsyncSync(directory);
  } finally {
    closeSync(directory);
  }
}
function executableBinding(filename) {
  canonical(filename);
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const info = fstatSync(fd);
    if (
      !info.isFile() ||
      info.uid !== process.getuid() ||
      !(info.mode & 0o100) ||
      info.size > 32 * 1024 * 1024
    )
      reject("watchdog-executable-invalid");
    const hash = createHash("sha256");
    const bytes = Buffer.alloc(65536);
    let total = 0;
    let count;
    while ((count = readSync(fd, bytes, 0, bytes.length, null)) !== 0) {
      total += count;
      if (total > 32 * 1024 * 1024) reject("watchdog-executable-oversized");
      hash.update(bytes.subarray(0, count));
    }
    const after = fstatSync(fd);
    if (
      total !== info.size ||
      after.mtimeMs !== info.mtimeMs ||
      after.size !== info.size
    )
      reject("watchdog-executable-changed");
    return {
      sha256: hash.digest("hex"),
      dev: info.dev,
      ino: info.ino,
      size: info.size,
      mtimeMs: info.mtimeMs,
    };
  } finally {
    closeSync(fd);
  }
}
const defaultClock = {
  now: () => Date.now(),
  monotonic: () => performance.now(),
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
};
const fresh = (value, now) =>
  positive(value?.observedUnixMillis) &&
  value.observedUnixMillis <= now + 100 &&
  now - value.observedUnixMillis <= 2000;

/** The helper is a separate process group. This adapter never signals the product PID. */
export function createWatchdogAdapter({
  executable,
  executableSha256,
  productReport,
  workDirectory,
  newReportDirectory,
  spawnHelper = spawn,
  clock = defaultClock,
}) {
  let frozenIdentity = null;
  let productOwner = null;
  let reportOwner = null;
  let child = null;
  let spawned = false;
  let closed = false;
  let exitCode = null;
  let exitSignal = null;
  let childError = false;
  let previousReport = null;
  let previousProduct = null;
  let disarmWritten = false;
  let abortWritten = false;
  let attempted = false;
  const bind = (frozen) => {
    if (
      frozen?.contractId !== CONTRACT ||
      !runIdValid(frozen.runId) ||
      !positive(frozen.product?.processId) ||
      frozen.product.runId !== frozen.runId
    )
      reject("watchdog-frozen-invalid");
    const value = { runId: frozen.runId, productPid: frozen.product.processId };
    if (
      frozenIdentity &&
      (value.runId !== frozenIdentity.runId ||
        value.productPid !== frozenIdentity.productPid)
    )
      reject("watchdog-frozen-changed");
    frozenIdentity ??= value;
    return value;
  };
  const owner = (directory, expected) => {
    const info = privateDirectory(directory);
    const marker = readPrivate(path.join(directory, "owner"), 32);
    if (marker.bytes.toString("utf8") !== frozenIdentity.runId)
      reject("watchdog-owner-mismatch");
    const found = {
      directoryDev: info.dev,
      directoryIno: info.ino,
      markerDev: marker.identity.dev,
      markerIno: marker.identity.ino,
    };
    if (
      expected &&
      Object.keys(found).some((key) => found[key] !== expected[key])
    )
      reject("watchdog-owner-replaced");
    return found;
  };
  const product = () => {
    productOwner = owner(path.dirname(productReport), productOwner);
    const value = json(productReport);
    if (
      value?.schemaVersion !== 1 ||
      value.kind !== "everlast-one-press-product-status" ||
      value.contractId !== CONTRACT ||
      value.runId !== frozenIdentity.runId ||
      value.processId !== frozenIdentity.productPid ||
      value.observerState !== "observed" ||
      value.running !== true ||
      !positive(value.sequence) ||
      !fresh(value, clock.now())
    )
      reject("watchdog-product-unconfirmed");
    if (
      ![
        "Idle",
        "Starting",
        "Recording",
        "Finalizing",
        "Delivering",
        "Cancelling",
      ].includes(value.phase) ||
      !(value.sessionId === null || positive(value.sessionId))
    )
      reject("watchdog-product-invalid");
    if (
      previousProduct &&
      (value.sequence < previousProduct.sequence ||
        value.observedUnixMillis < previousProduct.observedUnixMillis ||
        (value.sequence === previousProduct.sequence &&
          JSON.stringify(value) !== JSON.stringify(previousProduct)))
    )
      reject("watchdog-product-regressed");
    previousProduct = value;
    return value;
  };
  const readReport = () => {
    reportOwner = owner(newReportDirectory, reportOwner);
    const value = json(path.join(newReportDirectory, "snapshot.json"));
    if (
      value?.schemaVersion !== 1 ||
      value.kind !== "everlast-single-press-watchdog" ||
      value.contractId !== CONTRACT ||
      value.runId !== frozenIdentity.runId ||
      value.parentPid !== process.pid ||
      value.productPid !== frozenIdentity.productPid ||
      value.processId !== child?.pid ||
      !positive(value.sequence) ||
      !fresh(value, clock.now()) ||
      ![
        "ready",
        "waiting-for-start",
        "recording",
        "processing",
        "verifying",
        "disarmed",
        "aborted",
        "failed",
      ].includes(value.state) ||
      ![
        value.armed,
        value.startObserved,
        value.cleanupAttempted,
        value.cleanupConfirmed,
      ].every((field) => typeof field === "boolean") ||
      !(value.reason === null || REASONS.includes(value.reason))
    )
      reject("watchdog-report-invalid");
    const terminal = ["disarmed", "aborted", "failed"].includes(value.state);
    if (terminal ? value.armed : !value.armed)
      reject("watchdog-report-invalid");
    if (value.state === "disarmed" || !terminal) {
      if (
        value.cleanupAttempted ||
        value.cleanupConfirmed ||
        value.reason !== null
      )
        reject("watchdog-report-invalid");
    } else if (
      !value.cleanupAttempted ||
      value.reason === null ||
      (value.state === "aborted"
        ? value.reason !== "requestedAbort"
        : value.reason === "requestedAbort")
    )
      reject("watchdog-report-invalid");
    if (
      previousReport &&
      (value.sequence < previousReport.sequence ||
        value.observedUnixMillis < previousReport.observedUnixMillis ||
        (value.sequence === previousReport.sequence &&
          JSON.stringify(value) !== JSON.stringify(previousReport)))
    )
      reject("watchdog-report-regressed");
    previousReport = value;
    return value;
  };
  const waitFor = async (predicate) => {
    const deadline = clock.monotonic() + 5000;
    while (clock.monotonic() < deadline) {
      let value;
      try {
        value = readReport();
      } catch {
        /* A missing/transient report never constitutes success. */
      }
      if (value && predicate(value)) return value;
      if (closed && !value) reject("watchdog-helper-lost");
      await clock.sleep(25);
    }
    reject("watchdog-ack-timeout");
  };
  const writeRequest = (name) => {
    owner(newReportDirectory, reportOwner);
    writeNew(path.join(newReportDirectory, name), {
      runId: frozenIdentity.runId,
    });
  };
  const api = {
    async preflight(frozen) {
      try {
        bind(frozen);
        if (!hashValid(executableSha256)) return false;
        privateDirectory(workDirectory);
        canonical(newReportDirectory);
        if (
          path.dirname(newReportDirectory) !== workDirectory ||
          path.basename(productReport) !== "snapshot.json"
        )
          return false;
        try {
          lstatSync(newReportDirectory);
          return false;
        } catch (error) {
          if (error.code !== "ENOENT") return false;
        }
        if (executableBinding(executable).sha256 !== executableSha256)
          return false;
        const status = product();
        return status.phase === "Idle" && status.sessionId === null;
      } catch {
        return false;
      }
    },
    async arm(frozen) {
      bind(frozen);
      if (attempted) reject("watchdog-already-attempted");
      if ((await api.preflight(frozen)) !== true)
        reject("watchdog-preflight-failed");
      attempted = true;
      const config = path.join(
        workDirectory,
        `watchdog-${frozenIdentity.runId}.json`,
      );
      writeNew(config, {
        schemaVersion: 1,
        contractId: CONTRACT,
        runId: frozenIdentity.runId,
        parentPid: process.pid,
        productPid: frozenIdentity.productPid,
        productReport,
      });
      try {
        child = spawnHelper(
          executable,
          ["--config", config, "--report-dir", newReportDirectory],
          {
            detached: true,
            shell: false,
            stdio: ["ignore", "ignore", "ignore"],
          },
        );
      } catch {
        childError = true;
        closed = true;
        reject("watchdog-spawn-failed");
      }
      child.once("spawn", () => {
        spawned = true;
      });
      child.once("error", () => {
        childError = true;
      });
      child.once("close", (code, signal) => {
        closed = true;
        exitCode = code;
        exitSignal = signal ?? null;
      });
      try {
        await waitFor((value) => {
          if (
            closed ||
            childError ||
            ["failed", "aborted", "disarmed"].includes(value.state)
          )
            reject("watchdog-arm-failed");
          return (
            spawned &&
            value.armed &&
            ["ready", "waiting-for-start"].includes(value.state) &&
            !value.cleanupAttempted &&
            !value.cleanupConfirmed
          );
        });
      } catch (error) {
        try {
          if ((await api.cleanup(frozen)).clean && !closed)
            await api.disarm(frozen);
        } catch {
          /* No successful arm is claimed; an unconfirmed guard stays protective. */
        }
        throw error;
      }
      return { armed: true, processId: child.pid };
    },
    async disarm(frozen) {
      bind(frozen);
      const status = product();
      if (status.phase !== "Idle" || status.sessionId !== null)
        reject("watchdog-disarm-not-idle");
      if (!child || childError) reject("watchdog-helper-lost");
      if (!disarmWritten) {
        writeRequest("disarm.json");
        disarmWritten = true;
      }
      await waitFor(
        (value) =>
          value.state === "disarmed" &&
          value.armed === false &&
          closed &&
          exitCode === 0 &&
          exitSignal === null,
      );
      return { disarmed: true };
    },
    async abort(frozen) {
      return api.cleanup(frozen, { terminateIdle: true });
    },
    async cleanup(frozen, { terminateIdle = false } = {}) {
      try {
        bind(frozen);
        try {
          const status = product();
          if (
            !terminateIdle &&
            status.phase === "Idle" &&
            status.sessionId === null
          )
            return { clean: true };
        } catch {
          /* No assumption of cleanup from a failed read. */
        }
        if (!child || childError) return { clean: false };
        try {
          const final = readReport();
          if (
            closed &&
            exitCode === 1 &&
            exitSignal === null &&
            ["aborted", "failed"].includes(final.state) &&
            final.cleanupAttempted &&
            final.cleanupConfirmed &&
            !final.armed
          )
            return { clean: true };
        } catch {
          /* Still try the explicitly bound helper's abort path. */
        }
        if (closed) return { clean: false };
        if (!abortWritten) {
          writeRequest("abort.json");
          abortWritten = true;
        }
        await waitFor(
          (value) =>
            ["aborted", "failed"].includes(value.state) &&
            value.cleanupAttempted &&
            value.cleanupConfirmed &&
            !value.armed &&
            closed &&
            exitCode === 1 &&
            exitSignal === null,
        );
        return { clean: true };
      } catch {
        return { clean: false };
      }
    },
  };
  return api;
}
