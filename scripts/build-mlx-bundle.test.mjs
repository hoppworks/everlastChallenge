import { test } from "node:test";
import assert from "node:assert/strict";
import { linkedMetallib, builtExecutable } from "./build-mlx-bundle.mjs";
test("worker path comes from the exact Cargo executable artifact", () => {
  const row = (name, executable) =>
    JSON.stringify({
      reason: "compiler-artifact",
      target: { name },
      executable,
    });
  assert.equal(
    builtExecutable(
      [
        row("other", "/wrong"),
        row("text-refinement-worker", "/current/worker"),
      ].join("\n"),
      "text-refinement-worker",
    ),
    "/current/worker",
  );
  assert.throws(
    () =>
      builtExecutable(
        row("text-refinement-worker", null),
        "text-refinement-worker",
      ),
    /exactly one/,
  );
  assert.throws(
    () =>
      builtExecutable(
        row("text-refinement-worker", "relative"),
        "text-refinement-worker",
      ),
    /absolute/,
  );
  assert.throws(
    () =>
      builtExecutable(
        [
          row("text-refinement-worker", "/one"),
          row("text-refinement-worker", "/two"),
        ].join("\n"),
        "text-refinement-worker",
      ),
    /exactly one/,
  );
});
test("bundle takes Metal kernels only from the MLX sys build selected by Cargo", () => {
  const messages = [
    {
      reason: "build-script-executed",
      package_id: "unrelated-native",
      out_dir: "/stale",
    },
    {
      reason: "build-script-executed",
      package_id: "exact-mlx-sys",
      out_dir: "/current",
    },
  ]
    .map(JSON.stringify)
    .join("\n");
  assert.equal(
    linkedMetallib(messages, "exact-mlx-sys"),
    "/current/lib/mlx.metallib",
  );
});
test("missing and ambiguous selected native builds cannot produce a bundle", () => {
  assert.throws(() => linkedMetallib("", "mlx"), /exactly one/);
  const row = (out) =>
    JSON.stringify({
      reason: "build-script-executed",
      package_id: "mlx",
      out_dir: out,
    });
  assert.throws(
    () => linkedMetallib([row("/one"), row("/two")].join("\n"), "mlx"),
    /exactly one/,
  );
  assert.throws(() => linkedMetallib(row("relative"), "mlx"), /absolute/);
});
