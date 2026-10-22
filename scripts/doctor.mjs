import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pinnedRust = readFileSync(
  path.join(root, "rust-toolchain.toml"),
  "utf8",
).match(/^channel\s*=\s*"([^"]+)"/m)?.[1];

function run(program, args) {
  const result = spawnSync(program, args, {
    encoding: "utf8",
    timeout: 15_000,
    maxBuffer: 16_384,
  });
  return result.status === 0 ? result.stdout.trim() : null;
}

/** Read-only host prerequisites; never installs tools or changes xcode-select. */
export function inspectPrerequisites({
  platform = process.platform,
  arch = process.arch,
  node = process.versions.node,
  execute = run,
  rust = pinnedRust,
} = {}) {
  const rows = [];
  const check = (name, ok, detail, help) =>
    rows.push({ name, ok, detail: detail ?? "nicht verfügbar", help });
  check(
    "Plattform",
    platform === "darwin" && arch === "arm64",
    `${platform}/${arch}`,
    "Der qualifizierte Build benötigt Apple Silicon und macOS 27.",
  );
  const [major, minor, patch] = node.split(".").map(Number);
  check(
    "Node.js",
    major >= 26 ||
      (major === 24 && minor >= 15) ||
      (major === 22 && (minor > 22 || (minor === 22 && patch >= 2))),
    node,
    "Node.js 22.22.2+, 24.15+ oder ab 26 installieren.",
  );
  const os = execute("sw_vers", ["-productVersion"]);
  check(
    "macOS",
    /^27\./.test(os ?? ""),
    os,
    "Auf einem qualifizierten macOS-27-Host bauen.",
  );
  const rustup = execute("rustup", ["--version"]);
  check(
    "rustup",
    Boolean(rustup),
    rustup,
    "rustup installieren; danach im Checkout rustup show ausführen.",
  );
  for (const command of ["rustc", "cargo"]) {
    const version = execute("rustup", ["run", rust, command, "--version"]);
    check(
      command,
      version?.split(" ")[1] === rust,
      version,
      `Gepinnte Toolchain ${rust} mit rustup show installieren.`,
    );
  }
  const cmake = execute("cmake", ["--version"]);
  const match = cmake?.match(/cmake version (\d+)\.(\d+)/);
  check(
    "CMake",
    Boolean(
      match &&
      (Number(match[1]) > 3 ||
        (Number(match[1]) === 3 && Number(match[2]) >= 25)),
    ),
    cmake?.split("\n")[0],
    "CMake >= 3.25 installieren und dem PATH hinzufügen (z. B. brew install cmake).",
  );
  const xcode = execute("xcodebuild", ["-version"]);
  check(
    "Xcode",
    /^Xcode 27\./.test(xcode ?? ""),
    xcode,
    "Volles Xcode 27 installieren/auswählen und dessen First Launch abschließen; geprüft: 27.1.",
  );
  const metal = execute("xcrun", ["--find", "metal"]);
  check(
    "Metal",
    Boolean(metal),
    metal,
    "Metal Toolchain in Xcode installieren, z. B. xcodebuild -downloadComponent MetalToolchain.",
  );
  return rows;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const rows = inspectPrerequisites();
  for (const row of rows) {
    console.log(`${row.ok ? "OK" : "FEHLT"} ${row.name}: ${row.detail}`);
    if (!row.ok) console.error(`  ${row.help}`);
  }
  process.exitCode = rows.every((row) => row.ok) ? 0 : 1;
}
