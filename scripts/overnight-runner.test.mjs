import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const runner = fileURLToPath(
  new URL("./overnight-runner.mjs", import.meta.url),
);
async function fixture(t) {
  const dir = await mkdtemp(path.join(os.tmpdir(), "overnight-test-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  await writeFile(path.join(dir, "source.txt"), "initial");
  return dir;
}
function job(dir, id, code, extra = {}) {
  return {
    id,
    argv: [process.execPath, "-e", code],
    cwd: dir,
    timeoutMs: 1500,
    dependsOn: [],
    inputs: ["source.txt"],
    outputs: [],
    ...extra,
  };
}
async function run(dir, jobs, overrides = {}) {
  const plan = {
    schemaVersion: 1,
    deadline: new Date(Date.now() + 30000).toISOString(),
    jobs,
    ...overrides,
  };
  await writeFile(path.join(dir, "plan.json"), JSON.stringify(plan));
  const child = spawnSync(
    process.execPath,
    [
      runner,
      "--plan",
      path.join(dir, "plan.json"),
      "--state",
      path.join(dir, "state"),
    ],
    { encoding: "utf8", timeout: 10000 },
  );
  const summary =
    child.status === null
      ? null
      : JSON.parse(
          await readFile(path.join(dir, "state", "summary.json"), "utf8").catch(
            () => "null",
          ),
        );
  return { ...child, summary, plan };
}

test("executes declared argv with closed stdin and persists command, hashes and logs", async (t) => {
  const dir = await fixture(t);
  const result = await run(dir, [
    job(
      dir,
      "check",
      "process.stdin.on('end',()=>console.log('closed'));process.stdin.resume()",
    ),
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.summary.jobs.check.status, "passed");
  assert.match(await readFile(result.summary.jobs.check.log, "utf8"), /closed/);
  assert.equal(result.summary.jobs.check.argv[0], process.execPath);
  assert.match(result.summary.jobs.check.fingerprint, /^[a-f0-9]{64}$/);
});

test("resumes only unchanged inputs and existing matching outputs, preserving older attempts", async (t) => {
  const dir = await fixture(t);
  const jobs = [
    job(
      dir,
      "check",
      "require('fs').appendFileSync('calls','x');require('fs').writeFileSync('artifact','built')",
      { outputs: ["artifact"] },
    ),
  ];
  const first = await run(dir, jobs);
  assert.equal(first.status, 0, first.stderr);
  const second = await run(dir, jobs);
  assert.equal(second.summary.jobs.check.reused, true);
  assert.equal(await readFile(path.join(dir, "calls"), "utf8"), "x");
  await writeFile(path.join(dir, "artifact"), "tampered");
  const third = await run(dir, jobs);
  assert.notEqual(third.summary.jobs.check.log, first.summary.jobs.check.log);
  assert.equal(await readFile(first.summary.jobs.check.log, "utf8"), "");
  await writeFile(path.join(dir, "source.txt"), "changed");
  const fourth = await run(dir, jobs);
  assert.notEqual(
    fourth.summary.jobs.check.fingerprint,
    first.summary.jobs.check.fingerprint,
  );
  assert.equal(await readFile(path.join(dir, "calls"), "utf8"), "xxx");
});

test("failure blocks only descendants, independent jobs run, unchanged failures are never blindly retried", async (t) => {
  const dir = await fixture(t);
  const jobs = [
    job(dir, "dependent", "require('fs').writeFileSync('forbidden','ran')", {
      dependsOn: ["failure"],
    }),
    job(
      dir,
      "failure",
      "require('fs').appendFileSync('attempts','x');process.exit(7)",
      { outputs: ["missing-output"] },
    ),
    job(dir, "independent", "console.log('independent')"),
  ];
  const first = await run(dir, jobs);
  assert.equal(first.status, 1);
  assert.equal(first.summary.jobs.failure.exitCode, 7);
  assert.equal(first.summary.jobs.dependent.status, "blocked");
  assert.equal(first.summary.jobs.independent.status, "passed");
  const second = await run(dir, jobs);
  assert.equal(second.summary.jobs.failure.reused, true);
  assert.equal(await readFile(path.join(dir, "attempts"), "utf8"), "x");
});

test("timeout cleans its process group and lets the next independent job finish", async (t) => {
  const dir = await fixture(t);
  const result = await run(dir, [
    job(
      dir,
      "timeout",
      "require('child_process').spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});console.log('started');setInterval(()=>{},1000)",
      { timeoutMs: 200 },
    ),
    job(dir, "after", "console.log('after')"),
  ]);
  assert.equal(result.status, 1);
  assert.equal(result.summary.jobs.timeout.status, "timed_out");
  assert.equal(result.summary.jobs.timeout.cleanupConfirmed, true);
  assert.equal(result.summary.jobs.after.status, "passed");
});

test("expired deadline starts no commands and produces an explicit incomplete journal", async (t) => {
  const dir = await fixture(t);
  const result = await run(
    dir,
    [job(dir, "late", "console.log('must not run')")],
    { deadline: "2020-01-01T00:00:00Z" },
  );
  assert.equal(result.status, 1);
  assert.equal(result.summary.jobs.late.status, "deadline");
  assert.equal(result.summary.jobs.late.log, undefined);
});

test("rejects unsafe or ambiguous plans before executing any command", async (t) => {
  const dir = await fixture(t);
  const invalid = [
    [job(dir, "../escape", "console.log('unsafe')")],
    [job(dir, "same", ""), job(dir, "same", "")],
    [job(dir, "cycle", "", { dependsOn: ["cycle"] })],
    [job(dir, "relative", "", { argv: ["node", "-e", ""] })],
    [job(dir, "unbounded", "", { timeoutMs: 0 })],
    [job(dir, "undeclared", "", { inputs: [] })],
  ];
  for (const jobs of invalid) {
    const result = await run(dir, jobs);
    assert.equal(result.status, 1, JSON.stringify(jobs));
    assert.equal(
      result.summary,
      null,
      "Invalid plan must not execute or create results",
    );
  }
});

test("missing input blocks only that job and records an immutable manifest", async (t) => {
  const dir = await fixture(t);
  const result = await run(dir, [
    job(dir, "missing", "", { inputs: ["absent"] }),
    job(dir, "available", "console.log('works')"),
  ]);
  assert.equal(result.status, 1);
  assert.equal(result.summary.jobs.missing.status, "blocked");
  assert.equal(result.summary.jobs.available.status, "passed");
  assert.deepEqual(
    JSON.parse(await readFile(result.summary.manifest, "utf8")),
    result.plan,
  );
});

test("never restarts an unknown prior start, while independent work continues", async (t) => {
  const dir = await fixture(t);
  const jobs = [
    job(dir, "unknown", "require('fs').writeFileSync('forbidden','ran')"),
    job(dir, "independent", ""),
  ];
  const first = await run(dir, jobs);
  const persisted = first.summary;
  persisted.jobs.unknown.status = "running";
  await writeFile(
    path.join(dir, "state", "summary.json"),
    JSON.stringify(persisted),
  );
  await rm(path.join(dir, "forbidden"));
  for (let count = 0; count < 2; count++) {
    const next = await run(dir, jobs);
    assert.equal(next.summary.jobs.unknown.status, "blocked");
    assert.equal(next.summary.jobs.unknown.requiresInspection, true);
    assert.equal(next.summary.jobs.independent.status, "passed");
    await assert.rejects(readFile(path.join(dir, "forbidden")), {
      code: "ENOENT",
    });
  }
});

test("exclusive state ownership refuses concurrent or stale runners rather than stealing their jobs", async (t) => {
  const dir = await fixture(t);
  const initial = await run(dir, [job(dir, "initial", "")]);
  await writeFile(
    path.join(dir, "state", "runner.lock"),
    JSON.stringify({ pid: process.pid }),
  );
  const next = await run(dir, [
    job(dir, "forbidden", "require('fs').writeFileSync('forbidden','ran')"),
  ]);
  assert.equal(next.status, 1);
  assert.match(next.stderr, /lock/i);
  assert.deepEqual(next.summary, initial.summary);
});

test("SIGTERM persists cleanup and prevents later jobs from starting", async (t) => {
  const dir = await fixture(t);
  const plan = {
    schemaVersion: 1,
    deadline: new Date(Date.now() + 30000).toISOString(),
    jobs: [
      job(dir, "active", "console.log('ready');setInterval(()=>{},1000)", {
        timeoutMs: 5000,
      }),
      job(dir, "after", "require('fs').writeFileSync('forbidden','ran')"),
    ],
  };
  await writeFile(path.join(dir, "plan.json"), JSON.stringify(plan));
  const child = spawn(
    process.execPath,
    [
      runner,
      "--plan",
      path.join(dir, "plan.json"),
      "--state",
      path.join(dir, "state"),
    ],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  let errors = "";
  child.stderr.on("data", (bytes) => {
    errors += bytes;
  });
  t.after(() => {
    if (child.exitCode === null) child.kill("SIGKILL");
  });
  const closed = new Promise((resolve) => child.on("close", resolve));
  const until = Date.now() + 3000;
  let snapshot;
  while (Date.now() < until) {
    snapshot = JSON.parse(
      await readFile(path.join(dir, "state", "summary.json"), "utf8").catch(
        () => "{}",
      ),
    );
    if (snapshot.jobs?.active?.pid) break;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.ok(
    snapshot.jobs?.active?.pid,
    "Wait for durable process identity before interrupting",
  );
  child.kill("SIGTERM");
  assert.equal(await closed, 1);
  const result = JSON.parse(
    await readFile(path.join(dir, "state", "summary.json"), "utf8"),
  );
  assert.equal(result.jobs.active.status, "interrupted", errors);
  assert.equal(result.jobs.active.cleanupConfirmed, true);
  assert.equal(result.jobs.after.status, "interrupted");
  await assert.rejects(readFile(path.join(dir, "forbidden")), {
    code: "ENOENT",
  });
});

test("deadline admission preserves completed receipts and never reexecutes unchanged work", async (t) => {
  const dir = await fixture(t);
  const jobs = [job(dir, "once", "require('fs').appendFileSync('calls','x')")];
  const first = await run(dir, jobs);
  const expired = await run(dir, jobs, { deadline: "2020-01-01T00:00:00Z" });
  assert.equal(expired.summary.jobs.once.status, "passed");
  assert.equal(expired.summary.jobs.once.reused, true);
  assert.equal(expired.summary.jobs.once.log, first.summary.jobs.once.log);
  const resumed = await run(dir, jobs);
  assert.equal(resumed.summary.jobs.once.reused, true);
  assert.equal(await readFile(path.join(dir, "calls"), "utf8"), "x");
  await writeFile(path.join(dir, "source.txt"), "changed");
  const deferred = await run(dir, jobs, { deadline: "2020-01-01T00:00:00Z" });
  assert.equal(deferred.summary.jobs.once.status, "deadline");
  assert.equal(
    deferred.summary.jobs.once.priorReceipt.log,
    first.summary.jobs.once.log,
  );
});
