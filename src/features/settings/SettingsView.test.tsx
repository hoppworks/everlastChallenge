import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { SettingsView } from "./SettingsView";
import type { SettingsViewProps } from "./SettingsView";

afterEach(cleanup);
function props(): SettingsViewProps {
  return {
    refinement: {
      available: true,
      phase: "Ready",
      downloadedBytes: 2278970666,
      totalBytes: 2278970666,
      canCancel: false,
      error: null,
    },
    onDownloadRefinement: vi.fn<SettingsViewProps["onDownloadRefinement"]>(),
    onCancelRefinementSetup:
      vi.fn<SettingsViewProps["onCancelRefinementSetup"]>(),
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
      devices: [{ name: "USB Mikrofon", available: true }],
      error: null,
      refreshing: false,
      needsRestart: false,
      recoveryRequired: false,
      languages: ["german"],
      keyLabels: "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("").map((key) => ({
        key: key as SettingsViewProps["preferences"]["keyLabels"][number]["key"],
        plain: key,
        command: key,
      })),
    },
    setup: {
      phase: "Ready",
      model: {
        name: "large-v3-turbo-q5_0",
        downloadBytes: 574041195,
        sourceUrl: "https://example.test/model",
      },
    },
    permissions: { microphone: "granted", accessibility: "granted" },
    busy: false,
    onSave: vi.fn<SettingsViewProps["onSave"]>(),
    onRefresh: vi.fn<SettingsViewProps["onRefresh"]>(),
    onMicrophone: vi.fn<SettingsViewProps["onMicrophone"]>(),
    onAccessibility: vi.fn<SettingsViewProps["onAccessibility"]>(),
    onClose: vi.fn<SettingsViewProps["onClose"]>(),
  };
}
test("requests native setup and displays its saved activation without another save", () => {
  const p = props();
  p.refinement = { ...p.refinement, phase: "Missing", downloadedBytes: 0 };
  const view = render(<SettingsView {...p} />);
  fireEvent.click(screen.getByRole("button", { name: "Textbereinigung" }));
  expect(
    (
      screen.getByRole("switch", {
        name: "Textbereinigung verwenden",
      }) as HTMLInputElement
    ).disabled,
  ).toBe(true);
  fireEvent.click(
    screen.getByRole("button", { name: "Einrichten und aktivieren" }),
  );
  expect(p.onDownloadRefinement).toHaveBeenCalledOnce();
  expect(p.onSave).not.toHaveBeenCalled();
  view.rerender(
    <SettingsView
      {...p}
      preferences={{
        ...p.preferences,
        current: { ...p.preferences.current, refinement: true },
      }}
      refinement={{
        ...p.refinement,
        phase: "Ready",
        downloadedBytes: 2278970666,
      }}
    />,
  );
  expect(
    (
      screen.getByRole("switch", {
        name: "Textbereinigung verwenden",
      }) as HTMLInputElement
    ).checked,
  ).toBe(true);
  expect(p.onSave).not.toHaveBeenCalled();
  expect(
    (
      screen.getByRole("button", {
        name: "Einstellungen speichern",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
});
test("download cancellation remains available while settings are busy", () => {
  const p = props();
  render(
    <SettingsView
      {...p}
      busy
      refinement={{ ...p.refinement, phase: "Downloading", canCancel: true }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Textbereinigung" }));
  const cancel = screen.getByRole("button", { name: "Einrichtung abbrechen" });
  expect((cancel as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(cancel);
  expect(p.onCancelRefinementSetup).toHaveBeenCalledOnce();
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
});
test("broken setup does not prevent switching refinement off", () => {
  const p = props();
  p.preferences.current.refinement = true;
  render(
    <SettingsView {...p} refinement={{ ...p.refinement, phase: "Failed" }} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Textbereinigung" }));
  fireEvent.click(screen.getByRole("switch"));
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  expect(p.onSave).toHaveBeenCalledWith({
    ...p.preferences.current,
    refinement: false,
  });
});
test("saves the chosen microphone and physical hotkey only after explicit submit", () => {
  const p = props();
  render(<SettingsView {...p} />);
  fireEvent.change(screen.getByLabelText("Mikrofon"), {
    target: { value: "USB Mikrofon" },
  });
  fireEvent.click(screen.getByText("Tastenkürzel ändern"));
  fireEvent.change(screen.getByLabelText("Physische Taste (US-Position)"), {
    target: { value: "K" },
  });
  fireEvent.click(screen.getByLabelText("Command"));
  expect(p.onSave).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  expect(p.onSave).toHaveBeenCalledWith({
    refinement: false,
    device: "USB Mikrofon",
    language: "german",
    hotkey: {
      key: "K",
      option: true,
      command: true,
      control: false,
      shift: false,
    },
  });
});

test("rejects missing or Shift-only modifiers without submitting", () => {
  const p = props();
  render(<SettingsView {...p} />);
  fireEvent.click(screen.getByText("Tastenkürzel ändern"));
  fireEvent.click(screen.getByLabelText("Option"));
  fireEvent.click(screen.getByLabelText("Shift"));
  expect(screen.getByRole("alert").textContent).toContain(
    "Shift allein reicht nicht",
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Einstellungen speichern",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  fireEvent.submit(
    screen
      .getByRole("button", { name: "Einstellungen speichern" })
      .closest("form")!,
  );
  expect(p.onSave).not.toHaveBeenCalled();
  fireEvent.click(screen.getByLabelText("Control"));
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  expect(p.onSave).toHaveBeenCalledOnce();
});

test("keeps an unsaved draft on unrelated snapshots and updates it for changed persisted settings", () => {
  const p = props();
  const view = render(<SettingsView {...p} />);
  fireEvent.change(screen.getByLabelText("Mikrofon"), {
    target: { value: "USB Mikrofon" },
  });
  view.rerender(
    <SettingsView
      {...p}
      preferences={{
        ...p.preferences,
        current: {
          ...p.preferences.current,
          hotkey: { ...p.preferences.current.hotkey },
        },
        refreshing: true,
      }}
    />,
  );
  expect((screen.getByLabelText("Mikrofon") as HTMLSelectElement).value).toBe(
    "USB Mikrofon",
  );
  view.rerender(
    <SettingsView
      {...p}
      preferences={{
        ...p.preferences,
        current: {
          ...p.preferences.current,
          hotkey: { ...p.preferences.current.hotkey, key: "G" },
        },
      }}
    />,
  );
  expect((screen.getByLabelText("Mikrofon") as HTMLSelectElement).value).toBe(
    "",
  );
  expect(
    (
      screen.getByLabelText(
        "Physische Taste (US-Position)",
      ) as HTMLSelectElement
    ).value,
  ).toBe("G");
});

test("retains an unavailable saved microphone and never silently selects system default", () => {
  const p = props();
  p.preferences.current.device = "Abgezogenes Mikrofon";
  render(<SettingsView {...p} />);
  const select = screen.getByLabelText("Mikrofon") as HTMLSelectElement;
  expect(select.value).toBe("Abgezogenes Mikrofon");
  expect(select.selectedOptions[0]?.disabled).toBe(true);
  expect(select.selectedOptions[0]?.textContent).toContain("nicht verfügbar");
  expect(p.onSave).not.toHaveBeenCalled();
  fireEvent.change(select, { target: { value: "" } });
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  expect(p.onSave).toHaveBeenCalledWith(
    expect.objectContaining({ device: null }),
  );
});

test("does not offer ambiguous device names or an unapproved language", () => {
  const p = props();
  p.preferences.devices = [
    { name: "USB Mikrofon", available: true },
    { name: "USB Mikrofon", available: true },
  ];
  render(<SettingsView {...p} />);
  const microphones = screen.getByLabelText("Mikrofon") as HTMLSelectElement;
  expect(microphones.options.length).toBe(2);
  expect(microphones.options[1]?.disabled).toBe(true);
  expect(screen.queryByRole("option", { name: "Englisch" })).toBeNull();
});

test.each(["busy", "refreshing", "restart"] as const)(
  "prevents refresh while %s and keeps closing available",
  (reason) => {
    const p = props();
    p.permissions = {
      microphone: "notDetermined",
      accessibility: "notDetermined",
    };
    p.busy = reason === "busy";
    p.preferences.refreshing = reason === "refreshing";
    p.preferences.needsRestart = reason === "restart";
    render(<SettingsView {...p} />);
    fireEvent.click(screen.getByRole("button", { name: "Aufnahme" }));
    expect(
      (
        screen.getByRole("button", {
          name: /Mikrofone (aktualisieren|werden gesucht)/,
        }) as HTMLButtonElement
      ).matches(":disabled"),
    ).toBe(true);
    expect(screen.getByLabelText("Mikrofon").matches(":disabled")).toBe(
      reason !== "refreshing",
    );
    fireEvent.click(screen.getByRole("button", { name: "Berechtigungen" }));
    expect(
      screen
        .getByRole("button", { name: "Mikrofonzugriff anfragen" })
        .matches(":disabled"),
    ).toBe(reason !== "refreshing");
    expect(screen.queryByText(/Bitte starte die App neu/) !== null).toBe(
      reason === "restart",
    );
    fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
    expect(p.onClose).toHaveBeenCalledOnce();
  },
);

test("refresh and permission controls call only their own actions", () => {
  const p = props();
  p.permissions = {
    microphone: "notDetermined",
    accessibility: "notDetermined",
  };
  render(<SettingsView {...p} />);
  fireEvent.click(screen.getByRole("button", { name: "Aufnahme" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofone aktualisieren" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Berechtigungen" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofonzugriff anfragen" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Freigabe anfragen" }));
  expect(p.onRefresh).toHaveBeenCalledOnce();
  expect(p.onMicrophone).toHaveBeenCalledOnce();
  expect(p.onAccessibility).toHaveBeenCalledOnce();
  expect(p.onSave).not.toHaveBeenCalled();
});

test("reports actual model setup failure and native settings errors without claiming success", () => {
  const p = props();
  p.setup.phase = "Failed";
  p.setup.error = "Modell fehlt";
  p.preferences.error = "Tastenkürzel bereits vergeben";
  render(<SettingsView {...p} />);
  fireEvent.click(screen.getByRole("button", { name: "Modell" }));
  expect(screen.getByText("Einrichtung fehlgeschlagen")).toBeDefined();
  expect(screen.getAllByRole("alert").map((node) => node.textContent)).toEqual([
    "Tastenkürzel bereits vergeben",
    "Modell fehlt",
  ]);
  expect(screen.queryByText("Für die lokale Erkennung bereit")).toBeNull();
});

test("explains a normalized ambiguous microphone without offering it for selection", () => {
  const p = props();
  p.preferences.current.device = "USB Mikrofon";
  p.preferences.devices = [{ name: "USB Mikrofon", available: false }];
  render(<SettingsView {...p} />);
  expect(
    screen
      .getByRole("option", { name: "USB Mikrofon (nicht eindeutig)" })
      .matches(":disabled"),
  ).toBe(true);
});

test("layout labels change while option values and the unsaved physical selection stay fixed", () => {
  const p = props();
  p.preferences.keyLabels = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    .split("")
    .map((key) => ({
      key: key as SettingsViewProps["preferences"]["keyLabels"][number]["key"],
      plain: key === "D" ? "E" : key,
      command: key,
    }));
  const view = render(<SettingsView {...p} />);
  const select = screen.getByLabelText(
    "Physische Taste (US-Position)",
  ) as HTMLSelectElement;
  expect(select.value).toBe("D");
  expect(
    screen.getByRole("option", { name: "E (US-Position D)" }),
  ).toBeTruthy();
  fireEvent.click(screen.getByLabelText("Command"));
  expect(
    screen.getByRole("option", { name: "D (US-Position D)" }),
  ).toBeTruthy();
  p.preferences = {
    ...p.preferences,
    keyLabels: p.preferences.keyLabels.map((entry) => ({
      ...entry,
      command: entry.key === "D" ? "X" : entry.command,
    })),
  };
  view.rerender(<SettingsView {...p} />);
  expect(select.value).toBe("D");
  expect(
    screen.getByRole("option", { name: "X (US-Position D)" }),
  ).toBeTruthy();
  expect((screen.getByLabelText("Command") as HTMLInputElement).checked).toBe(
    true,
  );
  expect(p.onSave).not.toHaveBeenCalled();
});

test("denied microphone explains recovery while accessibility retains its explicit request", () => {
  const p = props();
  p.permissions = { microphone: "denied", accessibility: "denied" };
  render(<SettingsView {...p} />);
  expect(
    screen.getByText(
      /Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon/,
    ),
  ).toBeDefined();
  expect(
    screen.getByText(
      /Systemeinstellungen → Datenschutz & Sicherheit → Bedienungshilfen/,
    ),
  ).toBeDefined();
  expect(
    screen.getAllByText(/aktiviere Everlast Coding Challenge/),
  ).toHaveLength(2);
  expect(
    screen.queryByRole("button", { name: "Mikrofonzugriff anfragen" }),
  ).toBeNull();
  expect(
    screen.getByRole("button", { name: "Freigabe anfragen" }),
  ).toBeDefined();
  expect(
    screen.queryByRole("button", { name: "Mikrofonstatus prüfen" }),
  ).toBeNull();
  expect(p.onMicrophone).not.toHaveBeenCalled();
  expect(p.onAccessibility).not.toHaveBeenCalled();
});

test("restricted permissions explain system restrictions without a self-approval promise", () => {
  const p = props();
  p.permissions = { microphone: "restricted", accessibility: "restricted" };
  render(<SettingsView {...p} />);
  expect(screen.getAllByText(/System- oder Verwaltungsvorgaben/)).toHaveLength(
    2,
  );
  expect(screen.queryByText(/aktiviere Everlast Coding Challenge/)).toBeNull();
  expect(
    screen.queryByRole("button", { name: /anfragen|freigeben/ }),
  ).toBeNull();
  expect(p.onMicrophone).not.toHaveBeenCalled();
  expect(p.onAccessibility).not.toHaveBeenCalled();
});

test("shows one settings area and preserves the recording draft across navigation", () => {
  const p = props();
  render(<SettingsView {...p} />);
  fireEvent.change(screen.getByLabelText("Mikrofon"), {
    target: { value: "USB Mikrofon" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Modell" }));
  expect(screen.queryByRole("combobox", { name: "Mikrofon" })).toBeNull();
  expect(screen.getByText("large-v3-turbo-q5_0")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Aufnahme" }));
  expect((screen.getByLabelText("Mikrofon") as HTMLSelectElement).value).toBe(
    "USB Mikrofon",
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  expect(p.onSave).toHaveBeenCalledWith(
    expect.objectContaining({ device: "USB Mikrofon" }),
  );
});

test("opens missing permissions first and offers the existing explicit AX request when denied", () => {
  const p = props();
  p.permissions.accessibility = "denied";
  render(<SettingsView {...p} />);
  expect(screen.queryByRole("combobox", { name: "Mikrofon" })).toBeNull();
  expect(
    screen.getByText(
      /Systemeinstellungen → Datenschutz & Sicherheit → Bedienungshilfen/,
    ),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Freigabe anfragen" }));
  expect(p.onAccessibility).toHaveBeenCalledOnce();
  expect(p.onMicrophone).not.toHaveBeenCalled();
});

test("navigation and closing remain available while editing is unsafe", () => {
  const p = props();
  p.busy = true;
  render(<SettingsView {...p} />);
  expect(screen.getByLabelText("Mikrofon").matches(":disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Modell" }));
  expect(screen.getByText("large-v3-turbo-q5_0")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Berechtigungen" }));
  expect(screen.getByText("Bedienungshilfen (Accessibility)")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Zurück zum Diktat" }));
  expect(p.onClose).toHaveBeenCalledOnce();
});

test("advanced controls start collapsed and reveal physical-key and source details on demand", () => {
  render(<SettingsView {...props()} />);
  const hotkey = screen.getByText("Tastenkürzel ändern").closest("details")!;
  expect(hotkey.open).toBe(false);
  fireEvent.click(screen.getByText("Tastenkürzel ändern"));
  expect(hotkey.open).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Modell" }));
  const model = screen.getByText("Modelldetails").closest("details")!;
  expect(model.open).toBe(false);
  fireEvent.click(screen.getByText("Modelldetails"));
  expect(model.open).toBe(true);
  expect(
    screen
      .getByRole("link", { name: "Modellquelle öffnen" })
      .getAttribute("href"),
  ).toBe("https://example.test/model");
});

test("command errors remain visible in every area and permission requests have their own busy gate", () => {
  const p = props();
  p.busy = true;
  p.permissions.accessibility = "denied";
  render(
    <SettingsView
      {...p}
      error="Freigabe konnte nicht angefragt werden"
      permissionsBusy={false}
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain(
    "Freigabe konnte nicht angefragt werden",
  );
  const request = screen.getByRole("button", { name: "Freigabe anfragen" });
  expect(request.matches(":disabled")).toBe(false);
  fireEvent.click(request);
  expect(p.onAccessibility).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "Aufnahme" }));
  expect(screen.getByLabelText("Mikrofon").matches(":disabled")).toBe(true);
  expect(screen.getByRole("alert").textContent).toContain(
    "Freigabe konnte nicht angefragt werden",
  );
});
test("hybrid model is fixed and settings offer no engine switch", () => {
  const p = props();
  render(<SettingsView {...p} />);
  fireEvent.click(screen.getByRole("button", { name: "Modell" }));
  expect(screen.queryByRole("combobox")).toBeNull();
  expect(
    screen.queryByRole("button", { name: "Modellwahl speichern" }),
  ).toBeNull();
  expect(
    screen.getByText("MLX-Hybrid · optimierter Encoder, whisper.cpp-Decoder"),
  ).toBeTruthy();
  expect(p.onSave).not.toHaveBeenCalled();
});
