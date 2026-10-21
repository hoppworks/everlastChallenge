import { test } from "node:test";
import assert from "node:assert/strict";
import { checkThresholds, resolveRustCoverageTools } from "./coverage.mjs";

test("coverage fails below a threshold or for missing metrics", () => {
  assert.deepEqual(checkThresholds({ lines: 90 }, { lines: 90 }), []);
  assert.match(
    checkThresholds({ lines: 89.99 }, { lines: 90 }).join(),
    /below/,
  );
  assert.match(checkThresholds({}, { lines: 90 }).join(), /missing/);
});

test("coverage uses LLVM tools from the selected compiler, including spaced paths", () => {
  const compiler = (command, args) => {
    assert.equal(command, "rustc");
    if (args.join(" ") === "--print sysroot") return "/compiler with spaces\n";
    if (args.join(" ") === "-vV")
      return "rustc 1.99.1\nhost: aarch64-apple-darwin\nLLVM version: 22.1.0\n";
    assert.fail("Unexpected compiler query");
  };
  assert.deepEqual(resolveRustCoverageTools(compiler), {
    profdata:
      "/compiler with spaces/lib/rustlib/aarch64-apple-darwin/bin/llvm-profdata",
    cov: "/compiler with spaces/lib/rustlib/aarch64-apple-darwin/bin/llvm-cov",
  });
});

test("coverage refuses an unknown compiler host instead of using Xcode tools", () => {
  assert.throws(
    () => resolveRustCoverageTools(() => "unknown compiler\n"),
    /compiler host/,
  );
});
