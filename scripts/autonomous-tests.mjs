import { spawn, execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  constants,
  closeSync,
  existsSync,
  fstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export { buildPlan } from "./test-plan.mjs";
import { buildPlan } from "./test-plan.mjs";
export const steps = buildPlan();

export function validateRecipient(
  value,
  now = Date.now(),
  isAlive = (pid) => {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  },
) {
  const ready =
    value?.schemaVersion === 1 &&
    value.kind === "everlast-own-native-recipient-status" &&
    Number.isSafeInteger(value.observedUnixMillis) &&
    now - value.observedUnixMillis >= -100 &&
    now - value.observedUnixMillis <= 2000 &&
    Number.isSafeInteger(value.processId) &&
    value.processId > 0 &&
    isAlive(value.processId) &&
    value.running === true &&
    value.status === "observed" &&
    value.applicationActive === true &&
    value.frontmostIsSelf === true &&
    value.windowKey === true &&
    ["multiline", "singleline"].includes(value.focusedField) &&
    value.publicExpectedSentenceId === "fleurs-de-dev-1517";
  return {
    ready,
    automaticDeliveryProven: false,
    limitation:
      "Own-window point observation; does not prove Everlast captured that field or delivered text.",
  };
}

export function readRecipient(filename) {
  const fd = openSync(filename, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.size > 65536 || stat.uid !== process.getuid())
      throw new Error("Invalid own-recipient report file");
    return validateRecipient(JSON.parse(readFileSync(fd, "utf8")));
  } finally {
    closeSync(fd);
  }
}

function sourceBinding(cwd, plan) {
  const names = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    { cwd, encoding: "utf8" },
  )
    .split("\0")
    .filter((name) => name && !name.endsWith(".md"))
    .sort();
  const digest = createHash("sha256");
  digest.update(
    JSON.stringify({
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      steps: plan,
    }),
  );
  for (const name of new Set(names)) {
    if (!existsSync(path.join(cwd, name))) {
      digest.update(`missing:${name}\0`);
      continue;
    }
    digest.update(`${name}\0`);
    digest.update(readFileSync(path.join(cwd, name)));
    digest.update("\0");
  }
  return digest.digest("hex");
}

async function execute(step, cwd, logFile) {
  const fd = openSync(logFile, "wx", 0o600);
  let child;
  try {
    child = spawn(step.command, step.args, {
      cwd,
      stdio: ["ignore", fd, fd],
      detached: process.platform !== "win32",
      shell: false,
    });
  } finally {
    closeSync(fd);
  }
  let launchError;
  let reason;
  let cleanup;
  const groupId = process.platform === "win32" ? child.pid : -child.pid;
  const exists = () => {
    if (!groupId) return false;
    try {
      process.kill(groupId, 0);
      return true;
    } catch (error) {
      if (error.code === "ESRCH") return false;
      throw error;
    }
  };
  const send = (signal) => {
    if (!groupId) return;
    try {
      process.kill(groupId, signal);
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  };
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const terminate = () => {
    if (!cleanup)
      cleanup = (async () => {
        send("SIGTERM");
        const grace = Date.now() + 500;
        while (exists() && Date.now() < grace) await wait(20);
        if (exists()) send("SIGKILL");
        const deadline = Date.now() + 2000;
        while (exists() && Date.now() < deadline) await wait(20);
        return !exists();
      })();
    return cleanup;
  };
  const interrupt = (signal) => {
    reason ??= `Interrupted by ${signal}`;
    void terminate();
  };
  const sigint = () => interrupt("SIGINT");
  const sigterm = () => interrupt("SIGTERM");
  process.on("SIGINT", sigint);
  process.on("SIGTERM", sigterm);
  const timer = setTimeout(() => {
    reason ??= "Timed out";
    void terminate();
  }, step.timeoutMs ?? 300000);
  try {
    const result = await new Promise((resolve) => {
      child.on("error", (error) => {
        launchError = error.code ?? "Spawn failed";
      });
      child.on("close", (exitCode, signal) => resolve({ exitCode, signal }));
    });
    clearTimeout(timer);
    if (!cleanup && exists()) {
      reason ??= "Descendant process remained after child exit";
      void terminate();
    }
    const cleanupConfirmed = cleanup ? await cleanup : true;
    if (!cleanupConfirmed)
      reason = "Process-group cleanup could not be confirmed";
    return {
      ...result,
      cleanupConfirmed,
      ...(launchError || reason ? { reason: launchError ?? reason } : {}),
    };
  } finally {
    clearTimeout(timer);
    process.removeListener("SIGINT", sigint);
    process.removeListener("SIGTERM", sigterm);
  }
}

export async function runChecks({
  steps: plan,
  cwd = root,
  reportDirectory,
  binding,
  recipient = null,
  concurrency = 1,
  profile = "custom",
}) {
  if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 3)
    throw new Error("Concurrency must be an integer from 1 to 3");
  mkdirSync(reportDirectory, { mode: 0o700 }); // Exclusive: never overwrite a prior run.
  const reportFile = path.join(reportDirectory, "report.json");
  const report = {
    schemaVersion: 1,
    kind: "everlast-autonomous-regression",
    startedAt: new Date().toISOString(),
    sourceBinding: binding,
    profile,
    concurrency,
    passed: false,
    hardwareAcceptance: false,
    recipient,
    limits: [
      "No microphone, GUI automation, TCC changes, real AX capture, Whisper quality or performance qualification.",
      "Controlled decoder and native-mailbox outcomes do not prove OS delivery.",
    ],
    steps: plan.map((step) => ({
      id: step.id,
      command: step.command,
      args: step.args,
      status: "skipped",
    })),
  };
  const persist = () => {
    const temporary = path.join(reportDirectory, "report.tmp");
    writeFileSync(temporary, `${JSON.stringify(report, null, 2)}\n`, {
      mode: 0o600,
    });
    renameSync(temporary, reportFile);
  };
  persist();
  const runStep = async (index) => {
    const step = plan[index];
    const item = report.steps[index];
    item.status = "running";
    item.startedAt = new Date().toISOString();
    persist();
    const logFile = path.join(reportDirectory, `${index}.log`);
    Object.assign(item, await execute(step, cwd, logFile));
    const log = readFileSync(logFile);
    item.logFile = path.basename(logFile);
    item.logSha256 = createHash("sha256").update(log).digest("hex");
    item.executedRustTests = [
      ...log.toString("utf8").matchAll(/test result: ok\. (\d+) passed;/g),
    ].reduce((sum, match) => sum + Number(match[1]), 0);
    if (
      step.requireRustTests &&
      item.exitCode === 0 &&
      item.executedRustTests === 0
    )
      item.reason = "No executed Rust tests";
    item.status = item.exitCode === 0 && !item.reason ? "passed" : "failed";
    item.finishedAt = new Date().toISOString();
    persist();
  };
  const pending = plan.map((_, index) => index);
  const active = new Map();
  const resources = new Set();
  let failed = false;
  while (pending.length || active.size) {
    while (!failed && active.size < concurrency) {
      const slot = pending.findIndex(
        (index) =>
          !plan[index].resource || !resources.has(plan[index].resource),
      );
      if (slot < 0) break;
      const [index] = pending.splice(slot, 1);
      const resource = plan[index].resource;
      if (resource) resources.add(resource);
      active.set(
        index,
        runStep(index)
          .catch((error) => {
            report.steps[index].status = "failed";
            report.steps[index].reason = error.message;
            report.steps[index].finishedAt = new Date().toISOString();
            persist();
          })
          .then(() => {
            if (report.steps[index].status === "failed") failed = true;
          })
          .finally(() => {
            active.delete(index);
            if (resource) resources.delete(resource);
          }),
      );
    }
    if (!active.size) break;
    await Promise.race(active.values());
  }
  report.passed =
    report.steps.length > 0 &&
    report.steps.every((item) => item.status === "passed");
  report.finishedAt = new Date().toISOString();
  persist();
  return report;
}

export function parseOptions(args) {
  const options = { profile: "regression", concurrency: 3, planOnly: false };
  const seen = new Set();
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (seen.has(arg)) throw new Error("Duplicate option");
    seen.add(arg);
    if (arg === "--plan") options.planOnly = true;
    else if (
      arg === "--profile" &&
      ["regression", "full"].includes(args[i + 1])
    )
      options.profile = args[++i];
    else if (arg === "--jobs" && /^[1-3]$/.test(args[i + 1] ?? ""))
      options.concurrency = Number(args[++i]);
    else
      throw new Error(
        "Invalid arguments; use --help. OS recipient readiness is separate: npm run test:os:readiness -- /absolute/snapshot.json",
      );
  }
  return options;
}

async function main(args) {
  if (args.length === 1 && args[0] === "--help") {
    console.log(
      "npm run test:autonomous [-- --profile regression|full --jobs 1|2|3 --plan]\nNo installed app, microphone, model weights, permissions or focused window required.",
    );
    return;
  }
  const options = parseOptions(args);
  const plan = buildPlan(options.profile);
  if (options.planOnly) {
    console.log(
      JSON.stringify(
        { ...options, steps: plan, hardwareAcceptance: false },
        null,
        2,
      ),
    );
    return;
  }
  const reportDirectory = path.join(
    root,
    "artifacts",
    "autonomous-tests",
    `${Date.now()}-${randomUUID()}`,
  );
  mkdirSync(path.dirname(reportDirectory), { recursive: true, mode: 0o700 });
  const binding = sourceBinding(root, plan);
  const report = await runChecks({
    steps: plan,
    reportDirectory,
    binding,
    concurrency: options.concurrency,
    profile: options.profile,
  });
  console.log(
    JSON.stringify({
      passed: report.passed,
      reportFile: path.join(reportDirectory, "report.json"),
      executedRustTests: report.steps.reduce(
        (sum, step) => sum + (step.executedRustTests ?? 0),
        0,
      ),
      hardwareAcceptance: false,
      failedStep:
        report.steps.find((step) => step.status === "failed")?.id ?? null,
    }),
  );
  if (!report.passed) process.exitCode = 1;
}
if (process.argv[1] === fileURLToPath(import.meta.url))
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
