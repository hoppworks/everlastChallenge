import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { RecoveryView } from "./RecoveryView";

afterEach(cleanup);
test("requires explicit confirmation before resetting settings", () => {
  const reset = vi.fn<() => void>();
  render(
    <RecoveryView
      pending={false}
      reason="Ungültige Einstellungen"
      onReset={reset}
    />,
  );
  expect(reset).not.toHaveBeenCalled();
  expect(
    screen.queryByRole("button", { name: "Zurücksetzen bestätigen" }),
  ).toBeNull();
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
  expect(reset).not.toHaveBeenCalled();
  expect(
    screen.getByText(/Bereits geladene Sprachmodelle bleiben erhalten/),
  ).toBeDefined();
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  expect(reset).toHaveBeenCalledOnce();
});

test("confirmation has keyboard focus and cancelling abandons it without resetting", () => {
  const reset = vi.fn<() => void>();
  render(<RecoveryView pending={false} reason={null} onReset={reset} />);
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));
  expect(reset).not.toHaveBeenCalled();
  expect(
    screen.queryByRole("button", { name: "Zurücksetzen bestätigen" }),
  ).toBeNull();
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
});

test("pending blocks duplicate confirmation and errors preserve the chosen focus and retry", () => {
  const reset = vi.fn<() => void>();
  const view = render(
    <RecoveryView pending={false} reason="Datei ungültig" onReset={reset} />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
  const cancel = screen.getByRole("button", { name: "Abbrechen" });
  cancel.focus();
  view.rerender(
    <RecoveryView
      pending={false}
      reason="Datei weiterhin ungültig"
      error="Speichern fehlgeschlagen"
      onReset={reset}
    />,
  );
  expect(document.activeElement).toBe(cancel);
  expect(screen.getByRole("alert").textContent).toBe(
    "Speichern fehlgeschlagen",
  );
  view.rerender(
    <RecoveryView pending={true} reason="Datei ungültig" onReset={reset} />,
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Zurücksetzen bestätigen",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("button", { name: "Abbrechen" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  expect(reset).not.toHaveBeenCalled();
  expect(screen.getByRole("status").textContent).toContain(
    "werden zurückgesetzt",
  );
  view.rerender(
    <RecoveryView
      pending={false}
      reason="Datei ungültig"
      error="Speichern fehlgeschlagen"
      onReset={reset}
    />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Zurücksetzen bestätigen" }),
  );
  expect(reset).toHaveBeenCalledOnce();
});

test("an externally pending recovery cannot open its confirmation", () => {
  const reset = vi.fn<() => void>();
  render(<RecoveryView pending={true} reason={null} onReset={reset} />);
  fireEvent.click(
    screen.getByRole("button", { name: "Einstellungen zurücksetzen" }),
  );
  expect(
    screen.queryByRole("button", { name: "Zurücksetzen bestätigen" }),
  ).toBeNull();
  expect(reset).not.toHaveBeenCalled();
});
