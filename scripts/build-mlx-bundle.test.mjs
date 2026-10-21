import { test } from "node:test";
import assert from "node:assert/strict";
import { linkedMetallib } from "./build-mlx-bundle.mjs";
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
