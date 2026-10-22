import { test } from "node:test";
import assert from "node:assert/strict";
import {
  microphoneConfigurationErrors,
  verifyDistribution,
  runtimeFileErrors,
  externalLibraryErrors,
} from "./verify-bundle.mjs";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  chmodSync,
  symlinkSync,
  rmSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";

test("runtime inventory rejects a missing or symlinked helper", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "bundle-inventory-"));
  try {
    const dir = path.join(root, "Contents/MacOS");
    mkdirSync(dir, { recursive: true });
    for (const name of ["app", "mlx.metallib"]) {
      writeFileSync(path.join(dir, name), "fixture");
      chmodSync(path.join(dir, name), 0o700);
    }
    assert.match(
      runtimeFileErrors(root, "app").join(),
      /text-refinement-worker/,
    );
    symlinkSync(
      path.join(dir, "app"),
      path.join(dir, "text-refinement-worker"),
    );
    assert.match(runtimeFileErrors(root, "app").join(), /regular file/);
    rmSync(path.join(dir, "text-refinement-worker"));
    writeFileSync(path.join(dir, "text-refinement-worker"), "fixture");
    chmodSync(path.join(dir, "text-refinement-worker"), 0o700);
    assert.deepEqual(runtimeFileErrors(root, "app"), []);
    assert.match(runtimeFileErrors(root, "../app").join(), /executable name/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
test("linked libraries cannot depend on a build machine or external rpath", () => {
  assert.deepEqual(
    externalLibraryErrors(
      "app:\n\t/usr/lib/libc++.1.dylib (compatibility version 1.0.0)\n\t/System/Library/Frameworks/Metal.framework/Metal (compatibility version 1.0.0)",
    ),
    [],
  );
  for (const lib of [
    "/opt/homebrew/lib/libmlx.dylib",
    "@rpath/libmlx.dylib",
    "/Users/build/lib.dylib",
  ])
    assert.match(
      externalLibraryErrors(
        `app:\n\t${lib} (compatibility version 1.0.0)`,
      ).join(),
      /Non-system/,
    );
});

test("usage text alone cannot authorize hardened-runtime audio input", () => {
  const info = { NSMicrophoneUsageDescription: "Local dictation" };
  assert.match(microphoneConfigurationErrors(info, {}).join(), /audio-input/);
  for (const value of [false, "true", 1])
    assert.match(
      microphoneConfigurationErrors(info, {
        "com.apple.security.device.audio-input": value,
      }).join(),
      /audio-input/,
    );
  assert.deepEqual(
    microphoneConfigurationErrors(info, {
      "com.apple.security.device.audio-input": true,
    }),
    [],
  );
});

test("a signed audio entitlement cannot replace the usage explanation", () => {
  const entitlements = { "com.apple.security.device.audio-input": true };
  for (const description of [undefined, "", " ", true])
    assert.match(
      microphoneConfigurationErrors(
        { NSMicrophoneUsageDescription: description },
        entitlements,
      ).join(),
      /usage description/,
    );
});

test("an intact local signature does not imply Gatekeeper acceptance", () => {
  assert.throws(
    () =>
      verifyDistribution("Example.app", {
        verify: () => {},
        run: () => {
          throw new Error("rejected");
        },
      }),
    /Gatekeeper rejected/,
  );
});

test("distribution verification rejects a missing stapled ticket", () => {
  assert.throws(
    () =>
      verifyDistribution("Example.app", {
        verify: () => {},
        run: (command) => {
          if (command === "/usr/bin/xcrun") throw new Error("no ticket");
        },
      }),
    /notarization ticket/,
  );
});

test("distribution verification requires signature, Gatekeeper and ticket checks", () => {
  const checks = [];
  verifyDistribution("Example.app", {
    verify: (bundle) => checks.push(["signature", bundle]),
    run: (command, args) => checks.push([command, ...args]),
  });
  assert.deepEqual(checks, [
    ["signature", "Example.app"],
    ["/usr/sbin/spctl", "--assess", "--type", "execute", "Example.app"],
    ["/usr/bin/xcrun", "stapler", "validate", "Example.app"],
  ]);
});
