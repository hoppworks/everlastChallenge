import { beforeEach, expect, test, vi } from "vitest";
import { resizeWindow } from "./window";
const native = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  innerSize: vi.fn<() => Promise<{ width: number; height: number }>>(),
  scaleFactor: vi.fn<() => Promise<number>>(),
  setSize: vi.fn<(value: { width: number; height: number }) => Promise<void>>(),
}));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: native.isTauri }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => native,
  LogicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
}));
beforeEach(() => {
  vi.resetAllMocks();
  native.isTauri.mockReturnValue(true);
  native.innerSize.mockResolvedValue({ width: 1200, height: 480 });
  native.scaleFactor.mockResolvedValue(2);
  native.setSize.mockResolvedValue(undefined);
});
test("changes logical height without discarding a user-selected Retina width", async () => {
  await resizeWindow("settings");
  expect(native.setSize).toHaveBeenLastCalledWith({ width: 600, height: 620 });
  await resizeWindow("compact");
  expect(native.setSize).toHaveBeenLastCalledWith({ width: 600, height: 240 });
});
test("an obsolete size read cannot override a newer layout", async () => {
  let finish: ((size: { width: number; height: number }) => void) | undefined;
  native.innerSize.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const old = resizeWindow("settings");
  await resizeWindow("compact");
  finish?.({ width: 1600, height: 1000 });
  await old;
  expect(native.setSize).toHaveBeenCalledTimes(1);
  expect(native.setSize).toHaveBeenCalledWith({ width: 600, height: 240 });
});
test("window failures remain observable and a browser preview never calls native APIs", async () => {
  native.setSize.mockRejectedValueOnce(new Error("window closed"));
  await expect(resizeWindow("expanded")).rejects.toThrow("window closed");
  native.isTauri.mockReturnValue(false);
  await resizeWindow("compact");
  expect(native.setSize).toHaveBeenCalledTimes(1);
});
