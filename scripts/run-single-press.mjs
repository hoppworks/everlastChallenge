import { execFileSync } from "node:child_process";
import {
  constants,
  closeSync,
  fstatSync,
  lstatSync,
  openSync,
  readSync,
  realpathSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import path from "node:path";
import {
  CONTRACT_ID,
  preflightSinglePress,
  createFileAdapters,
  readPrivateJson,
  runSinglePress,
} from "./single-press-runner.mjs";
import {
  createPlaybackAdapter,
  validatePublicAudio,
  PUBLIC_AUDIO_SHA256,
} from "./single-press-playback.mjs";
import { createWatchdogAdapter } from "./single-press-watchdog-adapter.mjs";

export const INSTALLED_BUNDLE = "/Applications/Everlast Dictation.app";
export const INSTALLED_EXECUTABLE = path.join(
  INSTALLED_BUNDLE,
  "Contents/MacOS/everlast-dictation",
);
const hashValid = (value) =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
function reject(code) {
  throw Object.assign(new Error(code), { code });
}
function exact(value, keys) {
  return (
    value &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.hasOwn(value, key))
  );
}
function canonical(filename) {
  if (
    typeof filename !== "string" ||
    !path.isAbsolute(filename) ||
    path.normalize(filename) !== filename ||
    realpathSync(path.dirname(filename)) !== path.dirname(filename)
  )
    reject("entry-path-invalid");
}
function hashedFile(filename, maximumBytes, collect = false) {
  canonical(filename);
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const before = fstatSync(fd);
    if (
      !before.isFile() ||
      before.uid !== process.getuid() ||
      before.size > maximumBytes
    )
      reject("entry-file-invalid");
    const hash = createHash("sha256");
    const bytes = Buffer.alloc(65536);
    const chunks = [];
    let total = 0;
    let count;
    while ((count = readSync(fd, bytes, 0, bytes.length, null)) !== 0) {
      total += count;
      if (total > maximumBytes) reject("entry-file-oversized");
      const portion = bytes.subarray(0, count);
      hash.update(portion);
      if (collect) chunks.push(Buffer.from(portion));
    }
    const after = fstatSync(fd);
    if (
      total !== before.size ||
      after.size !== before.size ||
      after.mtimeMs !== before.mtimeMs
    )
      reject("entry-file-changed");
    return {
      sha256: hash.digest("hex"),
      bytes: collect ? Buffer.concat(chunks) : null,
      identity: {
        dev: after.dev,
        ino: after.ino,
        size: after.size,
        mtimeMs: after.mtimeMs,
      },
    };
  } finally {
    closeSync(fd);
  }
}
function privateDocument(filename) {
  canonical(filename);
  const parent = lstatSync(path.dirname(filename));
  const info = lstatSync(filename);
  if (
    !parent.isDirectory() ||
    parent.uid !== process.getuid() ||
    (parent.mode & 0o777) !== 0o700 ||
    !info.isFile() ||
    info.uid !== process.getuid() ||
    (info.mode & 0o777) !== 0o600
  )
    reject("entry-private-document-invalid");
  const file = hashedFile(filename, 65536, true);
  let value;
  try {
    value = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(file.bytes),
    );
  } catch {
    reject("entry-json-invalid");
  }
  return { value, sha256: file.sha256 };
}
function validateConfig(config) {
  if (
    !exact(config, [
      "schemaVersion",
      "contractId",
      "frozen",
      "productReport",
      "recipientReport",
      "recipientDirectory",
      "reportDirectory",
      "bindings",
      "watchdog",
      "preparationProof",
    ]) ||
    config.schemaVersion !== 1 ||
    config.contractId !== CONTRACT_ID ||
    !exact(config.bindings, ["artifact", "model", "fixture"]) ||
    !exact(config.watchdog, [
      "executable",
      "executableSha256",
      "workDirectory",
      "newReportDirectory",
    ]) ||
    !config.frozen ||
    config.frozen.contractId !== CONTRACT_ID
  )
    reject("entry-config-invalid");
  for (const value of [
    config.productReport,
    config.recipientReport,
    config.recipientDirectory,
    config.reportDirectory,
    config.preparationProof,
    ...Object.values(config.bindings),
    config.watchdog.executable,
    config.watchdog.workDirectory,
    config.watchdog.newReportDirectory,
  ])
    if (
      typeof value !== "string" ||
      !path.isAbsolute(value) ||
      path.normalize(value) !== value
    )
      reject("entry-path-invalid");
  if (
    config.bindings.artifact !== INSTALLED_EXECUTABLE ||
    path.dirname(config.recipientReport) !== config.recipientDirectory ||
    config.frozen.fixtureSha256 !== PUBLIC_AUDIO_SHA256
  )
    reject("entry-installed-binding-invalid");
}
function verifyPreparation(config, now) {
  const file = privateDocument(config.preparationProof);
  const proof = file.value;
  const frozen = config.frozen;
  if (
    !exact(proof, [
      "schemaVersion",
      "kind",
      "contractId",
      "runId",
      "productPid",
      "recipientPid",
      "normalBuild",
      "installed",
      "mainWindowClosed",
      "recipientPrepared",
      "observedUnixMillis",
      "normalBuildProof",
    ]) ||
    proof.schemaVersion !== 1 ||
    proof.kind !== "single-press-ready-preparation" ||
    proof.contractId !== CONTRACT_ID ||
    proof.runId !== frozen.runId ||
    proof.productPid !== frozen.product.processId ||
    proof.recipientPid !== frozen.recipient.processId ||
    ![
      proof.normalBuild,
      proof.installed,
      proof.mainWindowClosed,
      proof.recipientPrepared,
    ].every((value) => value === true) ||
    !Number.isSafeInteger(proof.observedUnixMillis) ||
    proof.observedUnixMillis > now + 100 ||
    now - proof.observedUnixMillis > 30000
  )
    reject("preparation-not-ready");
  const build = proof.normalBuildProof;
  if (
    !exact(build, [
      "artifactSha256",
      "buildLogPath",
      "buildLogSha256",
      "cargoFingerprintPath",
      "cargoFingerprintSha256",
    ]) ||
    build.artifactSha256 !== frozen.artifactSha256 ||
    ![
      build.artifactSha256,
      build.buildLogSha256,
      build.cargoFingerprintSha256,
    ].every(hashValid)
  )
    reject("normal-build-proof-invalid");
  const log = hashedFile(build.buildLogPath, 8 * 1024 * 1024);
  const fingerprint = hashedFile(build.cargoFingerprintPath, 65536, true);
  if (
    log.sha256 !== build.buildLogSha256 ||
    log.identity.size === 0 ||
    fingerprint.sha256 !== build.cargoFingerprintSha256
  )
    reject("normal-build-evidence-mismatch");
  let features;
  try {
    const value = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(fingerprint.bytes),
    );
    if (typeof value.features !== "string")
      reject("normal-build-features-invalid");
    features = JSON.parse(value.features);
  } catch {
    reject("normal-build-features-invalid");
  }
  if (
    !Array.isArray(features) ||
    features.length !== 1 ||
    features[0] !== "default"
  )
    reject("normal-build-features-invalid");
  return {
    preparationProofSha256: file.sha256,
    buildLogSha256: log.sha256,
    cargoFingerprintSha256: fingerprint.sha256,
    artifactSha256: build.artifactSha256,
  };
}

/** Read-only local signature/plist inspection. No launch, activation, Gatekeeper override or TCC action. */
export function verifyInstalledBundle(config, { command = execFileSync } = {}) {
  if (config.bindings.artifact !== INSTALLED_EXECUTABLE)
    reject("entry-installed-binding-invalid");
  const options = {
    encoding: "utf8",
    stdio: ["pipe", "pipe", "pipe"],
    timeout: 10000,
    maxBuffer: 65536,
  };
  command(
    "/usr/bin/codesign",
    ["--verify", "--strict", "--deep", INSTALLED_BUNDLE],
    options,
  );
  const info = JSON.parse(
    command(
      "/usr/bin/plutil",
      [
        "-convert",
        "json",
        "-o",
        "-",
        path.join(INSTALLED_BUNDLE, "Contents/Info.plist"),
      ],
      options,
    ),
  );
  if (
    info.CFBundleIdentifier !== "de.hoppworks.everlast-dictation" ||
    info.CFBundleExecutable !== "everlast-dictation" ||
    Object.hasOwn(info, "LSEnvironment") ||
    typeof info.NSMicrophoneUsageDescription !== "string" ||
    info.NSMicrophoneUsageDescription.trim().length === 0
  )
    reject("installed-bundle-metadata-invalid");
  const signed = command(
    "/usr/bin/codesign",
    ["-d", "--entitlements", "-", "--xml", INSTALLED_BUNDLE],
    options,
  );
  const entitlements = JSON.parse(
    command("/usr/bin/plutil", ["-convert", "json", "-o", "-", "-"], {
      ...options,
      input: signed,
    }),
  );
  if (entitlements["com.apple.security.device.audio-input"] !== true)
    reject("installed-bundle-microphone-invalid");
  return true;
}
function verifyBindings(config) {
  const bound = {};
  for (const key of ["artifact", "model", "fixture"]) {
    bound[key] = hashedFile(config.bindings[key], 1073741824);
    if (bound[key].sha256 !== config.frozen[`${key}Sha256`])
      reject("entry-binding-hash-mismatch");
  }
  return {
    verifyFrozen: async () => {
      try {
        process.kill(config.frozen.product.processId, 0);
        process.kill(config.frozen.recipient.processId, 0);
        return ["artifact", "model", "fixture"].every((key) => {
          canonical(config.bindings[key]);
          const info = lstatSync(config.bindings[key]);
          const old = bound[key].identity;
          return (
            info.isFile() &&
            info.uid === process.getuid() &&
            info.dev === old.dev &&
            info.ino === old.ino &&
            info.size === old.size &&
            info.mtimeMs === old.mtimeMs
          );
        });
      } catch {
        return false;
      }
    },
  };
}

export async function executeSinglePressEntry(args, dependencies = {}) {
  if (
    args.length !== 2 ||
    !["--preflight", "--run"].includes(args[0]) ||
    !path.isAbsolute(args[1])
  )
    reject("single-press-arguments-invalid");
  const config = privateDocument(args[1]).value;
  validateConfig(config);
  const now = dependencies.now ?? Date.now;
  const evidence = verifyPreparation(config, now());
  if (
    (await (dependencies.verifyBundle ?? verifyInstalledBundle)(config)) !==
    true
  )
    reject("installed-bundle-unverified");
  const binding = await (dependencies.verifyBindings ?? verifyBindings)(config);
  const audioProof = (dependencies.verifyAudio ?? validatePublicAudio)(
    config.bindings.fixture,
  );
  if (
    audioProof?.sha256 !== PUBLIC_AUDIO_SHA256 ||
    audioProof.frames !== 4800000 ||
    audioProof.durationMillis !== 300000
  )
    reject("entry-audio-unverified");
  const watchdog = (dependencies.makeWatchdog ?? createWatchdogAdapter)({
    ...config.watchdog,
    productReport: config.productReport,
  });
  const observe = async () => ({
    product: readPrivateJson(config.productReport),
    recipient: readPrivateJson(config.recipientReport),
  });
  const preflight = {
    frozen: config.frozen,
    observe,
    verifyFrozen: binding.verifyFrozen,
    watchdog,
  };
  if (
    (await (dependencies.preflight ?? preflightSinglePress)(preflight))
      ?.ready !== true
  )
    reject("entry-readiness-unconfirmed");
  // A slow preflight cannot authorize an old external UI-preparation assertion.
  verifyPreparation(config, now());
  if (args[0] === "--preflight")
    return {
      mode: "preflight",
      ready: true,
      contractId: CONTRACT_ID,
      runId: config.frozen.runId,
      gestureConsumed: false,
      normalBuildEvidence: evidence,
    };
  const audio = (dependencies.makePlayback ?? createPlaybackAdapter)({
    fixturePath: config.bindings.fixture,
  });
  const adapters = (dependencies.makeFileAdapters ?? createFileAdapters)({
    ...config,
    audio,
    watchdog,
    cleanup: (frozen) => watchdog.cleanup(frozen),
  });
  const originalWrite = adapters.writeReport;
  adapters.writeReport = (report) =>
    originalWrite({ ...report, normalBuildEvidence: evidence });
  const result = await (dependencies.run ?? runSinglePress)(adapters);
  return { ...result, normalBuildEvidence: evidence };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  executeSinglePressEntry(process.argv.slice(2))
    .then((result) => {
      process.stdout.write(JSON.stringify(result) + "\n");
      process.exitCode = result.ready || result.state === "passed" ? 0 : 1;
    })
    .catch((error) => {
      const code =
        typeof error?.code === "string" && /^[a-z-]+$/.test(error.code)
          ? error.code
          : "single-press-entry-failed";
      process.stderr.write(
        JSON.stringify({ ready: false, error: code }) + "\n",
      );
      process.exitCode = 1;
    });
}
