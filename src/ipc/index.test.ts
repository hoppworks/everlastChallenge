import { beforeEach, expect, test, vi } from "vitest";
import {
  clearTranscript,
  copyTranscript,
  command,
  connect,
  decodeSnapshot,
  sessionCommand,
  saveSettings,
} from "./index";
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
    settingsRequest: 0,
    preferences: {
      current: {
        device: null,
        language: "german",
        hotkey: {
          key: "D",
          option: true,
          command: false,
          control: false,
          shift: false,
        },
      },
      devices: [],
      error: null,
      refreshing: false,
      needsRestart: false,
      recoveryRequired: false,
      languages: ["german"],
      keyLabels: "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("").map((key) => ({
        key: key as Snapshot["preferences"]["keyLabels"][number]["key"],
        plain: key,
        command: key,
      })),
    },
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
      maximumReached: false,
      lastHeldSession: null,
      elapsedMillis: 0,
      levelPermille: 0,
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

test("accepts the bounded native settings request counter", () => {
  const value = snapshot();
  expect(decodeSnapshot(value).settingsRequest).toBe(0);
  value.settingsRequest = 4_294_967_295;
  expect(decodeSnapshot(value).settingsRequest).toBe(4_294_967_295);
});

test.each([-1, 0.5, Number.NaN, 4_294_967_296])(
  "rejects invalid native settings request counter %s",
  (counter) => {
    expect(() =>
      decodeSnapshot({ ...snapshot(), settingsRequest: counter }),
    ).toThrow("Ungültiger Anwendungsstatus");
  },
);

test("accepts bounded live telemetry and available preference choices", () => {
  const value = snapshot();
  value.session.elapsedMillis = 300_000;
  value.session.levelPermille = 1000;
  value.preferences.devices = [{ name: "Externes Mikrofon", available: true }];
  value.preferences.current.device = "Externes Mikrofon";
  expect(decodeSnapshot(value)).toEqual(value);
});

test.each([
  { elapsedMillis: -1 },
  { elapsedMillis: 300_001 },
  { elapsedMillis: 1.5 },
  { levelPermille: -1 },
  { levelPermille: 1001 },
  { levelPermille: NaN },
])("rejects out-of-range live telemetry %#", (patch) => {
  const value = snapshot();
  expect(() =>
    decodeSnapshot({ ...value, session: { ...value.session, ...patch } }),
  ).toThrow("Ungültiger Anwendungsstatus");
});

test.each([
  { current: null },
  { current: { ...snapshot().preferences.current, model: "small" } },
  { current: { ...snapshot().preferences.current, language: "auto" } },
  { current: { ...snapshot().preferences.current, device: " bad " } },
  { current: { ...snapshot().preferences.current, device: "x".repeat(513) } },
  {
    current: {
      ...snapshot().preferences.current,
      hotkey: {
        key: "D",
        option: false,
        command: false,
        control: false,
        shift: true,
      },
    },
  },
  {
    current: {
      ...snapshot().preferences.current,
      hotkey: { ...snapshot().preferences.current.hotkey, key: "F1" },
    },
  },
  {
    devices: Array.from({ length: 129 }, () => ({
      name: "Mic",
      available: true,
    })),
  },
  { devices: [{ name: "Mic", available: "true" }] },
  { devices: [{ name: "Mic\n", available: true }] },
  { devices: [{ name: "Mic", available: true, extra: true }] },
  { languages: ["auto"] },
  { languages: ["german", "german"] },
  { languages: [] },
  { refreshing: 1 },
  { needsRestart: null },
  { error: false },
  { extra: true },
])("rejects malformed preferences %#", (patch) => {
  const value = snapshot();
  expect(() =>
    decodeSnapshot({
      ...value,
      preferences: { ...value.preferences, ...patch },
    }),
  ).toThrow("Ungültiger Anwendungsstatus");
});

test("saves only a validated preferences draft using the settings command", async () => {
  native.invoke.mockResolvedValue(undefined);
  const draft = snapshot().preferences.current;
  await saveSettings(draft);
  expect(native.invoke).toHaveBeenCalledWith("settings_save", {
    settings: draft,
  });
  await expect(saveSettings({ ...draft, device: " bad " })).rejects.toThrow(
    "Ungültige Einstellungen",
  );
  expect(native.invoke).toHaveBeenCalledTimes(1);
  await command("devices_refresh");
  expect(native.invoke).toHaveBeenLastCalledWith("devices_refresh");
});

test("accepts explicit startup recovery state and its dedicated reset command", async () => {
  const value = snapshot();
  value.preferences.recoveryRequired = true;
  value.preferences.error = "Einstellungen können nicht gelesen werden";
  expect(decodeSnapshot(value).preferences.recoveryRequired).toBe(true);
  expect(() =>
    decodeSnapshot({
      ...value,
      preferences: { ...value.preferences, recoveryRequired: 1 },
    }),
  ).toThrow("Ungültiger Anwendungsstatus");
  native.invoke.mockResolvedValue(undefined);
  await command("settings_reset");
  expect(native.invoke).toHaveBeenCalledWith("settings_reset");
});

test("accepts all physical layout labels but rejects incomplete or duplicated choices", () => {
  const value = snapshot();
  expect(decodeSnapshot(value).preferences.keyLabels).toHaveLength(26);
  expect(() =>
    decodeSnapshot({
      ...value,
      preferences: {
        ...value.preferences,
        keyLabels: value.preferences.keyLabels.slice(1),
      },
    }),
  ).toThrow("Ungültiger Anwendungsstatus");
  const duplicate = [...value.preferences.keyLabels];
  duplicate[25] = { key: "A", plain: "A", command: "A" };
  expect(() =>
    decodeSnapshot({
      ...value,
      preferences: { ...value.preferences, keyLabels: duplicate },
    }),
  ).toThrow("Ungültiger Anwendungsstatus");
});

test.each(["", "x".repeat(129), "bad\nlabel"])(
  "rejects invalid native key labels %j",
  (plain) => {
    const value = snapshot();
    const keyLabels = value.preferences.keyLabels.map((entry) =>
      entry.key === "D" ? { ...entry, plain } : entry,
    );
    expect(() =>
      decodeSnapshot({
        ...value,
        preferences: { ...value.preferences, keyLabels },
      }),
    ).toThrow("Ungültiger Anwendungsstatus");
  },
);

test("accepts a persistent Held session and maximum-stop fact, rejecting malformed identities", () => {
  const value = snapshot();
  const session = {
    ...value.session,
    lastHeldSession: 7,
    maximumReached: true,
  };
  expect(decodeSnapshot({ ...value, session }).session).toEqual(session);
  for (const lastHeldSession of [0, -1, 0.5, 9_007_199_254_740_992, "7"]) {
    expect(() =>
      decodeSnapshot({ ...value, session: { ...session, lastHeldSession } }),
    ).toThrow("Ungültiger Anwendungsstatus");
  }
  expect(() =>
    decodeSnapshot({ ...value, session: { ...session, maximumReached: 1 } }),
  ).toThrow("Ungültiger Anwendungsstatus");
});

test("clear guards the displayed result instead of unconditionally deleting a newer transcript", async () => {
  native.invoke.mockResolvedValue(undefined);
  await clearTranscript("Angezeigter Text");
  expect(native.invoke).toHaveBeenCalledWith("transcript_clear", {
    expectedText: "Angezeigter Text",
  });
});
test("qualified hybrid settings are accepted but the experimental decoder is rejected", async () => {
  const settings = {
    ...snapshot().preferences.current,
    model: "mlx_reference_encoder" as const,
  };
  await saveSettings(settings);
  expect(native.invoke).toHaveBeenCalledWith("settings_save", { settings });
  await expect(
    saveSettings({ ...settings, model: "mlx_fp16" } as never),
  ).rejects.toThrow("Ungültige Einstellungen");
});

test("snapshot decoding accepts the saved qualified hybrid model without widening provenance", () => {
  const value = snapshot();
  value.preferences.current.model = "mlx_reference_encoder";
  value.setup.model.downloadBytes = 3_813_494_037;
  value.setup.totalBytes = 3_813_494_037;
  expect(decodeSnapshot(value).preferences.current.model).toBe(
    "mlx_reference_encoder",
  );
  value.setup.model.sourceUrl = "https://untrusted.test/model";
  expect(() => decodeSnapshot(value)).toThrow("Ungültiger Anwendungsstatus");
});

test("copy sends exactly the displayed text without fetching a later snapshot", async () => {
  native.invoke.mockResolvedValue(undefined);
  await copyTranscript("Angezeigter Text A");
  expect(native.invoke).toHaveBeenCalledExactlyOnceWith("transcript_copy", {
    text: "Angezeigter Text A",
  });
});
test.each(["", "ä".repeat(131073)])(
  "copy rejects empty or oversized UTF-8 text",
  async (value) => {
    await expect(copyTranscript(value)).rejects.toThrow(
      "Der Text ist leer oder zu lang",
    );
    expect(native.invoke).not.toHaveBeenCalled();
  },
);
