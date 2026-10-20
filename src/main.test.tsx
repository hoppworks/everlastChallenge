import { act, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

vi.mock("./App", () => ({
  App: () => <main>TypeScript-Oberfläche aktiv</main>,
}));

test("the shipped frontend entrypoint mounts the actual React application", async () => {
  document.body.innerHTML = '<div id="root"></div>';
  await act(async () => {
    await import("./main");
  });
  expect(screen.getByText("TypeScript-Oberfläche aktiv")).toBeTruthy();
});
