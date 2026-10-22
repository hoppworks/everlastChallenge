import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { App } from "./App";
import type { Snapshot } from "./ipc";
import { resizeWindow } from "./ipc/window";

const ipc = vi.hoisted(() => ({
  copyTranscript: vi.fn<(text: string) => Promise<void>>(),
  clearTranscript: vi.fn<typeof import("./ipc").clearTranscript>(),
  connect: vi.fn<typeof import("./ipc").connect>(),
  command: vi.fn<typeof import("./ipc").command>(),
  sessionCommand: vi.fn<typeof import("./ipc").sessionCommand>(),
  saveSettings: vi.fn<typeof import("./ipc").saveSettings>(),
}));
vi.mock("./ipc", () => ipc);
vi.mock("./ipc/window", () => ({
  resizeWindow: vi
    .fn<typeof import("./ipc/window").resizeWindow>()
    .mockResolvedValue(undefined),
}));

function snapshot(): Snapshot {
  return {
    refinement: {
      setup: {
        available: true,
        probePassed: true,
        phase: "Ready",
        downloadedBytes: 2278970666,
        totalBytes: 2278970666,
        canCancel: false,
        error: null,
      },
      sessionId: null,
      original: null,
      status: "Disabled",
      message: null,
    },
    revision: 1,
    running: true,
    settingsBusy: false,
    settingsRequest: 0,
    preferences: {
      current: {
        refinement: false,
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
          "https://huggingface.co/ggerganov/whisper.cpp/resolve/revision/model.bin",
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

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(resizeWindow).mockResolvedValue(undefined);
  ipc.command.mockResolvedValue(undefined);
  ipc.copyTranscript.mockResolvedValue(undefined);
  ipc.clearTranscript.mockResolvedValue(undefined);
  ipc.sessionCommand.mockResolvedValue(undefined);
  ipc.saveSettings.mockResolvedValue(undefined);
});
afterEach(cleanup);

function connected(value = snapshot()) {
  let receive: ((snapshot: Snapshot) => void) | undefined;
  const unsubscribe = vi.fn<() => void>();
  ipc.connect.mockImplementation(
    async (callback: (snapshot: Snapshot) => void) => {
      receive = callback;
      callback(value);
      return unsubscribe;
    },
  );
  const view = render(<App />);
  return {
    ...view,
    unsubscribe,
    publish: (next: Snapshot) => act(() => receive?.(next)),
  };
}

test("native settings requests survive initial delivery and only reopen for a higher counter", async () => {
  const initial = { ...snapshot(), settingsRequest: 7 };
  const view = connected(initial);
  expect(
    await screen.findByRole("region", { name: "Einstellungen" }),
  ).toBeDefined();
  expect(screen.queryByRole("button", { name: "Einstellungen" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
  expect(document.activeElement).toBe(
    screen.getByRole("heading", { name: "Einstellungen" }),
  );
  const permission = screen.getByRole("button", {
    name: "Mikrofone aktualisieren",
  });
  permission.focus();
  view.publish({ ...initial, revision: 2, settingsRequest: 8 });
  expect(document.activeElement).toBe(permission);
  fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
  view.publish({ ...initial, revision: 3, settingsRequest: 8 });
  view.publish({ ...initial, revision: 4, settingsRequest: 6 });
  expect(screen.queryByRole("region", { name: "Einstellungen" })).toBeNull();
  act(() => {
    view.publish({ ...initial, revision: 5, settingsRequest: 9 });
    view.publish({ ...initial, revision: 6, settingsRequest: 8 });
  });
  expect(screen.getByRole("region", { name: "Einstellungen" })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
  view.publish({ ...initial, revision: 7, settingsRequest: 9 });
  expect(screen.queryByRole("region", { name: "Einstellungen" })).toBeNull();
  expect(ipc.command).not.toHaveBeenCalled();
  expect(ipc.sessionCommand).not.toHaveBeenCalled();
});

test("renders startup without claiming readiness before the first native snapshot", () => {
  ipc.connect.mockReturnValue(new Promise(() => {}));
  render(<App />);
  expect(screen.getByRole("status").textContent).toBe(
    "Anwendung wird gestartet …",
  );
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
});

test("subscribes once, accepts native updates, and unsubscribes on unmount", async () => {
  const view = connected();
  await screen.findByRole("button", { name: "Aufnehmen" });
  expect(ipc.connect).toHaveBeenCalledOnce();
  const next = snapshot();
  next.session = { ...next.session, phase: "Starting", id: 9 };
  view.publish(next);
  expect(screen.getByRole("status").textContent).toBe(
    "Mikrofon wird gestartet",
  );
  await act(async () => {});
  view.unmount();
  expect(view.unsubscribe).toHaveBeenCalledOnce();
  view.publish(snapshot());
  expect(screen.queryByRole("main")).toBeNull();
});

test("late subscription completion after unmount is immediately disconnected", async () => {
  let finish: ((stop: () => void) => void) | undefined;
  let receive: ((value: Snapshot) => void) | undefined;
  ipc.connect.mockImplementation((callback: (value: Snapshot) => void) => {
    receive = callback;
    return new Promise<() => void>((resolve) => {
      finish = resolve;
    });
  });
  const view = render(<App />);
  view.unmount();
  const unsubscribe = vi.fn<() => void>();
  await act(async () => {
    receive?.(snapshot());
    finish?.(unsubscribe);
  });
  expect(unsubscribe).toHaveBeenCalledOnce();
  expect(screen.queryByRole("main")).toBeNull();
});

test("connection errors are visible without synthesizing a ready snapshot", async () => {
  ipc.connect.mockRejectedValue(new Error("Verbindung fehlgeschlagen"));
  render(<App />);
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Verbindung fehlgeschlagen",
  );
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
});

test("late connection rejection after disposal does not render an error", async () => {
  let reject: ((reason: Error) => void) | undefined;
  ipc.connect.mockImplementation(
    () =>
      new Promise((_, fail) => {
        reject = fail;
      }),
  );
  const view = render(<App />);
  view.unmount();
  await act(async () => reject?.(new Error("late")));
  expect(screen.queryByRole("alert")).toBeNull();
});

test("start forwards one command and waits for native recording state", async () => {
  let resolve: (() => void) | undefined;
  ipc.command.mockImplementation(
    () =>
      new Promise<void>((finish) => {
        resolve = finish;
      }),
  );
  const view = connected();
  fireEvent.click(await screen.findByRole("button", { name: "Aufnehmen" }));
  expect(ipc.command).toHaveBeenCalledWith("recording_start");
  expect(screen.getByRole("status").textContent).toBe("Bereit zum Diktieren");
  expect(
    (screen.getByRole("button", { name: "Aufnehmen" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  const next = snapshot();
  next.session = { ...next.session, phase: "Recording", id: 17 };
  view.publish(next);
  fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));
  expect(ipc.sessionCommand).toHaveBeenCalledWith("recording_cancel", 17);
  await act(async () => resolve?.());
});

test.each([
  ["Stoppen", "recording_stop"],
  ["Abbrechen", "recording_cancel"],
] as const)(
  "%s forwards the current session identifier",
  async (button, command) => {
    const value = snapshot();
    value.session = { ...value.session, phase: "Recording", id: 42 };
    connected(value);
    fireEvent.click(await screen.findByRole("button", { name: button }));
    await waitFor(() =>
      expect(ipc.sessionCommand).toHaveBeenCalledWith(command, 42),
    );
  },
);

test("missing session identifiers never issue a targeted native action", async () => {
  const value = snapshot();
  value.session.phase = "Recording";
  connected(value);
  fireEvent.click(await screen.findByRole("button", { name: "Stoppen" }));
  fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));
  expect(ipc.sessionCommand).not.toHaveBeenCalled();
});

test("copy and clear preserve displayed text until the native snapshot changes", async () => {
  const value = snapshot();
  value.session.text = "Der gespeicherte Text.";
  const view = connected(value);
  fireEvent.click(await screen.findByRole("button", { name: "Kopieren" }));
  await waitFor(() =>
    expect(ipc.copyTranscript).toHaveBeenCalledWith("Der gespeicherte Text."),
  );
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "Löschen" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Löschen" }));
  await waitFor(() =>
    expect(ipc.clearTranscript).toHaveBeenCalledWith("Der gespeicherte Text."),
  );
  expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
    "Der gespeicherte Text.",
  );
  view.publish(snapshot());
  expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("");
});

test("download and cancellation forward explicit model commands", async () => {
  const value = snapshot();
  value.setup.phase = "Missing";
  const view = connected(value);
  fireEvent.click(await screen.findByRole("button", { name: "Modell laden" }));
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("model_download"),
  );
  value.setup = {
    ...value.setup,
    phase: "Downloading",
    downloadedBytes: 123000000,
    canCancel: true,
  };
  view.publish({ ...value, revision: 2 });
  fireEvent.click(
    screen.getByRole("button", { name: "Einrichtung abbrechen" }),
  );
  await waitFor(() => expect(ipc.command).toHaveBeenCalledWith("model_cancel"));
});

test("permission panel forwards user requests, respects busy, and can be closed", async () => {
  const initial = snapshot();
  initial.permissions = {
    microphone: "notDetermined",
    accessibility: "notDetermined",
  };
  const view = connected(initial);
  fireEvent.click(await screen.findByRole("button", { name: "Einstellungen" }));
  expect(screen.getByRole("region", { name: "Einstellungen" })).toBeDefined();
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofonzugriff anfragen" }),
  );
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("microphone_request"),
  );
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Freigabe anfragen",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Freigabe anfragen" }));
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("accessibility_request"),
  );
  await waitFor(() => expect(ipc.command).toHaveBeenCalledTimes(2));
  view.publish({ ...initial, settingsBusy: true });
  for (const name of ["Mikrofonzugriff anfragen", "Freigabe anfragen"]) {
    expect(
      (screen.getByRole("button", { name }) as HTMLButtonElement).disabled,
    ).toBe(false);
  }
  view.publish({
    ...initial,
    session: { ...initial.session, phase: "Recording" },
  });
  for (const name of ["Mikrofonzugriff anfragen", "Freigabe anfragen"]) {
    expect(
      (screen.getByRole("button", { name }) as HTMLButtonElement).disabled,
    ).toBe(true);
  }
  fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
  expect(screen.queryByRole("region", { name: "Einstellungen" })).toBeNull();
});

test("native outcome and error remain visible and a failed command supplies a recoverable error", async () => {
  const value = snapshot();
  value.error = "Native Warnung";
  value.session = {
    ...value.session,
    text: "Text bleibt erhalten",
    outcome: { kind: "held", message: "Fokus geändert" },
  };
  const view = connected(value);
  expect((await screen.findByRole("alert")).textContent).toBe("Native Warnung");
  expect(screen.getByRole("status").textContent).toBe(
    "Text wurde zurückgehalten",
  );
  ipc.copyTranscript.mockRejectedValueOnce(
    new Error("Kopieren fehlgeschlagen"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Kopieren" }));
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toContain(
      "Kopieren fehlgeschlagen",
    ),
  );
  view.publish({ ...value, error: null });
  fireEvent.click(screen.getByRole("button", { name: "Erneut aufnehmen" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
});

test("subscription errors remain visible without replacing the last valid snapshot", async () => {
  let report: ((error: unknown) => void) | undefined;
  ipc.connect.mockImplementation(async (receive, onError) => {
    report = onError;
    receive(snapshot());
    return vi.fn<() => void>();
  });
  const view = render(<App />);
  await screen.findByRole("button", { name: "Aufnehmen" });
  act(() => report?.(new Error("Ungültiger Anwendungsstatus")));
  expect(screen.getByRole("alert").textContent).toContain(
    "Ungültiger Anwendungsstatus",
  );
  expect(screen.getByRole("status").textContent).toBe("Bereit zum Diktieren");
  view.unmount();
  act(() => report?.(new Error("Verspäteter Fehler")));
  expect(screen.queryByRole("alert")).toBeNull();
});

test("settings save and refresh use explicit native actions and retain draft on rejected save", async () => {
  connected();
  fireEvent.click(await screen.findByRole("button", { name: "Einstellungen" }));
  fireEvent.change(screen.getByLabelText("Physische Taste (US-Position)"), {
    target: { value: "K" },
  });
  ipc.saveSettings.mockRejectedValueOnce(new Error("Tastenkürzel belegt"));
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toContain(
      "Tastenkürzel belegt",
    ),
  );
  expect(ipc.saveSettings).toHaveBeenCalledWith(
    expect.objectContaining({
      hotkey: {
        key: "K",
        option: true,
        command: false,
        control: false,
        shift: false,
      },
    }),
  );
  expect(
    (
      screen.getByLabelText(
        "Physische Taste (US-Position)",
      ) as HTMLSelectElement
    ).value,
  ).toBe("K");
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofone aktualisieren" }),
  );
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("devices_refresh"),
  );
});

test.each(["session", "setup"])(
  "settings edits are disabled during active %s",
  async (kind) => {
    const value = snapshot();
    value.settingsRequest = 1;
    if (kind === "session") value.session.phase = "Recording";
    else value.setup.phase = "Downloading";
    connected(value);
    await screen.findByRole("region", { name: "Einstellungen" });
    expect(screen.getByLabelText("Mikrofon").matches(":disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
    expect(screen.queryByRole("region", { name: "Einstellungen" })).toBeNull();
  },
);

test("native elapsed time and input level reach the recording readout", async () => {
  const value = snapshot();
  value.session = {
    ...value.session,
    phase: "Recording",
    elapsedMillis: 65000,
    levelPermille: 420,
  };
  connected(value);
  expect((await screen.findByLabelText("Aufnahmedauer")).textContent).toBe(
    "1:05",
  );
  expect(
    (screen.getByRole("meter", { name: "Mikrofonpegel" }) as HTMLMeterElement)
      .value,
  ).toBe(0.42);
});

test("invalid startup settings expose only explicit recovery and wait for native success", async () => {
  const value = snapshot();
  value.preferences.recoveryRequired = true;
  value.preferences.error = "Konfiguration konnte nicht gelesen werden";
  value.settingsRequest = 5;
  const view = connected(value);
  expect(
    await screen.findByRole("heading", {
      name: "Einstellungen können nicht geladen werden",
    }),
  ).toBe(document.activeElement);
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Modell laden" })).toBeNull();
  expect(screen.queryByRole("region", { name: "Einstellungen" })).toBeNull();
  expect(ipc.command).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
  expect(ipc.command).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("settings_reset"),
  );
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
  view.publish(snapshot());
  fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
  expect(screen.getByRole("button", { name: "Aufnehmen" })).toBeDefined();
});

test("failed explicit settings recovery can be retried without enabling recording", async () => {
  const value = snapshot();
  value.preferences.recoveryRequired = true;
  connected(value);
  let reject: ((reason: Error) => void) | undefined;
  ipc.command.mockImplementationOnce(
    () =>
      new Promise((_, fail) => {
        reject = fail;
      }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  expect(screen.getByRole("status").textContent).toContain(
    "werden zurückgesetzt",
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  expect(ipc.command).toHaveBeenCalledOnce();
  await act(async () => reject?.(new Error("Datenträger voll")));
  expect(screen.getByRole("alert").textContent).toContain("Datenträger voll");
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  await waitFor(() => expect(ipc.command).toHaveBeenCalledTimes(2));
  expect(ipc.command).toHaveBeenLastCalledWith("settings_reset");
});

test("renders the authoritative maximum-stop fact without dispatching another action", async () => {
  const value = snapshot();
  value.session.maximumReached = true;
  value.session.lastHeldSession = 7;
  value.session.outcome = { kind: "sent" };
  connected(value);
  expect(
    await screen.findByText(
      "Die Aufnahme wurde an der 5-Minuten-Grenze beendet.",
    ),
  ).toBeDefined();
  expect(
    screen.getByRole("heading", { name: "Einfügebefehl gesendet" }),
  ).toBeDefined();
  expect(ipc.command).not.toHaveBeenCalled();
  expect(ipc.sessionCommand).not.toHaveBeenCalled();
});

test("permission denial replaces the request with recovery guidance and follows refreshed snapshots", async () => {
  const value = snapshot();
  value.permissions = { microphone: "notDetermined", accessibility: "granted" };
  const view = connected(value);
  fireEvent.click(await screen.findByRole("button", { name: "Einstellungen" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofonzugriff anfragen" }),
  );
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("microphone_request"),
  );
  view.publish({
    ...value,
    revision: 2,
    permissions: { microphone: "denied", accessibility: "denied" },
  });
  expect(
    screen.queryByRole("button", { name: "Mikrofonzugriff anfragen" }),
  ).toBeNull();
  expect(
    screen.getByRole("button", { name: "Freigabe anfragen" }),
  ).toBeDefined();
  expect(
    screen.getByText(
      /Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon/,
    ),
  ).toBeDefined();
  expect(screen.getByText(/Kehre nach der Änderung/)).toBeDefined();
  expect(ipc.command).toHaveBeenCalledTimes(1);
  view.publish({
    ...value,
    revision: 3,
    permissions: { microphone: "granted", accessibility: "granted" },
  });
  expect(
    screen.queryByText(/Systemeinstellungen → Datenschutz & Sicherheit/),
  ).toBeNull();
  expect(ipc.command).toHaveBeenCalledTimes(1);
});

test("settings replace the dictation screen and restore the retained transcript on close", async () => {
  const value = snapshot();
  value.session.text = "Mein vorhandener Text";
  connected(value);
  fireEvent.click(await screen.findByRole("button", { name: "Einstellungen" }));
  expect(screen.queryByRole("textbox", { name: "Letzter Text" })).toBeNull();
  expect(screen.getByRole("heading", { name: "Einstellungen" })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
  expect(
    (
      screen.getByRole("textbox", {
        name: "Letzter Text",
      }) as HTMLTextAreaElement
    ).value,
  ).toBe("Mein vorhandener Text");
});

test("settings navigation stays available while the model is loading", async () => {
  const value = snapshot();
  value.setup.phase = "Loading";
  value.settingsBusy = true;
  connected(value);
  const button = await screen.findByRole("button", { name: "Einstellungen" });
  expect((button as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(button);
  expect(screen.getByRole("region", { name: "Einstellungen" })).toBeDefined();
});

test.each(["Idle", "Recording"] as const)(
  "a stopped service preserves text and copy but offers no dead actions after %s",
  async (phase) => {
    const value = snapshot();
    value.running = false;
    value.setup.phase = "Failed";
    value.session = {
      ...value.session,
      phase,
      text: "Letzter sichtbarer Text",
      id: phase === "Recording" ? 42 : null,
    };
    value.preferences.recoveryRequired = true;
    value.settingsRequest = 1;
    value.error = "Der Übergabedienst wurde beendet";
    connected(value);
    expect(
      await screen.findByText("Bitte beende die App und öffne sie erneut."),
    ).toBeDefined();
    expect(screen.getByRole("alert").textContent).toContain(
      "Der Übergabedienst wurde beendet",
    );
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
      "Letzter sichtbarer Text",
    );
    expect(
      screen.getAllByRole("button").map((button) => button.textContent),
    ).toEqual(["Kopieren"]);
    fireEvent.click(screen.getByRole("button", { name: "Kopieren" }));
    await waitFor(() =>
      expect(ipc.copyTranscript).toHaveBeenCalledWith(
        "Letzter sichtbarer Text",
      ),
    );
    expect(ipc.command).not.toHaveBeenCalled();
    expect(ipc.sessionCommand).not.toHaveBeenCalled();
  },
);

test("a running service retains retry for a recoverable model download failure", async () => {
  const value = snapshot();
  value.setup.phase = "Failed";
  connected(value);
  fireEvent.click(
    await screen.findByRole("button", { name: "Erneut versuchen" }),
  );
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("model_download"),
  );
});
