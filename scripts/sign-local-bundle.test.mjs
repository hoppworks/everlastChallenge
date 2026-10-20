import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  rmSync,
  readFileSync,
  renameSync,
  realpathSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { signLocalBundle } from "./sign-local-bundle.mjs";

test("invalid machine identity is rejected before any command or bundle mutation", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "everlast-sign-test-"));
  try {
    writeFileSync(
      path.join(root, ".everlast-signing.json"),
      JSON.stringify({ signingIdentity: "-" }),
    );
    const marker = path.join(root, "bundle-marker");
    writeFileSync(marker, "unchanged");
    const calls = [];
    assert.throws(
      () => signLocalBundle({ root, run: (...args) => calls.push(args) }),
      /signingIdentity/,
    );
    assert.equal(calls.length, 0);
    assert.equal(readFileSync(marker, "utf8"), "unchanged");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function fixture() {
  const root = mkdtempSync(path.join(os.tmpdir(), "everlast-sign-test-"));
  const bundle = path.join(
    root,
    "target/aarch64-apple-darwin/release/bundle/macos/Everlast Dictation.app",
  );
  mkdirSync(path.join(bundle, "Contents/MacOS"), { recursive: true });
  mkdirSync(path.join(root, "src-tauri"));
  writeFileSync(
    path.join(root, ".everlast-signing.json"),
    JSON.stringify({
      signingIdentity: "73E69FEC7B988BA45831F5250114E400A920DD70",
    }),
  );
  writeFileSync(path.join(bundle, "Contents/Info.plist"), "test metadata");
  writeFileSync(
    path.join(root, "src-tauri/Entitlements.plist"),
    "test entitlements",
  );
  writeFileSync(
    path.join(bundle, "Contents/MacOS/everlast-dictation"),
    Buffer.from([0xcf, 0xfa, 0xed, 0xfe]),
    { mode: 0o755 },
  );
  return { root, bundle };
}
const info = {
  CFBundleIdentifier: "de.hoppworks.everlast-dictation",
  CFBundleExecutable: "everlast-dictation",
  CFBundlePackageType: "APPL",
};
function commands(calls, metadata = info) {
  return (program, args, options) => {
    calls.push({ program, args, options });
    if (program === "/usr/bin/plutil")
      return JSON.stringify(
        args.at(-1).endsWith("Info.plist")
          ? metadata
          : { "com.apple.security.device.audio-input": true },
      );
    return "";
  };
}

test("only the selected main app is signed; strict verification surrounds signing", () => {
  const { root } = fixture();
  try {
    const calls = [];
    const result = signLocalBundle({
      root,
      env: {},
      run: commands(calls),
      verify: (bundle) => calls.push({ verify: bundle }),
    });
    const signIndex = calls.findIndex(
      (call) => call.program === "/usr/bin/codesign",
    );
    assert.equal(calls[signIndex - 1].verify, result.bundle);
    assert.equal(calls[signIndex + 1].verify, result.bundle);
    assert.deepEqual(calls[signIndex].args, [
      "--force",
      "--sign",
      "73E69FEC7B988BA45831F5250114E400A920DD70",
      "--timestamp=none",
      "--options",
      "runtime",
      "--entitlements",
      path.join(
        result.bundle.split("/target/")[0],
        "src-tauri/Entitlements.plist",
      ),
      result.bundle,
    ]);
    assert.equal(
      calls.some((call) => call.args?.includes("--deep")),
      false,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("foreign identity and nested executable bundles are rejected before codesign", () => {
  for (const nested of [false, true]) {
    const { root, bundle } = fixture();
    try {
      if (nested) mkdirSync(path.join(bundle, "Contents/Helper.app"));
      const calls = [];
      assert.throws(
        () =>
          signLocalBundle({
            root,
            env: {},
            run: commands(
              calls,
              nested ? info : { ...info, CFBundleIdentifier: "other.app" },
            ),
            verify: () =>
              assert.fail("verification started before structural validation"),
          }),
        nested ? /Nested executable/ : /identity/,
      );
      assert.equal(
        calls.some((call) => call.program === "/usr/bin/codesign"),
        false,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }
});

test("app-only build normalizes target path and does not pass account credential variables", () => {
  const { root } = fixture();
  try {
    const calls = [];
    const result = signLocalBundle({
      root,
      build: true,
      env: {
        CARGO_TARGET_DIR: "target",
        APPLE_CERTIFICATE: "must-not-pass",
        PATH: process.env.PATH,
      },
      run: commands(calls),
      verify() {},
    });
    const build = calls[0];
    assert.deepEqual(build.args, [
      "build",
      "--bundles",
      "app",
      "--target",
      "aarch64-apple-darwin",
    ]);
    assert.equal(path.isAbsolute(build.options.env.CARGO_TARGET_DIR), true);
    assert.equal(build.options.env.APPLE_CERTIFICATE, undefined);
    assert.equal(build.options.env.APPLE_SIGNING_IDENTITY, "-");
    assert.equal(
      result.bundle.startsWith(build.options.env.CARGO_TARGET_DIR + path.sep),
      true,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("experimental bundle identity is read from this checkout before signing", () => {
  const { root, bundle } = fixture();
  try {
    const experimental = path.join(
      path.dirname(bundle),
      "Everlast Dictation MLX.app",
    );
    renameSync(bundle, experimental);
    writeFileSync(
      path.join(root, "src-tauri/tauri.conf.json"),
      JSON.stringify({
        productName: "Everlast Dictation MLX",
        identifier: "de.hoppworks.everlast-dictation-mlx",
      }),
    );
    const calls = [];
    const result = signLocalBundle({
      root,
      env: {},
      run: commands(calls, {
        ...info,
        CFBundleIdentifier: "de.hoppworks.everlast-dictation-mlx",
      }),
      verify() {},
    });
    assert.equal(result.bundle, realpathSync(experimental));
    assert.equal(
      calls.filter((call) => call.program === "/usr/bin/codesign").length,
      1,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("local MLX release builds through the kernel packaging helper", () => {
  const { root } = fixture();
  try {
    mkdirSync(path.join(root, "scripts"));
    const helper = path.join(root, "scripts/build-mlx-bundle.mjs");
    writeFileSync(helper, "// MLX packaging entry point\n");
    const calls = [];
    signLocalBundle({
      root,
      build: true,
      env: { PATH: process.env.PATH, APPLE_CERTIFICATE: "must-not-pass" },
      run: commands(calls),
      verify() {},
    });
    assert.equal(calls[0].program, process.execPath);
    assert.deepEqual(calls[0].args, [realpathSync(helper)]);
    assert.equal(calls[0].options.cwd, realpathSync(root));
    assert.equal(calls[0].options.env.APPLE_CERTIFICATE, undefined);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
