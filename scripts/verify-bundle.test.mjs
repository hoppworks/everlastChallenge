import { test } from "node:test";
import assert from "node:assert/strict";
import { microphoneConfigurationErrors } from "./verify-bundle.mjs";

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
