import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import test from "node:test";
import { main, requestControl } from "./test-control.mjs";

const snapshot = {
  setupReady: true,
  microphoneGranted: true,
  accessibilityGranted: true,
  phase: "Idle",
  sessionId: null,
  outcome: null,
  publicSentenceMatches: false,
  transcriptCharacters: 0,
  actionError: false,
  controlledSessionId: null,
  controlledStartPending: false,
};
const response = (op, changes = {}) => ({
  schemaVersion: 1,
  ok: true,
  op,
  snapshot,
  error: null,
  ...changes,
});
const fieldMetadata = {
  kind: "own-test-recipient-field-metadata",
  pid: 123,
  role: "textArea",
  roleRead: "value",
  roleErrorCode: null,
  secure: false,
  subroleRead: "noValue",
  subroleErrorCode: -25212,
  enabled: null,
  enabledRead: "noValue",
  enabledErrorCode: -25212,
  valueSettable: true,
  valueSettableErrorCode: null,
  selectedTextSettable: null,
  selectedTextSettableErrorCode: -25205,
  captureEligible: false,
  holdReason: "unknownEditability",
};

async function withServer(handler, run) {
  const directory = realpathSync(mkdtempSync("/tmp/elc-"));
  chmodSync(directory, 0o700);
  const sockets = new Set();
  const server = net.createServer((socket) => {
    sockets.add(socket);
    socket.on("error", () => {});
    socket.on("close", () => sockets.delete(socket));
    handler(socket);
  });
  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(path.join(directory, "control.sock"), resolve);
    });
    chmodSync(path.join(directory, "control.sock"), 0o600);
    await run(directory);
  } finally {
    for (const socket of sockets) socket.destroy();
    if (server.listening) await new Promise((resolve) => server.close(resolve));
    rmSync(directory, { recursive: true, force: true });
  }
}

function reply(value) {
  return (socket) =>
    socket.once("data", () => socket.end(`${JSON.stringify(value)}\n`));
}

test("inspect accepts only bounded own-field metadata, without text or session authority", async () => {
  const expected = response("inspect", { snapshot: fieldMetadata });
  await withServer(reply(expected), async (controlDir) => {
    assert.deepEqual(
      await requestControl({ controlDir, op: "inspect" }),
      expected,
    );
    await assert.rejects(
      requestControl({ controlDir, op: "inspect", session: 1 }),
    );
  });
  for (const extra of [
    { transcript: "PRIVATE_SENTINEL" },
    { role: "PRIVATE_SENTINEL" },
    { pid: 0 },
    { enabledErrorCode: 1.5 },
    { captureEligible: "true" },
    { enabledRead: "PRIVATE_SENTINEL" },
  ]) {
    await withServer(
      reply(response("inspect", { snapshot: { ...fieldMetadata, ...extra } })),
      async (controlDir) => {
        await assert.rejects(
          requestControl({ controlDir, op: "inspect" }),
          (error) => {
            assert.doesNotMatch(error.message, /PRIVATE_SENTINEL/);
            return /protocol/i.test(error.message);
          },
        );
      },
    );
  }
});

test("one bounded request preserves operation/session and accepts fragmented sanitized reply", async () => {
  for (const op of ["status", "start", "stop", "cancel"]) {
    const session = ["stop", "cancel"].includes(op) ? 17 : undefined;
    await withServer(
      (socket) =>
        socket.once("data", (bytes) => {
          assert.deepEqual(JSON.parse(bytes), {
            schemaVersion: 1,
            op,
            ...(session ? { session } : {}),
          });
          const text = `${JSON.stringify(response(op))}\n`;
          socket.write(text.slice(0, 8));
          socket.end(text.slice(8));
        }),
      async (controlDir) => {
        const result = await requestControl({ controlDir, op, session });
        assert.deepEqual(result, response(op));
      },
    );
  }
});

test("malformed, mismatched and extra-field replies fail without echoing payload", async () => {
  for (const value of [
    response("start"),
    response("status", { schemaVersion: 2 }),
    response("status", { transcript: "PRIVATE_SENTINEL" }),
    response("status", {
      snapshot: { ...snapshot, transcript: "PRIVATE_SENTINEL" },
    }),
    response("status", { snapshot: { ...snapshot, sessionId: 1.5 } }),
    response("status", { snapshot: { ...snapshot, controlledSessionId: 0 } }),
    response("status", {
      snapshot: { ...snapshot, outcome: "PRIVATE_SENTINEL" },
    }),
    response("status", {
      snapshot: { ...snapshot, controlledStartPending: "false" },
    }),
  ]) {
    await withServer(reply(value), async (controlDir) => {
      await assert.rejects(
        requestControl({ controlDir, op: "status" }),
        (error) => {
          assert.doesNotMatch(error.message, /PRIVATE_SENTINEL/);
          return /protocol/i.test(error.message);
        },
      );
    });
  }
  await withServer(
    (socket) => socket.once("data", () => socket.end("PRIVATE_SENTINEL\n")),
    async (controlDir) => {
      await assert.rejects(
        requestControl({ controlDir, op: "status" }),
        /protocol/i,
      );
    },
  );
});

test("oversized reply, multiple records and unfinished record are rejected", async () => {
  for (const text of [
    "x".repeat(16385),
    Buffer.from([0xff, 0x0a]),
    `${JSON.stringify(response("status"))}\n{}\n`,
    JSON.stringify(response("status")),
  ]) {
    await withServer(
      (socket) => socket.once("data", () => socket.end(text)),
      async (controlDir) => {
        await assert.rejects(
          requestControl({ controlDir, op: "status" }),
          /protocol/i,
        );
      },
    );
  }
});

test("timeout and early close reject, close owned client and never retry command", async () => {
  for (const close of [false, true]) {
    let commands = 0;
    let closed;
    const closure = new Promise((resolve) => {
      closed = resolve;
    });
    await withServer(
      (socket) => {
        socket.once("close", closed);
        socket.on("data", () => {
          commands += 1;
          if (close) socket.end();
        });
      },
      async (controlDir) => {
        await assert.rejects(
          requestControl({ controlDir, op: "start", timeoutMs: 40 }),
          /timeout|closed/i,
        );
        await closure;
        assert.equal(commands, 1);
      },
    );
  }
});

test("operation/session errors and nonprivate directories are rejected before connection", async () => {
  let commands = 0;
  await withServer(
    (socket) => {
      commands += 1;
      socket.end();
    },
    async (controlDir) => {
      for (const options of [
        { op: "unknown" },
        { op: "stop" },
        { op: "status", session: 1 },
        { op: "cancel", session: 0 },
        { op: "cancel", session: Number.MAX_SAFE_INTEGER + 1 },
        { op: "start", timeoutMs: Infinity },
      ]) {
        await assert.rejects(requestControl({ controlDir, ...options }));
      }
      chmodSync(controlDir, 0o755);
      await assert.rejects(
        requestControl({ controlDir, op: "status" }),
        /private/i,
      );
      assert.equal(commands, 0);
    },
  );
});

test("server rejection returns only a generic error and no untrusted error text", async () => {
  await withServer(
    reply(
      response("start", {
        ok: false,
        snapshot: null,
        error: "PRIVATE_SENTINEL",
      }),
    ),
    async (controlDir) => {
      const result = await requestControl({ controlDir, op: "start" });
      assert.equal(result.ok, false);
      assert.equal(result.error, "Control command rejected");
      assert.doesNotMatch(JSON.stringify(result), /PRIVATE_SENTINEL/);
    },
  );
});

test("CLI parser uses only explicit env/flag directory and keeps owned session fields", async () => {
  await withServer(
    reply(
      response("stop", {
        snapshot: {
          ...snapshot,
          phase: "Finalizing",
          sessionId: 17,
          controlledSessionId: 17,
          outcome: "local",
        },
      }),
    ),
    async (controlDir) => {
      const result = await main(["stop", "17"], {
        EVERLAST_TEST_CONTROL_DIR: controlDir,
      });
      assert.equal(result.snapshot.controlledSessionId, 17);
      assert.equal(result.snapshot.controlledStartPending, false);
    },
  );
  for (const args of [
    ["stop", "0"],
    ["status", "--unknown"],
    ["status", "--timeout-ms", "forever"],
    ["status", "--control-dir"],
    ["status", "1"],
    ["status", "17", "extra"],
  ]) {
    await assert.rejects(main(args, {}));
  }
});

test("untrusted socket mode is rejected before sending a command", async () => {
  let connections = 0;
  await withServer(
    (socket) => {
      connections += 1;
      socket.end();
    },
    async (controlDir) => {
      chmodSync(path.join(controlDir, "control.sock"), 0o666);
      await assert.rejects(
        requestControl({ controlDir, op: "status" }),
        /private/i,
      );
      assert.equal(connections, 0);
    },
  );
});

const hostObservation = {
  schemaVersion: 1,
  kind: "controlled-host-start-observation-v1",
  qualification: false,
  physicalHotkey: false,
  exactCpalLatency: false,
  recordCount: 1,
  pendingOrdinal: null,
  overflowAttempts: 0,
  counterError: false,
  observerContentionOrPoison: false,
  lastRecord: {
    ordinal: 1,
    session: 7,
    observedUpperNanos: 180000000,
    ackOffsetNanos: 1000000,
    reason: null,
  },
};
test("host start keeps exact status and passive observation excludes arbitrary payloads", async () => {
  await withServer(reply(response("start-host")), async (controlDir) => {
    assert.equal(
      (await requestControl({ controlDir, op: "start-host" })).ok,
      true,
    );
  });
  await withServer(
    reply(response("host-observation", { snapshot: hostObservation })),
    async (controlDir) => {
      assert.deepEqual(
        (await requestControl({ controlDir, op: "host-observation" })).snapshot,
        hostObservation,
      );
      await assert.rejects(
        requestControl({ controlDir, op: "host-observation", session: 7 }),
      );
    },
  );
  for (const extra of [
    { privateText: "sentinel" },
    { physicalHotkey: true },
    { recordCount: 182 },
    { lastRecord: { ...hostObservation.lastRecord, session: 0 } },
  ]) {
    await withServer(
      reply(
        response("host-observation", {
          snapshot: { ...hostObservation, ...extra },
        }),
      ),
      async (controlDir) => {
        await assert.rejects(
          requestControl({ controlDir, op: "host-observation" }),
        );
      },
    );
  }
});

test("handoff observation remains bounded passive metadata with no text or target authority", async () => {
  const handoff = {
    schemaVersion: 1,
    kind: "native-handoff-timing-v1",
    maximumRecords: 32,
    qualification: false,
    exactEventPostInstant: false,
    invalid: false,
    overflow: false,
    observerContentionOrPoison: false,
    records: [
      {
        session: 7,
        textReadyObserved: true,
        deliveryTokenMatched: true,
        sendLowerNanos: 1000000,
        sendUpperNanos: 2000000,
        sendSucceeded: true,
        sendCommitted: true,
        clipboardCompletionLowerNanos: 500000000,
        clipboardCompletionUpperNanos: 501000000,
        cleanupSucceeded: true,
        restoreOutcome: "restored",
      },
    ],
  };
  await withServer(
    reply(response("handoff-observation", { snapshot: handoff })),
    async (controlDir) => {
      assert.deepEqual(
        (await requestControl({ controlDir, op: "handoff-observation" }))
          .snapshot,
        handoff,
      );
    },
  );
  for (const extra of [
    { privateText: "sentinel" },
    { records: [{ ...handoff.records[0], text: "sentinel" }] },
    { maximumRecords: 33 },
    { exactEventPostInstant: true },
  ]) {
    await withServer(
      reply(
        response("handoff-observation", { snapshot: { ...handoff, ...extra } }),
      ),
      async (controlDir) => {
        await assert.rejects(
          requestControl({ controlDir, op: "handoff-observation" }),
        );
      },
    );
  }
});
