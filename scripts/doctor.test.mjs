import assert from "node:assert/strict";
import test from "node:test";
import { inspectPrerequisites } from "./doctor.mjs";

const versions = {
  sw_vers: "27.0.1",
  rustup: "rustup 1.29.0",
  rustc: "rustc 1.99.1 (qualified)",
  cargo: "cargo 1.99.1 (qualified)",
  cmake: "cmake version 4.4.4",
  xcodebuild: "Xcode 27.1\nBuild version 27A9275",
  xcrun: "/qualified/Metal.xctoolchain/usr/bin/metal",
};
function inspect(overrides = {}, options = {}) {
  return inspectPrerequisites({
    platform: "darwin",
    arch: "arm64",
    node: "22.23.1",
    rust: "1.99.1",
    execute: (command, args) => {
      const tool =
        command === "rustup" && args[0] === "run" ? args[2] : command;
      return { ...versions, ...overrides }[tool] ?? null;
    },
    ...options,
  });
}
test("qualified host passes without installing or modifying anything", () => {
  assert.ok(inspect().every((row) => row.ok));
});
test("fresh-host missing prerequisites have individual actionable failures", () => {
  const rows = inspect({
    rustup: null,
    rustc: null,
    cargo: null,
    cmake: null,
    xcrun: null,
  });
  assert.deepEqual(
    rows.filter((row) => !row.ok).map((row) => row.name),
    ["rustup", "rustc", "cargo", "CMake", "Metal"],
  );
  assert.ok(rows.filter((row) => !row.ok).every((row) => row.help.length > 0));
});
test("wrong toolchain and old CMake fail even when executables exist", () => {
  const rows = inspect({
    rustc: "rustc 1.98.0",
    cmake: "cmake version 3.24.9",
  });
  assert.deepEqual(
    rows.filter((row) => !row.ok).map((row) => row.name),
    ["rustc", "CMake"],
  );
});
test("unsupported host and old Node are not qualified by matching tool versions", () => {
  assert.deepEqual(
    inspect({}, { arch: "x64", node: "22.11.0" })
      .filter((row) => !row.ok)
      .map((row) => row.name),
    ["Plattform", "Node.js"],
  );
});

test("Rust inspection never calls auto-installing rustup proxies", () => {
  const calls = [];
  inspectPrerequisites({
    platform: "darwin",
    arch: "arm64",
    node: "22.23.1",
    rust: "1.99.1",
    execute: (command, args) => {
      calls.push([command, args]);
      return null;
    },
  });
  assert.deepEqual(
    calls.filter(
      ([command, args]) => command === "rustup" && args[0] === "run",
    ),
    [
      ["rustup", ["run", "1.99.1", "rustc", "--version"]],
      ["rustup", ["run", "1.99.1", "cargo", "--version"]],
    ],
  );
  assert.ok(
    calls.every(
      ([command, args]) =>
        command !== "rustc" &&
        command !== "cargo" &&
        !args.includes("--install"),
    ),
  );
});
