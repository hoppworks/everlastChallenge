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

const ipc = vi.hoisted(() => ({
  connect: vi.fn<typeof import("./ipc").connect>(),
  command: vi.fn<typeof import("./ipc").command>(),
  sessionCommand: vi.fn<typeof import("./ipc").sessionCommand>(),
}));
vi.mock("./ipc", () => ipc);

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
    },
    permissions: { microphone: "granted", accessibility: "granted" },
    labels: { device: "Standardmikrofon", language: "Deutsch", hotkey: "⌥D" },
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  ipc.command.mockResolvedValue(undefined);
  ipc.sessionCommand.mockResolvedValue(undefined);
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
    expect(ipc.command).toHaveBeenCalledWith("transcript_copy"),
  );
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "Löschen" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Löschen" }));
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("transcript_clear"),
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
  const view = connected();
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
          name: "Bedienungshilfen freigeben",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Bedienungshilfen freigeben" }),
  );
  await waitFor(() =>
    expect(ipc.command).toHaveBeenCalledWith("accessibility_request"),
  );
  view.publish({ ...snapshot(), settingsBusy: true });
  expect(
    (
      screen.getByRole("button", {
        name: "Mikrofonzugriff anfragen",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Bedienungshilfen freigeben",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Schließen" }));
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
  ipc.command.mockRejectedValueOnce(new Error("Kopieren fehlgeschlagen"));
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
