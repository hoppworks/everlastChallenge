import { execFileSync } from "node:child_process";
import { readFileSync, lstatSync } from "node:fs";
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
export function runtimeFileErrors(bundle, executable) {
  if (
    typeof executable !== "string" ||
    !/^[\w.-]+$/.test(executable) ||
    executable === "." ||
    executable === ".."
  )
    return ["Invalid bundle executable name"];
  const errors = [];
  for (const name of [executable, "text-refinement-worker", "mlx.metallib"]) {
    try {
      const stat = lstatSync(path.join(bundle, "Contents/MacOS", name));
      if (
        !stat.isFile() ||
        stat.isSymbolicLink() ||
        stat.size === 0 ||
        stat.size > 512 * 1024 * 1024
      )
        errors.push(`${name} must be a nonempty regular file`);
      if (name !== "mlx.metallib" && (stat.mode & 0o111) === 0)
        errors.push(`${name} is not executable`);
    } catch {
      errors.push(`Missing runtime file: ${name}`);
    }
  }
  return errors;
}
export function externalLibraryErrors(output) {
  return output
    .split("\n")
    .slice(1)
    .map((line) => line.trim().split(" (", 1)[0])
    .filter(Boolean)
    .filter(
      (name) =>
        !name.startsWith("/System/Library/") && !name.startsWith("/usr/lib/"),
    )
    .map((name) => `Non-system runtime dependency: ${name}`);
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
  const fileErrors = runtimeFileErrors(bundle, info.CFBundleExecutable);
  if (fileErrors.length) throw new Error(fileErrors.join("; "));
  for (const name of [info.CFBundleExecutable, "text-refinement-worker"]) {
    const file = path.join(bundle, "Contents/MacOS", name);
    execFileSync("/usr/bin/codesign", ["--verify", "--strict", file], {
      stdio: "pipe",
    });
    const architectures = execFileSync("/usr/bin/lipo", ["-archs", file], {
      encoding: "utf8",
    }).trim();
    if (architectures !== "arm64")
      throw new Error(`Unexpected architecture for ${name}: ${architectures}`);
    const dependencies = externalLibraryErrors(
      execFileSync("/usr/bin/otool", ["-L", file], { encoding: "utf8" }),
    );
    if (dependencies.length) throw new Error(dependencies.join("; "));
  }
  execFileSync(
    "/usr/bin/codesign",
    ["--verify", "--strict", path.join(bundle, "Contents/MacOS/mlx.metallib")],
    { stdio: "pipe" },
  );
  const signed = execFileSync(
    "/usr/bin/codesign",
    ["-d", "--entitlements", "-", "--xml", bundle],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  const entitlements = signed.length === 0 ? {} : readPlist(signed);
  const errors = microphoneConfigurationErrors(info, entitlements);
  if (errors.length > 0) throw new Error(errors.join("; "));
  console.log(
    "Bundle runtime inventory, signatures, architecture, linkage and microphone declarations verified",
  );
}

/** Signature integrity alone is insufficient for a normal downloaded app. */
export function verifyDistribution(
  bundle,
  { run = execFileSync, verify = verifyBundle } = {},
) {
  verify(bundle);
  const options = { stdio: "pipe", timeout: 30_000 };
  try {
    run("/usr/sbin/spctl", ["--assess", "--type", "execute", bundle], options);
  } catch {
    throw new Error(
      "Gatekeeper rejected the app. A valid local signature is not distribution approval; check Developer ID signing and Apple notarization.",
    );
  }
  try {
    run("/usr/bin/xcrun", ["stapler", "validate", bundle], options);
  } catch {
    throw new Error("The app has no valid stapled Apple notarization ticket.");
  }
  console.log("Gatekeeper acceptance and stapled notarization ticket verified");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2);
    const distribution = args[0] === "--distribution";
    if (distribution) args.shift();
    if (args.length > 1 || args[0]?.startsWith("-"))
      throw new Error(
        "Usage: node scripts/verify-bundle.mjs [--distribution] [BUILT.app]",
      );
    const bundle =
      args[0] ??
      path.join(
        process.env.CARGO_TARGET_DIR || "target",
        "aarch64-apple-darwin/release/bundle/macos",
        JSON.parse(
          readFileSync(
            new URL("../src-tauri/tauri.conf.json", import.meta.url),
            "utf8",
          ),
        ).productName + ".app",
      );
    if (distribution) verifyDistribution(bundle);
    else verifyBundle(bundle);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
