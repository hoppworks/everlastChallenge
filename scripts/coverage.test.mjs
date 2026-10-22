import { test } from "node:test";
import assert from "node:assert/strict";
import {
  checkThresholds,
  checkFileThresholds,
  resolveRustCoverageTools,
} from "./coverage.mjs";

test("file coverage cannot hide a missing or weak boundary behind another file", () => {
  const minimum = { "owner.rs": 80, "protocol.rs": 90 };
  const owner = { lines: 99, count: 100, covered: 99 };
  assert.match(
    checkFileThresholds({ "owner.rs": owner }, minimum).join(),
    /protocol.rs.*missing/,
  );
  assert.match(
    checkFileThresholds(
      {
        "owner.rs": owner,
        "protocol.rs": { lines: 89, count: 100, covered: 89 },
      },
      minimum,
    ).join(),
    /protocol.rs.*below/,
  );
  assert.deepEqual(
    checkFileThresholds(
      {
        "owner.rs": owner,
        "protocol.rs": { lines: 90, count: 100, covered: 90 },
      },
      minimum,
    ),
    [],
  );
});

test("file coverage requires a nonempty positive policy and executable measurements", () => {
  assert.match(checkFileThresholds({}, {}).join(), /empty/);
  for (const minimum of [0, -1, 101, NaN])
    assert.match(
      checkFileThresholds({}, { "owner.rs": minimum }).join(),
      /invalid/,
    );
  assert.match(
    checkFileThresholds(
      { "owner.rs": { lines: 100, count: 0, covered: 0 } },
      { "owner.rs": 80 },
    ).join(),
    /executable/,
  );
});

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
