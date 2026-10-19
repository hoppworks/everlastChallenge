import assert from "node:assert/strict";
import test from "node:test";
import { runHostStartCampaign } from "./host-start-campaign.mjs";
const cases = ["de", "en"].flatMap((language) =>
  [5, 10, 15].map((seconds) => ({
    id: `${language}-${seconds}`,
    language,
    seconds,
    audioSha256: "a".repeat(64),
  })),
);
function fixture({ badUpper = false, foreign = false } = {}) {
  let tick = 0,
    sid = 0,
    count = 0,
    phase = "Idle",
    owned = null;
  const calls = [];
  const command = async ({ op, session }) => {
    calls.push({ op, session });
    if (op === "start-host") {
      sid++;
      owned = sid;
      phase = "Recording";
      count++;
    }
    if (op === "cancel") {
      assert.equal(session, owned);
      phase = "Idle";
      owned = null;
    }
    const observation = {
      schemaVersion: 1,
      kind: "controlled-host-start-observation-v1",
      qualification: false,
      physicalHotkey: false,
      exactCpalLatency: false,
      recordCount: count,
      pendingOrdinal: null,
      overflowAttempts: 0,
      counterError: false,
      observerContentionOrPoison: false,
      lastRecord: count
        ? {
            ordinal: count,
            session: foreign ? sid + 1 : sid,
            observedUpperNanos: badUpper ? null : 180000000,
            ackOffsetNanos: 10000000,
            reason: badUpper ? "timeout" : null,
          }
        : null,
    };
    return {
      ok: true,
      snapshot:
        op === "host-observation"
          ? observation
          : {
              setupReady: true,
              microphoneGranted: true,
              accessibilityGranted: true,
              phase,
              sessionId: owned,
              controlledSessionId: owned,
              controlledStartPending: false,
            },
    };
  };
  return {
    calls,
    writeReport: async () => {},
    command,
    now: () => tick,
    sleep: async (ms) => {
      tick += ms;
    },
  };
}
test("one warmup plus 180 owned starts retain six fixed future-input cases without audio", async () => {
  const f = fixture();
  const report = await runHostStartCampaign({ cases, ...f });
  assert.equal(report.passed, true);
  assert.equal(report.trials.length, 181);
  assert.equal(report.trials.filter((t) => t.warmup).length, 1);
  assert.equal(f.calls.filter((c) => c.op === "start-host").length, 181);
  assert.equal(f.calls.filter((c) => c.op === "cancel").length, 181);
  assert.deepEqual(
    report.cases.map((c) => c.medianUpperMillis),
    [180, 180, 180, 180, 180, 180],
  );
  assert.equal(report.physicalHotkey, false);
  assert.equal(report.futureInputOnly, true);
});
test("missing or foreign measurement fails without borrowing endpoint ownership", async () => {
  for (const option of [{ badUpper: true }, { foreign: true }]) {
    const f = fixture(option);
    const report = await runHostStartCampaign({ cases, ...f });
    assert.equal(report.passed, false);
    assert.equal(report.cleanupConfirmed, false);
    assert.equal(f.calls.filter((c) => c.op === "start-host").length, 1);
    assert.deepEqual(
      f.calls.filter((c) => c.op === "cancel").map((c) => c.session),
      [],
    );
  }
});
test("pre-existing developer session is never cancelled by a campaign rejected at preflight", async () => {
  const calls = [];
  const report = await runHostStartCampaign({
    cases,
    writeReport: async () => {},
    command: async ({ op, session }) => {
      calls.push({ op, session });
      if (op !== "status") throw new Error("unexpected-cancel");
      return {
        ok: true,
        snapshot:
          op === "status"
            ? {
                phase: "Recording",
                sessionId: 9,
                controlledSessionId: 9,
                controlledStartPending: false,
              }
            : {},
      };
    },
    now: () => 0,
    sleep: async () => {},
  });
  assert.equal(report.passed, false);
  assert.equal(report.cleanupConfirmed, false);
  assert.deepEqual(
    calls.map((c) => c.op),
    ["status", "status"],
  );
});
test("unknown start reply is not retried and cannot authorize later endpoint ownership", async () => {
  const f = fixture();
  const actual = f.command;
  const report = await runHostStartCampaign({
    cases,
    ...f,
    command: async (args) => {
      const response = await actual(args);
      if (args.op === "start-host") throw new Error("unknown-start-outcome");
      return response;
    },
  });
  assert.equal(report.passed, false);
  assert.equal(report.cleanupConfirmed, false);
  assert.equal(f.calls.filter((c) => c.op === "start-host").length, 1);
  assert.deepEqual(
    f.calls.filter((c) => c.op === "cancel").map((c) => c.session),
    [],
  );
});
test("all valid observations are retained when a case median exceeds 250ms", async () => {
  const f = fixture();
  const actual = f.command;
  const report = await runHostStartCampaign({
    cases,
    ...f,
    command: async (args) => {
      const response = await actual(args);
      if (args.op === "host-observation" && response.snapshot.lastRecord)
        response.snapshot.lastRecord.observedUpperNanos = 251000000;
      return response;
    },
  });
  assert.equal(report.passed, false);
  assert.equal(report.cleanupConfirmed, true);
  assert.equal(report.trials.length, 181);
  assert.equal(report.error, "start-median-exceeds-250ms");
  assert.equal(report.cases[0].medianUpperMillis, 251);
});

test("even median averages middle pair and excludes the single slow warmup", async () => {
  const f = fixture();
  const actual = f.command;
  const report = await runHostStartCampaign({
    cases,
    ...f,
    command: async (args) => {
      const response = await actual(args);
      const r = response.snapshot.lastRecord;
      if (args.op === "host-observation" && r) {
        if (r.ordinal === 1) r.observedUpperNanos = 999000000;
        else if (r.ordinal <= 16) r.observedUpperNanos = 100000000;
        else if (r.ordinal <= 31) r.observedUpperNanos = 200000000;
      }
      return response;
    },
  });
  assert.equal(report.passed, true);
  assert.equal(report.cases[0].medianUpperMillis, 150);
  assert.equal(report.trials[0].observedUpperNanos, 999000000);
});

test("false final cleanup forces failure even after all measured medians passed", async () => {
  const f = fixture();
  const actual = f.command;
  let finalStatuses = 0;
  const report = await runHostStartCampaign({
    cases,
    ...f,
    command: async (args) => {
      const response = await actual(args);
      if (
        args.op === "status" &&
        f.calls.filter((c) => c.op === "cancel").length === 181 &&
        ++finalStatuses === 2
      ) {
        response.snapshot = {
          ...response.snapshot,
          phase: "Recording",
          sessionId: 999,
          controlledSessionId: 999,
        };
      }
      return response;
    },
  });
  assert.equal(report.cleanupConfirmed, false);
  assert.equal(report.passed, false);
  assert.equal(report.error, "cleanup-unconfirmed");
  assert.equal(
    f.calls.some((c) => c.op === "cancel" && c.session === 999),
    false,
  );
});

test("rejected or unknown start never cancels another caller's later controlled SID", async () => {
  for (const mode of ["rejected", "unknown"]) {
    const f = fixture();
    const actual = f.command;
    let attempted = false;
    const report = await runHostStartCampaign({
      cases,
      ...f,
      command: async (args) => {
        const response = await actual(args);
        if (args.op === "start-host") {
          attempted = true;
          if (mode === "unknown") throw new Error("unknown-start-outcome");
          return { ok: false, snapshot: null };
        }
        if (args.op === "status" && attempted)
          response.snapshot = {
            ...response.snapshot,
            phase: "Recording",
            sessionId: 999,
            controlledSessionId: 999,
          };
        return response;
      },
    });
    assert.equal(report.passed, false);
    assert.equal(report.cleanupConfirmed, false);
    assert.deepEqual(
      f.calls.filter((c) => c.op === "cancel"),
      [],
    );
  }
});

test("successful ACK with a failed own record cannot adopt another caller's recording", async () => {
  const f = fixture({ badUpper: true });
  const actual = f.command;
  const report = await runHostStartCampaign({
    cases,
    ...f,
    command: async (args) => {
      const response = await actual(args);
      if (args.op === "status" && f.calls.some((c) => c.op === "start-host"))
        response.snapshot = {
          ...response.snapshot,
          phase: "Recording",
          sessionId: 999,
          controlledSessionId: 999,
        };
      return response;
    },
  });
  assert.equal(report.passed, false);
  assert.equal(report.cleanupConfirmed, false);
  assert.deepEqual(
    f.calls.filter((c) => c.op === "cancel"),
    [],
  );
});

test("cleanup keeps frozen validated SID and never adopts a later replacement", async () => {
  const f = fixture();
  const actual = f.command;
  let cancelUncertain = false;
  const report = await runHostStartCampaign({
    cases,
    ...f,
    command: async (args) => {
      const response = await actual(args);
      if (args.op === "cancel") {
        cancelUncertain = true;
        throw new Error("unknown-cancel-outcome");
      }
      if (args.op === "status" && cancelUncertain)
        response.snapshot = {
          ...response.snapshot,
          phase: "Recording",
          sessionId: 999,
          controlledSessionId: 999,
        };
      return response;
    },
  });
  assert.equal(report.passed, false);
  assert.equal(report.cleanupConfirmed, false);
  assert.deepEqual(
    f.calls.filter((c) => c.op === "cancel").map((c) => c.session),
    [1],
  );
});
