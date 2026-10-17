import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function microphoneConfigurationErrors(info, entitlements) {
  const errors = [];
  if (
    typeof info.NSMicrophoneUsageDescription !== "string" ||
    info.NSMicrophoneUsageDescription.trim().length === 0
  )
    errors.push(
      "Missing microphone usage description in the bundled Info.plist",
    );
  if (entitlements["com.apple.security.device.audio-input"] !== true)
    errors.push(
      "Missing signed audio-input entitlement for the hardened runtime",
    );
  return errors;
}

function readPlist(input) {
  return JSON.parse(
    execFileSync("/usr/bin/plutil", ["-convert", "json", "-o", "-", "-"], {
      input,
      encoding: "utf8",
    }),
  );
}

export function verifyBundle(bundle) {
  execFileSync(
    "/usr/bin/codesign",
    ["--verify", "--deep", "--strict", bundle],
    {
      stdio: "pipe",
    },
  );
  const info = readPlist(
    readFileSync(path.join(bundle, "Contents", "Info.plist")),
  );
  const signed = execFileSync(
    "/usr/bin/codesign",
    ["-d", "--entitlements", "-", "--xml", bundle],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  const entitlements = signed.length === 0 ? {} : readPlist(signed);
  const errors = microphoneConfigurationErrors(info, entitlements);
  if (errors.length > 0) throw new Error(errors.join("; "));
  console.log("Bundle signature and microphone declarations verified");
}

if (process.argv[1] === fileURLToPath(import.meta.url))
  verifyBundle(
    process.argv[2] ??
      "target/aarch64-apple-darwin/release/bundle/macos/Everlast Dictation.app",
  );
