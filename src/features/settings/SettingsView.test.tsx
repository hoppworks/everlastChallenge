import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { SettingsView } from "./SettingsView";
import type { SettingsViewProps } from "./SettingsView";

afterEach(cleanup);
function props(): SettingsViewProps {
  return {
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
      devices: [{ name: "USB Mikrofon", available: true }],
      error: null,
      refreshing: false,
      needsRestart: false,
      languages: ["german"],
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
test("saves the chosen microphone and physical hotkey only after explicit submit", () => {
  const p = props();
  render(<SettingsView {...p} />);
  fireEvent.change(screen.getByLabelText("Mikrofon"), {
    target: { value: "USB Mikrofon" },
  });
  fireEvent.change(screen.getByLabelText("Physische Taste (US-Position)"), {
    target: { value: "K" },
  });
  fireEvent.click(screen.getByLabelText("Command"));
  expect(p.onSave).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen speichern" }),
  );
  expect(p.onSave).toHaveBeenCalledWith({
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
    p.busy = reason === "busy";
    p.preferences.refreshing = reason === "refreshing";
    p.preferences.needsRestart = reason === "restart";
    render(<SettingsView {...p} />);
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
    expect(
      screen
        .getByRole("button", { name: "Mikrofonzugriff anfragen" })
        .matches(":disabled"),
    ).toBe(reason !== "refreshing");
    expect(screen.queryByText(/Bitte starte die App neu/) !== null).toBe(
      reason === "restart",
    );
    fireEvent.click(screen.getByRole("button", { name: "Schließen" }));
    expect(p.onClose).toHaveBeenCalledOnce();
  },
);

test("refresh and permission controls call only their own actions", () => {
  const p = props();
  render(<SettingsView {...p} />);
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofone aktualisieren" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Mikrofonzugriff anfragen" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Bedienungshilfen freigeben" }),
  );
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
