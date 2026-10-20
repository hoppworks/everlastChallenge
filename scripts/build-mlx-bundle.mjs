import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { verifyBundle } from "./verify-bundle.mjs";
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export function linkedMetallib(messages, packageId) {
  const rows = messages
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const outputs = new Set(
    rows
      .filter(
        (row) =>
          row.reason === "build-script-executed" &&
          row.package_id === packageId,
      )
      .map((row) => row.out_dir),
  );
  if (outputs.size !== 1)
    throw new Error("Expected exactly one active MLX sys build output");
  const output = [...outputs][0];
  if (typeof output !== "string" || !path.isAbsolute(output))
    throw new Error("MLX build output must be absolute");
  return path.join(output, "lib/mlx.metallib");
}
function digest(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}
export function buildMlxBundle(root = ROOT) {
  const env = {
    ...Object.fromEntries(
      Object.entries(process.env).filter(([key]) => !key.startsWith("APPLE_")),
    ),
    MACOSX_DEPLOYMENT_TARGET: "27.0",
    APPLE_SIGNING_IDENTITY: "-",
  };
  const run = (program, args, options = {}) =>
    execFileSync(program, args, {
      cwd: root,
      env,
      stdio: "inherit",
      ...options,
    });
  run("npm", ["run", "build"]);
  const metadata = JSON.parse(
    run(
      "cargo",
      [
        "metadata",
        "--locked",
        "--format-version=1",
        "--features",
        "mlx-encoder-f16",
      ],
      {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "inherit"],
        maxBuffer: 32 * 1024 * 1024,
      },
    ),
  );
  const packages = metadata.packages.filter(
    (item) =>
      item.name === "mlxrs-sys" &&
      path.resolve(item.manifest_path) ===
        path.join(root, "vendor/mlxrs-sys/Cargo.toml"),
  );
  if (packages.length !== 1)
    throw new Error("Expected the pinned local MLX sys package");
  const messages = run(
    "cargo",
    [
      "build",
      "--locked",
      "--release",
      "--target",
      "aarch64-apple-darwin",
      "-p",
      "everlast-dictation",
      "--bin",
      "everlast-dictation",
      "--features",
      "mlx-encoder-f16",
      "--message-format=json-render-diagnostics",
    ],
    {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  const metallib = linkedMetallib(messages, packages[0].id);
  const stat = lstatSync(metallib);
  if (
    !stat.isFile() ||
    stat.isSymbolicLink() ||
    stat.size === 0 ||
    stat.size > 512 * 1024 * 1024
  )
    throw new Error("Invalid active Metal library");
  const kernelSha256 = digest(metallib);
  // Package first, then sign the actual destination. MetalLib signatures are
  // stored in xattrs, which Tauri's ordinary file copy does not preserve.
  const override = {
    bundle: { macOS: { files: { "MacOS/mlx.metallib": metallib } } },
  };
  run(path.join(root, "node_modules/.bin/tauri"), [
    "build",
    "--bundles",
    "app",
    "--target",
    "aarch64-apple-darwin",
    "--features",
    "mlx-encoder-f16",
    "--no-sign",
    "--config",
    JSON.stringify(override),
  ]);
  const config = JSON.parse(
    readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const bundle = path.join(
    metadata.target_directory,
    "aarch64-apple-darwin/release/bundle/macos",
    config.productName + ".app",
  );
  const bundled = path.join(bundle, "Contents/MacOS/mlx.metallib");
  if (
    !existsSync(bundled) ||
    digest(bundled) !== kernelSha256 ||
    digest(metallib) !== kernelSha256
  )
    throw new Error("Bundled kernels differ from the selected native build");
  run("/usr/bin/codesign", [
    "--force",
    "--sign",
    "-",
    "--timestamp=none",
    bundled,
  ]);
  run("/usr/bin/codesign", [
    "--force",
    "--sign",
    "-",
    "--timestamp=none",
    "--options",
    "runtime",
    "--entitlements",
    path.join(root, "src-tauri/Entitlements.plist"),
    bundle,
  ]);
  verifyBundle(bundle);
  const receipt = {
    bundle,
    identifier: config.identifier,
    sourceMetallib: metallib,
    metalLibraryBytes: stat.size,
    kernelSha256,
    metalLibrarySigning:
      "ad-hoc timestamp=none on the copied library, then on the app; xattrs must be preserved when relocating the bundle",
    override,
    installed: false,
    published: false,
  };
  const folder = path.join(root, "artifacts/implementation");
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    path.join(folder, `mlx-bundle-${Date.now()}.json`),
    JSON.stringify(receipt, null, 2) + "\n",
    { flag: "wx", mode: 0o600 },
  );
  return receipt;
}
if (process.argv[1] === fileURLToPath(import.meta.url))
  console.log(JSON.stringify(buildMlxBundle(), null, 2));
