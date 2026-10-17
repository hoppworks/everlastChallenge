import { beforeEach, expect, test, vi } from "vitest";
import { command, connect, decodeSnapshot, sessionCommand } from "./index";
import type { Snapshot } from "./index";

const native = vi.hoisted(() => ({
  invoke: vi.fn<(...args: unknown[]) => Promise<unknown>>(),
  listen:
    vi.fn<
      (
        name: string,
        callback: (event: { payload: unknown }) => void,
      ) => Promise<() => void>
    >(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));

function snapshot(): Snapshot {
  return {
    revision: 1,
    running: true,
    settingsBusy: false,
    error: null,
    setup: {
      phase: "Ready",
      model: {
        name: "large-v3-turbo-q5_0",
        downloadBytes: 574041195,
        sourceUrl:
          "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-large-v3-turbo-q5_0.bin",
      },
      downloadedBytes: 0,
      totalBytes: 574041195,
      canCancel: false,
      error: null,
    },
    session: {
      phase: "Idle",
      id: null,
      text: "",
      outcome: null,
      maximumWarning: false,
    },
    permissions: { microphone: "granted", accessibility: "granted" },
    labels: { device: "Standardmikrofon", language: "Deutsch", hotkey: "⌥D" },
  };
}

beforeEach(() => vi.resetAllMocks());

test("accepts native null option values and bounded German text unchanged", () => {
  const value = snapshot();
  value.error = "Verbindung gestört";
  value.setup.error = "Erneut versuchen";
  value.session.text = "Ein Satz.\nNoch ein Satz mit Umlauten: äöüß.";
  expect(decodeSnapshot(value)).toEqual(value);
});

test.each([
  "Missing",
  "Checking",
  "Downloading",
  "Loading",
  "Cancelling",
  "Cancelled",
  "Ready",
  "Failed",
] as const)("accepts documented setup phase %s", (phase) => {
  const value = snapshot();
  value.setup.phase = phase;
  expect(decodeSnapshot(value).setup.phase).toBe(phase);
});
test.each([
  "Idle",
  "Starting",
  "Recording",
  "Finalizing",
  "Delivering",
  "Cancelling",
] as const)("accepts documented session phase %s", (phase) => {
  const value = snapshot();
  value.session.phase = phase;
  value.session.id = 7;
  expect(decodeSnapshot(value).session.phase).toBe(phase);
});
test.each([
  "granted",
  "denied",
  "notDetermined",
  "restricted",
  "unavailable",
] as const)("accepts documented permission %s", (status) => {
  const value = snapshot();
  value.permissions = { microphone: status, accessibility: status };
  expect(decodeSnapshot(value).permissions.microphone).toBe(status);
});
test.each(["local", "empty", "sent", "cancelled"] as const)(
  "accepts outcome %s without message",
  (kind) => {
    const value = snapshot();
    value.session.outcome = { kind };
    expect(decodeSnapshot(value).session.outcome).toEqual({ kind });
  },
);
test.each(["held", "failed"] as const)(
  "accepts outcome %s with a reason",
  (kind) => {
    const value = snapshot();
    value.session.outcome = { kind, message: "Das Ziel ist nicht mehr aktiv." };
    expect(decodeSnapshot(value).session.outcome).toEqual(
      value.session.outcome,
    );
  },
);

test.each([null, false, 1, "status", [], {}, { ...snapshot(), extra: true }])(
  "rejects invalid top-level value %#",
  (value) => {
    expect(() => decodeSnapshot(value)).toThrow("Ungültiger Anwendungsstatus");
  },
);

test.each([
  ["revision", -1],
  ["revision", 1.5],
  ["revision", Number.MAX_SAFE_INTEGER + 1],
  ["revision", "1"],
  ["running", "true"],
  ["settingsBusy", null],
  ["error", false],
  ["error", "x".repeat(4097)],
] as const)("rejects invalid top-level field %s (%#)", (key, value) => {
  expect(() => decodeSnapshot({ ...snapshot(), [key]: value })).toThrow(
    "Ungültiger Anwendungsstatus",
  );
});

test.each([
  { phase: "unknown" },
  { downloadedBytes: -1 },
  { totalBytes: null },
  { downloadedBytes: 574041196 },
  { canCancel: 1 },
  { error: [] },
  { extra: "unknown" },
  { model: null },
  {
    model: {
      name: "x",
      downloadBytes: 1,
      sourceUrl: "https://evil.example/model",
    },
  },
  {
    model: {
      ...snapshot().setup.model,
      sourceUrl: "http://huggingface.co/ggerganov/whisper.cpp/resolve/model",
    },
  },
  { model: { ...snapshot().setup.model, downloadBytes: NaN } },
  { model: { ...snapshot().setup.model, name: "x".repeat(4097) } },
  { model: { ...snapshot().setup.model, extra: true } },
])("rejects invalid setup contract %#", (patch) => {
  const value = snapshot();
  expect(() =>
    decodeSnapshot({ ...value, setup: { ...value.setup, ...patch } }),
  ).toThrow("Ungültiger Anwendungsstatus");
});

test.each([
  { phase: "unknown" },
  { id: 0 },
  { id: -1 },
  { id: 1.5 },
  { id: undefined },
  { text: null },
  { text: "x".repeat(262145) },
  { maximumWarning: "true" },
  { extra: true },
])("rejects invalid session contract %#", (patch) => {
  const value = snapshot();
  expect(() =>
    decodeSnapshot({ ...value, session: { ...value.session, ...patch } }),
  ).toThrow("Ungültiger Anwendungsstatus");
});

test.each([
  undefined,
  "sent",
  {},
  { kind: "unknown" },
  { kind: "sent", message: "extra" },
  { kind: "held" },
  { kind: "failed", message: 5 },
  { kind: "held", message: "x".repeat(4097) },
  { kind: "failed", message: "reason", extra: true },
])("rejects invalid outcome contract %#", (outcome) => {
  const value = snapshot();
  expect(() =>
    decodeSnapshot({ ...value, session: { ...value.session, outcome } }),
  ).toThrow("Ungültiges Diktatergebnis");
});

test.each([
  { labels: null },
  { labels: { ...snapshot().labels, device: 1 } },
  { labels: { ...snapshot().labels, language: [] } },
  { labels: { ...snapshot().labels, hotkey: null } },
  { permissions: [] },
  { permissions: { microphone: "unknown", accessibility: "granted" } },
  { permissions: { microphone: "granted", accessibility: false } },
])("rejects invalid labels or permissions %#", (patch) => {
  expect(() => decodeSnapshot({ ...snapshot(), ...patch })).toThrow(
    "Ungültiger Anwendungsstatus",
  );
});

test("listener is installed before the snapshot; newer events win over a late older snapshot", async () => {
  const received: number[] = [];
  const order: string[] = [];
  const unlisten = vi.fn<() => void>();
  let deliver: ((event: { payload: unknown }) => void) | undefined;
  native.listen.mockImplementation(
    async (_name: string, callback: typeof deliver) => {
      order.push("listen");
      deliver = callback;
      return unlisten;
    },
  );
  native.invoke.mockImplementation(async () => {
    order.push("snapshot");
    deliver?.({ payload: { ...snapshot(), revision: 3 } });
    return { ...snapshot(), revision: 2 };
  });
  const stop = await connect((value) => received.push(value.revision));
  expect(order).toEqual(["listen", "snapshot"]);
  expect(received).toEqual([3]);
  deliver?.({ payload: { ...snapshot(), revision: 3 } });
  deliver?.({ payload: { ...snapshot(), revision: 1 } });
  deliver?.({ payload: { ...snapshot(), revision: 4 } });
  expect(received).toEqual([3, 4]);
  stop();
  expect(unlisten).toHaveBeenCalledOnce();
  deliver?.({ payload: { ...snapshot(), revision: 5 } });
  expect(received).toEqual([3, 4]);
});

test("snapshot failure removes the already-installed listener and rejects", async () => {
  const unlisten = vi.fn<() => void>();
  native.listen.mockResolvedValue(unlisten);
  native.invoke.mockRejectedValue(new Error("native disconnected"));
  await expect(connect(vi.fn<(value: Snapshot) => void>())).rejects.toThrow(
    "native disconnected",
  );
  expect(unlisten).toHaveBeenCalledOnce();
});

test("invalid initial payload rejects without publishing it and cleans up", async () => {
  const unlisten = vi.fn<() => void>();
  const receive = vi.fn<(value: Snapshot) => void>();
  native.listen.mockResolvedValue(unlisten);
  native.invoke.mockResolvedValue({ revision: 1 });
  await expect(connect(receive)).rejects.toThrow("Ungültiger Anwendungsstatus");
  expect(receive).not.toHaveBeenCalled();
  expect(unlisten).toHaveBeenCalledOnce();
});

test("subscription failure never requests a snapshot", async () => {
  native.listen.mockRejectedValue(new Error("listener failed"));
  await expect(connect(vi.fn<(value: Snapshot) => void>())).rejects.toThrow(
    "listener failed",
  );
  expect(native.invoke).not.toHaveBeenCalled();
});

test("malformed payloads queued after unsubscribe are ignored", async () => {
  let deliver: ((event: { payload: unknown }) => void) | undefined;
  native.listen.mockImplementation(
    async (_name: string, callback: typeof deliver) => {
      deliver = callback;
      return vi.fn<() => void>();
    },
  );
  native.invoke.mockResolvedValue(snapshot());
  const stop = await connect(vi.fn<(value: Snapshot) => void>());
  stop();
  expect(() => deliver?.({ payload: null })).not.toThrow();
});

test.each([
  "model_download",
  "model_cancel",
  "recording_start",
  "transcript_copy",
  "transcript_clear",
  "microphone_request",
  "accessibility_request",
] as const)("forwards explicit command %s", async (name) => {
  native.invoke.mockResolvedValue(undefined);
  await command(name);
  expect(native.invoke).toHaveBeenCalledWith(name);
});
test.each(["recording_stop", "recording_cancel"] as const)(
  "forwards %s with the target session",
  async (name) => {
    native.invoke.mockResolvedValue(undefined);
    await sessionCommand(name, 7);
    expect(native.invoke).toHaveBeenCalledWith(name, { session: 7 });
  },
);
test.each([0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])(
  "rejects invalid session %s before invocation",
  async (session) => {
    await expect(sessionCommand("recording_cancel", session)).rejects.toThrow(
      "Ungültige Sitzungskennung",
    );
    expect(native.invoke).not.toHaveBeenCalled();
  },
);

test("invalid event reports an error without crashing or advancing the accepted revision", async () => {
  let deliver: ((event: { payload: unknown }) => void) | undefined;
  const receive = vi.fn<(value: Snapshot) => void>();
  const report = vi.fn<(error: unknown) => void>();
  native.listen.mockImplementation(async (_name, callback) => {
    deliver = callback;
    return vi.fn<() => void>();
  });
  native.invoke.mockResolvedValue(snapshot());
  const stop = await connect(receive, report);
  expect(() => deliver?.({ payload: { revision: 500 } })).not.toThrow();
  expect(report).toHaveBeenCalledOnce();
  expect(report).toHaveBeenCalledWith(
    expect.objectContaining({ message: "Ungültiger Anwendungsstatus" }),
  );
  deliver?.({ payload: { ...snapshot(), revision: 2 } });
  expect(receive.mock.calls.map(([value]) => value.revision)).toEqual([1, 2]);
  stop();
  deliver?.({ payload: null });
  expect(report).toHaveBeenCalledOnce();
});

test("malformed events are also isolated when no error callback is provided", async () => {
  let deliver: ((event: { payload: unknown }) => void) | undefined;
  native.listen.mockImplementation(async (_name, callback) => {
    deliver = callback;
    return vi.fn<() => void>();
  });
  native.invoke.mockResolvedValue(snapshot());
  const stop = await connect(vi.fn<(value: Snapshot) => void>());
  expect(() => deliver?.({ payload: null })).not.toThrow();
  stop();
});

test("transcript limit counts UTF-8 bytes consistently with the native 256 KiB limit", () => {
  const value = snapshot();
  value.session.text = "ä".repeat(131072);
  expect(decodeSnapshot(value).session.text).toHaveLength(131072);
  value.session.text += "ä";
  expect(() => decodeSnapshot(value)).toThrow("Ungültiger Anwendungsstatus");
});
