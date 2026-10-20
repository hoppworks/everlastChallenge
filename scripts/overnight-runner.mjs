import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  readFileSync,
  readSync,
  writeFileSync,
  mkdirSync,
  openSync,
  closeSync,
  renameSync,
  statSync,
  realpathSync,
  unlinkSync,
} from "node:fs";
import path from "node:path";

const hash = (value) => createHash("sha256").update(value).digest("hex");
const runnerHash = hash(readFileSync(new URL(import.meta.url)));
const cleanupReserveMs = 1000;
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
let interrupted;
let hardDeadline = Infinity;
const onSigint = () => {
  interrupted = "SIGINT";
};
const onSigterm = () => {
  interrupted = "SIGTERM";
};
function hashFile(filename, checkDeadline = true) {
  const digest = createHash("sha256");
  const fd = openSync(filename, "r");
  try {
    const chunk = Buffer.allocUnsafe(1024 * 1024);
    for (;;) {
      if (checkDeadline && Date.now() >= hardDeadline)
        throw new Error("Deadline reached while hashing declared files");
      const length = readSync(fd, chunk);
      if (!length) break;
      digest.update(chunk.subarray(0, length));
    }
    return digest.digest("hex");
  } finally {
    closeSync(fd);
  }
}
function files(cwd, names, checkDeadline = true) {
  return names.map((name) => {
    const filename = path.resolve(cwd, name);
    if (!statSync(filename).isFile())
      throw new Error(`Expected explicit regular file: ${filename}`);
    return [
      filename,
      realpathSync(filename),
      statSync(filename).mode,
      hashFile(filename, checkDeadline),
    ];
  });
}
function readJSON(file, fallback) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    if (error.code === "ENOENT") return fallback;
    throw error;
  }
}
function ordered(jobs) {
  const remaining = new Map(jobs.map((job) => [job.id, job]));
  const done = new Set();
  const result = [];
  while (remaining.size) {
    const next = [...remaining.values()].find((job) =>
      job.dependsOn.every((id) => done.has(id)),
    );
    if (!next) throw new Error("Missing dependency or dependency cycle");
    result.push(next);
    remaining.delete(next.id);
    done.add(next.id);
  }
  return result;
}
function validate(plan) {
  const strings = (value) =>
    Array.isArray(value) &&
    value.every(
      (item) =>
        typeof item === "string" && item.length > 0 && !item.includes("\0"),
    );
  if (
    plan.schemaVersion !== 1 ||
    !Array.isArray(plan.jobs) ||
    plan.jobs.length === 0 ||
    typeof plan.deadline !== "string" ||
    !/(Z|[+-]\d\d:\d\d)$/.test(plan.deadline) ||
    !Number.isFinite(Date.parse(plan.deadline))
  )
    throw new Error(
      "Expected schemaVersion 1, nonempty jobs and absolute deadline with timezone",
    );
  const ids = new Set();
  for (const job of plan.jobs) {
    if (
      !job ||
      typeof job.id !== "string" ||
      !/^[a-zA-Z0-9][a-zA-Z0-9_-]{0,79}$/.test(job.id) ||
      ids.has(job.id)
    )
      throw new Error("Job IDs must be unique safe names");
    ids.add(job.id);
    if (
      Object.keys(job).some(
        (key) =>
          ![
            "id",
            "argv",
            "cwd",
            "timeoutMs",
            "dependsOn",
            "inputs",
            "outputs",
            "env",
            "resourceGroup",
          ].includes(key),
      )
    )
      throw new Error(`Unknown job field for ${job.id}`);
    if (
      !Array.isArray(job.argv) ||
      !job.argv.length ||
      job.argv.some((arg) => typeof arg !== "string" || arg.includes("\0")) ||
      !path.isAbsolute(job.argv[0]) ||
      !path.isAbsolute(job.cwd ?? "") ||
      !statSync(job.cwd).isDirectory() ||
      !Number.isSafeInteger(job.timeoutMs) ||
      job.timeoutMs <= 0 ||
      job.timeoutMs > 2147483647 ||
      !strings(job.dependsOn) ||
      !strings(job.inputs) ||
      job.inputs.length === 0 ||
      !strings(job.outputs)
    )
      throw new Error(
        `Invalid argv/cwd/timeout/dependencies/explicit files for ${job.id}`,
      );
    if (
      /^(sh|bash|zsh|dash|fish|csh|ksh|cmd|powershell|pwsh)(\.exe)?$/.test(
        path.basename(job.argv[0]),
      )
    )
      throw new Error("Shell jobs are not supported; declare executable argv");
    if (
      job.env !== undefined &&
      (!job.env ||
        Array.isArray(job.env) ||
        typeof job.env !== "object" ||
        Object.entries(job.env).some(
          ([name, value]) =>
            !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) ||
            typeof value !== "string" ||
            value.includes("\0"),
        ))
    )
      throw new Error(`Invalid environment for ${job.id}`);
    if (
      job.resourceGroup !== undefined &&
      (typeof job.resourceGroup !== "string" ||
        !/^[a-zA-Z0-9_-]+$/.test(job.resourceGroup))
    )
      throw new Error(`Invalid resource group for ${job.id}`);
  }
  return ordered(plan.jobs);
}
function save(file, value) {
  const temporary = `${file}.${randomUUID()}.tmp`;
  writeFileSync(temporary, JSON.stringify(value, null, 2) + "\n", {
    mode: 0o600,
  });
  renameSync(temporary, file);
}
async function execute(job, log, timeoutMs, onSpawn) {
  const fd = openSync(log, "wx", 0o600);
  let child;
  try {
    child = spawn(job.argv[0], job.argv.slice(1), {
      cwd: job.cwd,
      env: { ...process.env, ...job.env },
      stdio: ["ignore", fd, fd],
      shell: false,
      detached: process.platform !== "win32",
    });
  } finally {
    closeSync(fd);
  }
  let timedOut = false;
  let cleanup;
  const group = child.pid ? -child.pid : undefined;
  const alive = () => {
    if (!group) return false;
    try {
      process.kill(group, 0);
      return true;
    } catch (error) {
      if (error.code === "ESRCH") return false;
      if (error.code === "EPERM") return true;
      throw error;
    }
  };
  const send = (signal) => {
    if (!group) return;
    try {
      process.kill(group, signal);
    } catch (error) {
      if (!["ESRCH", "EPERM"].includes(error.code)) throw error;
    }
  };
  const terminate = () =>
    (cleanup ??= (async () => {
      send("SIGTERM");
      const grace = Date.now() + 150;
      while (alive() && Date.now() < grace) await wait(10);
      if (alive()) send("SIGKILL");
      const end = Date.now() + 500;
      while (alive() && Date.now() < end) await wait(10);
      return !alive();
    })().catch(() => false));
  const timer = setTimeout(() => {
    timedOut = true;
    void terminate();
  }, timeoutMs);
  const onInterrupt = () => {
    void terminate();
  };
  process.on("SIGINT", onInterrupt);
  process.on("SIGTERM", onInterrupt);
  const completion = new Promise((resolve) => {
    let error;
    child.on("error", (value) => {
      error = value.message;
    });
    child.on("close", (exitCode, signal) =>
      resolve({ exitCode, signal, error }),
    );
  });
  let result;
  try {
    onSpawn(child.pid);
    result = await completion;
  } catch (error) {
    await terminate();
    await completion;
    throw error;
  } finally {
    clearTimeout(timer);
    process.off("SIGINT", onInterrupt);
    process.off("SIGTERM", onInterrupt);
  }
  const descendantsRemained = !cleanup && alive();
  const cleanupConfirmed = cleanup
    ? await cleanup
    : descendantsRemained
      ? await terminate()
      : true;
  return {
    ...result,
    cleanupConfirmed,
    pid: child.pid ?? null,
    processGroupId: child.pid ?? null,
    status: interrupted
      ? "interrupted"
      : timedOut
        ? "timed_out"
        : !cleanupConfirmed || descendantsRemained
          ? "failed"
          : result.exitCode === 0
            ? "passed"
            : "failed",
    ...(descendantsRemained
      ? { reason: "Descendant process remained after command exit" }
      : {}),
  };
}
async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 4 || args[0] !== "--plan" || args[2] !== "--state")
    throw new Error(
      "Usage: node scripts/overnight-runner.mjs --plan PLAN.json --state STATE_DIRECTORY",
    );
  const plan = JSON.parse(readFileSync(args[1], "utf8"));
  const jobs = validate(plan);
  const deadline = Date.parse(plan.deadline);
  hardDeadline = deadline;
  if (!Number.isFinite(deadline))
    throw new Error("Plan requires an absolute deadline");
  if (process.platform === "win32")
    throw new Error("Process-group cleanup requires macOS or Linux");
  process.on("SIGINT", onSigint);
  process.on("SIGTERM", onSigterm);
  const state = path.resolve(args[3]);
  mkdirSync(state, { recursive: true, mode: 0o700 });
  const lock = path.join(state, "runner.lock");
  try {
    writeFileSync(
      lock,
      JSON.stringify({ pid: process.pid, startedAt: new Date().toISOString() }),
      { flag: "wx", mode: 0o600 },
    );
  } catch (error) {
    throw new Error(
      `Runner lock unavailable; inspect existing owner before resuming: ${error.message}`,
    );
  }
  try {
    const manifest = path.join(
      state,
      `plan-${hash(JSON.stringify(plan))}.json`,
    );
    try {
      writeFileSync(manifest, JSON.stringify(plan, null, 2) + "\n", {
        flag: "wx",
        mode: 0o600,
      });
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
    }
    const previous = readJSON(path.join(state, "summary.json"), { jobs: {} });
    const summary = {
      schemaVersion: 1,
      manifest,
      deadline: plan.deadline,
      startedAt: new Date().toISOString(),
      jobs: {},
    };
    let unsafeGroup = Object.values(previous.jobs).some(
      (result) => result.cleanupConfirmed === false,
    );
    for (const job of jobs) {
      const previousJob = previous.jobs[job.id];
      const prior = previousJob?.priorReceipt ?? previousJob;
      const preservedReceipt = prior ? { priorReceipt: prior } : {};
      if (
        prior?.status === "running" ||
        prior?.requiresInspection ||
        unsafeGroup
      ) {
        summary.jobs[job.id] = {
          ...prior,
          status: "blocked",
          requiresInspection: true,
          reason:
            "Unknown prior start or unconfirmed process cleanup; inspect owned process identity before retry",
        };
        save(path.join(state, "summary.json"), summary);
        continue;
      }
      if (
        !prior &&
        (interrupted ||
          Date.now() + job.timeoutMs + cleanupReserveMs > deadline)
      ) {
        summary.jobs[job.id] = {
          status: interrupted ? "interrupted" : "deadline",
          reason: interrupted ?? "Insufficient time before hard deadline",
        };
        save(path.join(state, "summary.json"), summary);
        continue;
      }
      if (job.dependsOn.some((id) => summary.jobs[id].status !== "passed")) {
        summary.jobs[job.id] = {
          status: "blocked",
          reason: "Dependency did not pass",
          dependsOn: job.dependsOn,
          ...preservedReceipt,
        };
        save(path.join(state, "summary.json"), summary);
        continue;
      }
      let inputHashes;
      let fingerprint;
      try {
        // Existing receipts may be verified read-only even after admission closes.
        inputHashes = files(job.cwd, job.inputs, !prior);
        fingerprint = hash(
          JSON.stringify({
            job,
            inputs: inputHashes,
            executable: files(job.cwd, [job.argv[0]], !prior),
            environment: Object.entries({ ...process.env, ...job.env }).sort(),
            runner: runnerHash,
            platform: process.platform,
            arch: process.arch,
            dependencies: job.dependsOn.map((id) => [
              id,
              summary.jobs[id].fingerprint,
              summary.jobs[id].outputHashes,
            ]),
          }),
        );
      } catch (error) {
        summary.jobs[job.id] = {
          status: "blocked",
          reason: error.message,
          ...preservedReceipt,
        };
        save(path.join(state, "summary.json"), summary);
        continue;
      }
      let reusable = false;
      if (
        prior?.fingerprint === fingerprint &&
        ["passed", "failed", "timed_out", "interrupted"].includes(prior.status)
      ) {
        try {
          reusable =
            (prior.status !== "passed" ||
              JSON.stringify(files(job.cwd, job.outputs, false)) ===
                JSON.stringify(prior.outputHashes)) &&
            hashFile(prior.log, false) === prior.logHash;
        } catch {}
        if (!reusable && prior.status !== "passed") {
          summary.jobs[job.id] = {
            ...prior,
            status: "blocked",
            requiresInspection: true,
            reason:
              "Unchanged failed attempt has incomplete evidence; automatic retry refused",
          };
          save(path.join(state, "summary.json"), summary);
          continue;
        }
      }
      if (reusable) {
        summary.jobs[job.id] = { ...prior, reused: true };
        save(path.join(state, "summary.json"), summary);
        continue;
      }
      if (
        interrupted ||
        Date.now() + job.timeoutMs + cleanupReserveMs > deadline
      ) {
        summary.jobs[job.id] = {
          status: interrupted ? "interrupted" : "deadline",
          reason: "Insufficient time after input validation",
          ...preservedReceipt,
        };
        save(path.join(state, "summary.json"), summary);
        continue;
      }
      const attempt = path.join(state, `${job.id}-${randomUUID()}`);
      mkdirSync(attempt, { mode: 0o700 });
      const log = path.join(attempt, "output.log");
      const initial = {
        argv: job.argv,
        cwd: job.cwd,
        timeoutMs: job.timeoutMs,
        dependsOn: job.dependsOn,
        resourceGroup: job.resourceGroup ?? null,
        fingerprint,
        inputHashes,
        log,
        startedAt: new Date().toISOString(),
        status: "running",
      };
      const persist = (result) => {
        save(path.join(attempt, "result.json"), result);
        summary.jobs[job.id] = result;
        save(path.join(state, "summary.json"), summary);
      };
      persist(initial);
      const result = {
        ...initial,
        ...(await execute(job, log, job.timeoutMs, (pid) =>
          persist({ ...initial, pid, processGroupId: pid }),
        )),
        finishedAt: new Date().toISOString(),
        logHash: hashFile(log, false),
      };
      unsafeGroup = !result.cleanupConfirmed;
      if (result.status === "passed") {
        try {
          result.outputHashes = files(job.cwd, job.outputs);
          if (
            JSON.stringify(files(job.cwd, job.inputs)) !==
            JSON.stringify(inputHashes)
          )
            throw new Error("Declared inputs changed during execution");
        } catch (error) {
          result.status = "failed";
          result.reason = error.message;
        }
      }
      persist(result);
    }
    summary.finishedAt = new Date().toISOString();
    save(path.join(state, "summary.json"), summary);
    process.exitCode = Object.values(summary.jobs).every(
      (job) => job.status === "passed",
    )
      ? 0
      : 1;
  } finally {
    unlinkSync(lock);
    process.off("SIGINT", onSigint);
    process.off("SIGTERM", onSigterm);
  }
}
main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
