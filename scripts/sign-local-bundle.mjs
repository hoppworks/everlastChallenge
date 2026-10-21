import { execFileSync } from "node:child_process";
import {
  closeSync,
  existsSync,
  lstatSync,
  openSync,
  readFileSync,
  readSync,
  readdirSync,
  realpathSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { verifyBundle } from "./verify-bundle.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MAIN = "everlast-dictation";
function regular(file, limit) {
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink() || (limit && stat.size > limit))
    throw new Error(`Expected bounded regular file: ${file}`);
  return stat;
}
function identity(root) {
  const file = path.join(root, ".everlast-signing.json");
  regular(file, 4096);
  const config = JSON.parse(readFileSync(file, "utf8"));
  if (
    !config ||
    Array.isArray(config) ||
    Object.keys(config).length !== 1 ||
    typeof config.signingIdentity !== "string" ||
    !/^[a-fA-F0-9]{40}$/.test(config.signingIdentity)
  )
    throw new Error(
      "signingIdentity must be exactly a 40-hex certificate fingerprint, with no other fields",
    );
  return config.signingIdentity.toUpperCase();
}
function bundleConfiguration(root) {
  const file = path.join(root, "src-tauri/tauri.conf.json");
  if (!existsSync(file))
    return {
      productName: "Everlast Coding Challenge",
      identifier: "de.hoppworks.everlast-dictation",
    };
  regular(file, 65536);
  const config = JSON.parse(readFileSync(file, "utf8"));
  if (
    !/^[A-Za-z0-9][A-Za-z0-9 ._-]{0,79}$/.test(config.productName) ||
    !/^[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$/.test(config.identifier)
  )
    throw new Error("Invalid configured bundle identity");
  return config;
}
export function builtBundlePath(root = ROOT, env = process.env) {
  const target = path.resolve(root, env.CARGO_TARGET_DIR || "target");
  return path.join(
    target,
    `aarch64-apple-darwin/release/bundle/macos/${bundleConfiguration(root).productName}.app`,
  );
}
function validateBundle(bundle, run, root) {
  if (
    !bundle.endsWith(".app") ||
    !lstatSync(bundle).isDirectory() ||
    realpathSync(bundle) !== bundle
  )
    throw new Error(
      "Expected a real built .app directory without symlink components",
    );
  const infoPath = path.join(bundle, "Contents/Info.plist");
  regular(infoPath, 65536);
  const info = JSON.parse(
    run("/usr/bin/plutil", ["-convert", "json", "-o", "-", infoPath], {
      encoding: "utf8",
      maxBuffer: 65536,
    }),
  );
  if (
    info.CFBundleIdentifier !== bundleConfiguration(root).identifier ||
    info.CFBundleExecutable !== MAIN ||
    info.CFBundlePackageType !== "APPL"
  )
    throw new Error(
      "Bundle identity/executable is not the expected Everlast app",
    );
  const executable = path.join(bundle, "Contents/MacOS", MAIN);
  if (!(regular(executable).mode & 0o111))
    throw new Error("Main executable is not executable");
  let count = 0;
  function visit(directory) {
    for (const item of readdirSync(directory, { withFileTypes: true })) {
      if (++count > 4096) throw new Error("Bundle entry limit exceeded");
      const file = path.join(directory, item.name);
      const stat = lstatSync(file);
      if (stat.isSymbolicLink())
        throw new Error("Symlinked bundle contents are unsupported");
      if (stat.isDirectory()) {
        if (/\.(app|framework|xpc|appex)$/i.test(item.name))
          throw new Error("Nested executable bundle is unsupported");
        visit(file);
      } else if (stat.isFile()) {
        const header = Buffer.alloc(4);
        const fd = openSync(file, "r");
        try {
          readSync(fd, header, 0, 4, 0);
        } finally {
          closeSync(fd);
        }
        const magic = header.readUInt32BE(0);
        const macho = [
          0xfeedface, 0xfeedfacf, 0xcefaedfe, 0xcffaedfe, 0xcafebabe,
          0xbebafeca, 0xcafebabf, 0xbfbafeca,
        ].includes(magic);
        if (file !== executable && (stat.mode & 0o111 || macho))
          throw new Error("Additional executable payload is unsupported");
        if (file === executable && !macho)
          throw new Error("Main executable is not Mach-O");
      } else throw new Error("Nonregular bundle content is unsupported");
    }
  }
  visit(bundle);
  const entitlements = path.join(root, "src-tauri/Entitlements.plist");
  regular(entitlements, 65536);
  const rights = JSON.parse(
    run("/usr/bin/plutil", ["-convert", "json", "-o", "-", entitlements], {
      encoding: "utf8",
      maxBuffer: 65536,
    }),
  );
  if (rights["com.apple.security.device.audio-input"] !== true)
    throw new Error("Expected audio-input entitlement");
  return entitlements;
}
export function signLocalBundle({
  root = ROOT,
  bundle,
  env = process.env,
  run = execFileSync,
  verify = verifyBundle,
  build = false,
} = {}) {
  root = realpathSync(root);
  const fingerprint = identity(root); // Before build or any signing mutation.
  if (build) {
    if (bundle)
      throw new Error(
        "--build uses the configured target directory, not an explicit bundle",
      );
    const buildEnv = Object.fromEntries(
      Object.entries(env).filter(([key]) => !key.startsWith("APPLE_")),
    );
    Object.assign(buildEnv, {
      MACOSX_DEPLOYMENT_TARGET: "27.0",
      CARGO_TARGET_DIR: path.resolve(root, env.CARGO_TARGET_DIR || "target"),
      APPLE_SIGNING_IDENTITY: "-",
    });
    const mlxBuilder = path.join(root, "scripts/build-mlx-bundle.mjs");
    if (existsSync(mlxBuilder)) {
      regular(mlxBuilder, 65536);
      run(process.execPath, [mlxBuilder], {
        cwd: root,
        env: buildEnv,
        stdio: "inherit",
      });
    } else {
      run(
        path.join(root, "node_modules/.bin/tauri"),
        ["build", "--bundles", "app", "--target", "aarch64-apple-darwin"],
        { cwd: root, env: buildEnv, stdio: "inherit" },
      );
    }
  }
  bundle = path.resolve(root, bundle || builtBundlePath(root, env));
  const entitlements = validateBundle(bundle, run, root);
  verify(bundle);
  run(
    "/usr/bin/codesign",
    [
      "--force",
      "--sign",
      fingerprint,
      "--timestamp=none",
      "--options",
      "runtime",
      "--entitlements",
      entitlements,
      bundle,
    ],
    { stdio: "inherit" },
  );
  verify(bundle);
  return { bundle, signingIdentity: fingerprint };
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2);
    if (args.length > 1 || (args[0]?.startsWith("-") && args[0] !== "--build"))
      throw new Error(
        "Usage: node scripts/sign-local-bundle.mjs [BUILT.app|--build]",
      );
    const result = signLocalBundle({
      bundle: args[0] === "--build" ? undefined : args[0],
      build: args[0] === "--build",
    });
    console.log(`Locally signed and verified: ${result.bundle}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
