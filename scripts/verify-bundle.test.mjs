import { test } from "node:test";
import assert from "node:assert/strict";
import {
  microphoneConfigurationErrors,
  verifyDistribution,
} from "./verify-bundle.mjs";

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
