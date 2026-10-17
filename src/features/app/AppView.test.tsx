import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { AppView, type AppViewProps } from "./AppView";

afterEach(cleanup);

function fixture(): AppViewProps {
  const noop = () => {};
  return {
    setup: {
      phase: "Ready",
      model: {
        name: "large-v3-turbo-q5_0",
        downloadBytes: 574041195,
        sourceUrl: "https://huggingface.co/ggerganov/whisper.cpp",
      },
    },
    session: { phase: "Idle", text: "" },
    permissions: { microphone: "granted", accessibility: "granted" },
    labels: { device: "Standardmikrofon", language: "Deutsch", hotkey: "⌥D" },
    onDownload: noop,
    onCancelSetup: noop,
    onStart: noop,
    onStop: noop,
    onCancel: noop,
    onCopy: noop,
    onClear: noop,
    onSettings: noop,
  };
}

test("names the dictation utility and provides settings", () => {
  render(<AppView {...fixture()} />);
  expect(
    screen.getByRole("heading", { name: "Everlast Dictation" }),
  ).toBeDefined();
  expect(screen.getByRole("button", { name: "Einstellungen" })).toBeDefined();
});

test("clicking start waits for native state and does not pretend recording", () => {
  const props = fixture();
  let starts = 0;
  props.onStart = () => {
    starts += 1;
  };
  const view = render(<AppView {...props} />);
  screen.getByRole("button", { name: "Aufnehmen" }).click();
  expect(starts).toBe(1);
  expect(screen.queryByText("Aufnahme läuft")).toBeNull();
  view.rerender(
    <AppView {...props} session={{ phase: "Starting", text: "" }} />,
  );
  expect(screen.getByRole("status").textContent).toBe(
    "Mikrofon wird gestartet",
  );
  expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
});

test("shows native recording measurements and keeps stop and cancel distinct", () => {
  const props = fixture();
  let stops = 0;
  let cancels = 0;
  props.onStop = () => {
    stops += 1;
  };
  props.onCancel = () => {
    cancels += 1;
  };
  render(
    <AppView
      {...props}
      session={{
        phase: "Recording",
        text: "",
        elapsedSeconds: 275,
        level: 0.4,
        maximumWarning: true,
      }}
    />,
  );
  expect(screen.getByLabelText("Aufnahmedauer").textContent).toBe("4:35");
  expect(
    (screen.getByRole("meter", { name: "Mikrofonpegel" }) as HTMLMeterElement)
      .value,
  ).toBe(0.4);
  expect(screen.getByText(/endet bei 5 Minuten/)).toBeDefined();
  screen.getByRole("button", { name: "Stoppen" }).click();
  screen.getByRole("button", { name: "Abbrechen" }).click();
  expect([stops, cancels]).toEqual([1, 1]);
});

test.each([
  ["Starting", "Mikrofon wird gestartet", true],
  ["Finalizing", "Diktat wird verarbeitet", true],
  ["Delivering", "Text wird übergeben", true],
  ["Cancelling", "Aufnahme wird beendet", false],
] as const)(
  "%s exposes only applicable session actions",
  (phase, title, cancellable) => {
    render(<AppView {...fixture()} session={{ phase, text: "alter Text" }} />);
    expect(screen.getByRole("status").textContent).toBe(title);
    expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
    expect(Boolean(screen.queryByRole("button", { name: "Abbrechen" }))).toBe(
      cancellable,
    );
    expect(screen.queryByRole("button", { name: "Stoppen" })).toBeNull();
  },
);

test.each([
  ["Missing", "Modell einrichten", "Modell laden"],
  ["Cancelled", "Einrichtung abgebrochen", "Modell laden"],
  ["Failed", "Modell konnte nicht eingerichtet werden", "Erneut versuchen"],
] as const)(
  "%s offers an explicit model action and provenance",
  (phase, title, action) => {
    const props = fixture();
    let downloads = 0;
    props.onDownload = () => {
      downloads += 1;
    };
    render(
      <AppView
        {...props}
        setup={{ ...props.setup, phase, error: "Netzwerk nicht verfügbar" }}
      />,
    );
    expect(screen.getByRole("status").textContent).toBe(title);
    expect(
      screen.getByRole("link", { name: "Hugging Face" }).getAttribute("href"),
    ).toBe(props.setup.model.sourceUrl);
    expect(screen.getByText(/574 MB/)).toBeDefined();
    screen.getByRole("button", { name: action }).click();
    expect(downloads).toBe(1);
    expect(screen.queryByRole("button", { name: "Aufnehmen" })).toBeNull();
    expect(screen.queryByRole("alert")?.textContent ?? null).toBe(
      phase === "Failed" ? "Netzwerk nicht verfügbar" : null,
    );
  },
);

test("download renders only real byte progress and delegates cancellation", () => {
  const props = fixture();
  let cancelled = 0;
  props.onCancelSetup = () => {
    cancelled += 1;
  };
  const view = render(
    <AppView
      {...props}
      setup={{
        ...props.setup,
        phase: "Downloading",
        downloadedBytes: 2_000_000,
        totalBytes: 10_000_000,
        canCancel: true,
      }}
    />,
  );
  const progress = screen.getByRole("progressbar", {
    name: "Modelldownload",
  }) as HTMLProgressElement;
  expect(progress.value).toBe(2_000_000);
  expect(progress.max).toBe(10_000_000);
  expect(screen.getByText("2 MB von 10 MB")).toBeDefined();
  screen.getByRole("button", { name: "Einrichtung abbrechen" }).click();
  expect(cancelled).toBe(1);
  view.rerender(
    <AppView {...props} setup={{ ...props.setup, phase: "Downloading" }} />,
  );
  expect(screen.getByRole("progressbar").hasAttribute("value")).toBe(false);
  expect(
    screen.queryByRole("button", { name: "Einrichtung abbrechen" }),
  ).toBeNull();
});

test.each([
  ["Checking", "Modell wird geprüft"],
  ["Loading", "Spracherkennung wird gestartet"],
  ["Cancelling", "Einrichtung wird abgebrochen"],
] as const)("%s has a true status without invented percent", (phase, title) => {
  const props = fixture();
  render(<AppView {...props} setup={{ ...props.setup, phase }} />);
  expect(screen.getByRole("status").textContent).toBe(title);
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.queryByRole("button", { name: "Modell laden" })).toBeNull();
});

test("permission hints distinguish recording from automatic delivery", () => {
  const props = fixture();
  const view = render(
    <AppView
      {...props}
      permissions={{ microphone: "denied", accessibility: "denied" }}
    />,
  );
  expect(
    (screen.getByRole("button", { name: "Aufnehmen" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(screen.getByText(/Für Aufnahmen ist der Zugriff/)).toBeDefined();
  expect(
    screen.getByText(/Automatisches Einfügen ist noch nicht verfügbar/),
  ).toBeDefined();
  view.rerender(
    <AppView
      {...props}
      permissions={{ microphone: "granted", accessibility: "denied" }}
    />,
  );
  expect(
    (screen.getByRole("button", { name: "Aufnehmen" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
  view.rerender(
    <AppView
      {...props}
      permissions={{ microphone: "restricted", accessibility: "granted" }}
    />,
  );
  expect(
    screen.getByText(/durch Systemeinstellungen eingeschränkt/),
  ).toBeDefined();
});

test.each([
  ["local", "Diktat ist fertig"],
  ["empty", "Kein Sprachinhalt erkannt"],
  ["sent", "Einfügebefehl gesendet"],
  ["cancelled", "Diktat abgebrochen"],
  ["held", "Text wurde zurückgehalten"],
  ["failed", "Diktat konnte nicht abgeschlossen werden"],
] as const)(
  "%s preserves the last transcript and truthful outcome",
  (kind, title) => {
    const text = "Zeile eins.\n" + "Unveränderter langer Inhalt. ".repeat(100);
    const outcome =
      kind === "held" || kind === "failed"
        ? { kind, message: "Das ursprüngliche Feld ist nicht mehr aktiv." }
        : { kind };
    render(
      <AppView {...fixture()} session={{ phase: "Idle", text, outcome }} />,
    );
    expect(screen.getByRole("status").textContent).toBe(title);
    const transcript = screen.getByRole("textbox", {
      name: "Letzter Text",
    }) as HTMLTextAreaElement;
    expect(transcript.value).toBe(text);
    expect(transcript.readOnly).toBe(true);
    expect(
      screen.getByRole("button", { name: "Erneut aufnehmen" }),
    ).toBeDefined();
    expect(
      Boolean(
        screen.queryByText("Das ursprüngliche Feld ist nicht mehr aktiv."),
      ),
    ).toBe(kind === "held" || kind === "failed");
    expect(screen.queryByText("Text erfolgreich eingefügt")).toBeNull();
  },
);

test("copy, clear and settings delegate actions and respect pending requests", () => {
  const props = fixture();
  const called: string[] = [];
  props.onCopy = () => {
    called.push("copy");
  };
  props.onClear = () => {
    called.push("clear");
  };
  props.onSettings = () => {
    called.push("settings");
  };
  const view = render(
    <AppView {...props} session={{ phase: "Idle", text: "Text" }} />,
  );
  for (const name of ["Kopieren", "Löschen", "Einstellungen"])
    screen.getByRole("button", { name }).click();
  expect(called).toEqual(["copy", "clear", "settings"]);
  expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
    "Text",
  );
  view.rerender(
    <AppView
      {...props}
      pending
      settingsBusy
      error="Anfrage nicht angenommen"
    />,
  );
  for (const name of ["Kopieren", "Löschen", "Einstellungen", "Aufnehmen"])
    expect(
      (screen.getByRole("button", { name }) as HTMLButtonElement).disabled,
    ).toBe(true);
  expect(screen.getByRole("alert").textContent).toBe(
    "Anfrage nicht angenommen",
  );
});

test("pending ordinary requests never disable priority stop and cancellation", () => {
  const props = fixture();
  let stops = 0;
  let cancels = 0;
  props.onStop = () => {
    stops += 1;
  };
  props.onCancel = () => {
    cancels += 1;
  };
  const view = render(
    <AppView {...props} pending session={{ phase: "Recording", text: "" }} />,
  );
  const stop = screen.getByRole("button", {
    name: "Stoppen",
  }) as HTMLButtonElement;
  const cancel = screen.getByRole("button", {
    name: "Abbrechen",
  }) as HTMLButtonElement;
  expect(stop.disabled).toBe(false);
  expect(cancel.disabled).toBe(false);
  stop.click();
  cancel.click();
  expect([stops, cancels]).toEqual([1, 1]);
  view.rerender(
    <AppView {...props} pending session={{ phase: "Finalizing", text: "" }} />,
  );
  expect(
    (screen.getByRole("button", { name: "Abbrechen" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
  view.rerender(
    <AppView
      {...props}
      pending
      setup={{ ...props.setup, phase: "Downloading", canCancel: true }}
    />,
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Einrichtung abbrechen",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
});

test("a ready model without microphone permission is not presented as ready to dictate", () => {
  const props = fixture();
  const view = render(
    <AppView
      {...props}
      permissions={{ microphone: "denied", accessibility: "granted" }}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe(
    "Mikrofonzugriff erforderlich",
  );
  view.rerender(
    <AppView
      {...props}
      permissions={{ microphone: "denied", accessibility: "granted" }}
      session={{
        phase: "Idle",
        text: "Vorhandener Text",
        outcome: { kind: "sent" },
      }}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("Einfügebefehl gesendet");
  view.rerender(
    <AppView
      {...props}
      permissions={{ microphone: "denied", accessibility: "granted" }}
      session={{
        phase: "Idle",
        text: "Vorhandener Text",
        outcome: { kind: "held", message: "Ziel nicht verfügbar" },
      }}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe(
    "Text wurde zurückgehalten",
  );
});
