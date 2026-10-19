import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { runChecks, validateRecipient } from "./autonomous-tests.mjs";

test("a failing child stops the run and preserves the failure report", async () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "everlast-autonomous-"));
  try {
    const report = await runChecks({
      cwd: directory,
      reportDirectory: path.join(directory, "run"),
      binding: "test-input",
      steps: [
        {
          id: "broken",
          command: process.execPath,
          args: ["-e", "process.exit(7)"],
        },
        {
          id: "must-not-run",
          command: process.execPath,
          args: ["-e", "process.exit(0)"],
        },
      ],
    });
    assert.equal(report.passed, false);
    assert.equal(report.steps[0].exitCode, 7);
    assert.equal(report.steps[1].status, "skipped");
    assert.equal(
      JSON.parse(readFileSync(path.join(directory, "run", "report.json")))
        .passed,
      false,
    );
    assert.equal(report.hardwareAcceptance, false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("zero matching cargo tests is a failure even when the child exits zero", async () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "everlast-autonomous-"));
  try {
    const report = await runChecks({
      cwd: directory,
      reportDirectory: path.join(directory, "run"),
      binding: "test-input",
      steps: [
        {
          id: "empty",
          command: process.execPath,
          args: [
            "-e",
            'console.log("test result: ok. 0 passed; 0 failed; 4 filtered out")',
          ],
          requireRustTests: true,
        },
      ],
    });
    assert.equal(report.passed, false);
    assert.equal(report.steps[0].status, "failed");
    assert.equal(report.steps[0].reason, "No executed Rust tests");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("fresh own-window evidence permits readiness but never proves delivery", () => {
  const now = 10_000;
  const evidence = {
    schemaVersion: 1,
    kind: "everlast-own-native-recipient-status",
    observedUnixMillis: now,
    processId: 123,
    running: true,
    status: "observed",
    applicationActive: true,
    frontmostIsSelf: true,
    ownBundleIdentifier: "org.example.own-test-recipient",
    windowKey: true,
    focusedField: "multiline",
    publicExpectedSentenceId: "fleurs-de-dev-1517",
    expectedSentenceMatches: false,
    expectedSentenceOccurrences: 0,
    shortcutCharacterPresent: false,
  };
  assert.equal(validateRecipient(evidence, now, () => true).ready, true);
  assert.equal(
    validateRecipient(evidence, now + 2001, () => true).ready,
    false,
  );
  assert.equal(
    validateRecipient(
      { ...evidence, observedUnixMillis: now + 2000 },
      now,
      () => true,
    ).ready,
    false,
  );
  assert.equal(validateRecipient(evidence, now, () => false).ready, false);
  assert.equal(
    validateRecipient({ ...evidence, windowKey: false }, now, () => true).ready,
    false,
  );
  assert.equal(
    validateRecipient({ ...evidence, focusedField: "none" }, now, () => true)
      .ready,
    false,
  );
  assert.equal(
    validateRecipient({ ...evidence, running: false }, now, () => true).ready,
    false,
  );
  assert.equal(
    validateRecipient(
      { ...evidence, status: "observation-limit-reached" },
      now,
      () => true,
    ).ready,
    false,
  );
  assert.equal(
    validateRecipient(evidence, now, () => true).automaticDeliveryProven,
    false,
  );
});

test("active key window without own frontmost identity is not ready", () => {
  const evidence = {
    schemaVersion: 1,
    kind: "everlast-own-native-recipient-status",
    observedUnixMillis: 10000,
    processId: 123,
    running: true,
    status: "observed",
    applicationActive: true,
    windowKey: true,
    focusedField: "multiline",
    publicExpectedSentenceId: "fleurs-de-dev-1517",
    frontmostIsSelf: false,
    ownBundleIdentifier: "org.example.own-test-recipient",
  };
  assert.equal(validateRecipient(evidence, 10000, () => true).ready, false);
  const { frontmostIsSelf: _missing, ...historical } = evidence;
  assert.equal(validateRecipient(historical, 10000, () => true).ready, false);
  assert.equal(
    validateRecipient({ ...evidence, frontmostIsSelf: true }, 10000, () => true)
      .ready,
    true,
  );
});

test("a successful run records actual child results and refuses to overwrite evidence", async () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "everlast-autonomous-"));
  try {
    const options = {
      cwd: directory,
      reportDirectory: path.join(directory, "run"),
      binding: "test-input",
      steps: [
        {
          id: "one",
          command: process.execPath,
          args: ["-e", 'console.log("test result: ok. 2 passed; 0 failed;")'],
          requireRustTests: true,
        },
      ],
    };
    const report = await runChecks(options);
    assert.equal(report.passed, true);
    assert.equal(report.steps[0].executedRustTests, 2);
    await assert.rejects(runChecks(options), /exist/i);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("timeout reaps a descendant that ignores SIGTERM after its parent exits", async () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "everlast-autonomous-"));
  const pidFile = path.join(directory, "descendant.pid");
  let pid;
  try {
    const program = `
      const {spawn}=require('node:child_process');
      const child=spawn(process.execPath,['-e',"process.on('SIGTERM',()=>{});require('node:fs').writeFileSync(process.argv[1],String(process.pid));setInterval(()=>{},1000)",process.argv[1]],{stdio:'ignore'});
      process.on('SIGTERM',()=>process.exit(0));
      setInterval(()=>{},1000);
    `;
    const report = await runChecks({
      cwd: directory,
      reportDirectory: path.join(directory, "run"),
      binding: "test-input",
      steps: [
        {
          id: "timeout",
          command: process.execPath,
          args: ["-e", program, pidFile],
          timeoutMs: 400,
        },
      ],
    });
    pid = Number(readFileSync(pidFile, "utf8"));
    assert.equal(report.passed, false);
    assert.equal(report.steps[0].reason, "Timed out");
    assert.throws(() => process.kill(pid, 0), /ESRCH/);
  } finally {
    if (pid) {
      try {
        process.kill(pid, "SIGKILL");
      } catch {
        /* Already reaped. */
      }
    }
    rmSync(directory, { recursive: true, force: true });
  }
});

test("SIGTERM of the runner cleans its child group before returning failure", async () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "everlast-autonomous-"));
  const pidFile = path.join(directory, "child.pid");
  let childPid;
  let runner;
  try {
    const code = `import {runChecks} from ${JSON.stringify(new URL("./autonomous-tests.mjs", import.meta.url).href)};
      const report = await runChecks({cwd:process.argv[1],reportDirectory:process.argv[1]+'/run',binding:'signal-test',steps:[{
      id:'interrupt',command:process.execPath,args:['-e',"process.on('SIGTERM',()=>{});require('node:fs').writeFileSync(process.argv[1],String(process.pid));setInterval(()=>{},1000)",process.argv[2]]}]});
      process.exitCode=report.passed?0:1;`;
    runner = spawn(
      process.execPath,
      ["--input-type=module", "-e", code, directory, pidFile],
      { stdio: "ignore" },
    );
    const closed = new Promise((resolve, reject) => {
      runner.once("error", reject);
      runner.once("close", (code) => resolve(code));
    });
    const deadline = Date.now() + 3000;
    while (!existsSync(pidFile) && Date.now() < deadline)
      await new Promise((resolve) => setTimeout(resolve, 20));
    childPid = Number(readFileSync(pidFile, "utf8"));
    runner.kill("SIGTERM");
    assert.equal(await closed, 1);
    assert.throws(() => process.kill(childPid, 0), /ESRCH/);
    const report = JSON.parse(
      readFileSync(path.join(directory, "run", "report.json")),
    );
    assert.equal(report.steps[0].reason, "Interrupted by SIGTERM");
    assert.equal(report.steps[0].cleanupConfirmed, true);
  } finally {
    if (runner?.exitCode === null) runner.kill("SIGKILL");
    if (childPid) {
      try {
        process.kill(childPid, "SIGKILL");
      } catch {
        /* Already reaped. */
      }
    }
    rmSync(directory, { recursive: true, force: true });
  }
});
