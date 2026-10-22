/** Collect original license files from the resolved target graph, without claiming link closure. */
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { check } from "./tooling/audio-data.ts";
import { canonicalJson, publishIdentical, sha256 } from "./tooling/files.ts";
interface Package {
  id: string;
  name: string;
  version: string;
  source: string | null;
  manifest_path: string;
  license: string | null;
  license_file: string | null;
  repository: string | null;
}
interface Node {
  id: string;
  deps: Array<{ pkg: string; dep_kinds: Array<{ kind: string | null }> }>;
}
interface Metadata {
  packages: Package[];
  resolve: { nodes: Node[] };
}
interface Text {
  path: string;
  sha256: string;
  bytes: number;
  originalPath?: string;
  source?: unknown;
}
function collect(directory: string, licenseFile: string | null): Text[] {
  const names = readdirSync(directory, { withFileTypes: true })
    .filter(
      (e) =>
        e.isFile() &&
        /^(licen[cs]e|copying|notice|authors)([._-]|$)/i.test(e.name),
    )
    .map((e) => e.name);
  if (
    licenseFile &&
    !path.isAbsolute(licenseFile) &&
    !licenseFile.split(/[\\/]/).includes("..") &&
    existsSync(path.join(directory, licenseFile))
  )
    names.push(licenseFile);
  return [...new Set(names)].sort().map((name) => {
    const data = readFileSync(path.join(directory, name));
    check(data.length <= 2 * 1024 * 1024, "License text too large");
    const digest = sha256(data),
      output = `docs/licenses/third-party/texts/${digest}.txt`;
    publishIdentical(output, data);
    return {
      path: output,
      sha256: digest,
      bytes: data.length,
      originalPath: name,
    };
  });
}
export function generate() {
  const retained = new Map<string, Text[]>();
  for (const file of ["components.json", "mlx-components.json"]) {
    const prior = JSON.parse(
      readFileSync("docs/licenses/third-party/" + file, "utf8"),
    ) as {
      components: Array<{ name: string; version: string; texts: Text[] }>;
    };
    for (const item of prior.components)
      if (item.texts.length)
        retained.set(`${item.name}@${item.version}`, item.texts);
  }
  const supplement = JSON.parse(
    readFileSync(
      "docs/licenses/third-party/package-update-supplement.json",
      "utf8",
    ),
  ) as { texts: Array<Text & { components: string[] }> };
  for (const item of supplement.texts)
    for (const component of item.components)
      retained.set(component, [...(retained.get(component) ?? []), item]);
  const withRetained = (pkg: Package): Text[] => {
    const current = collect(path.dirname(pkg.manifest_path), pkg.license_file);
    const texts = current.length
      ? current
      : (retained.get(`${pkg.name}@${pkg.version}`) ?? []);
    for (const text of texts)
      check(
        sha256(readFileSync(text.path)) === text.sha256,
        "Retained license hash differs",
      );
    return texts;
  };
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--format-version",
        "1",
        "--filter-platform",
        "aarch64-apple-darwin",
        "--locked",
      ],
      { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
    ),
  ) as Metadata;
  const byId = new Map(metadata.packages.map((p) => [p.id, p])),
    nodes = new Map(metadata.resolve.nodes.map((n) => [n.id, n])),
    host = metadata.packages.find((p) => p.name === "everlast-dictation");
  check(host, "Host package missing");
  const visited = new Set<string>();
  const visit = (id: string) => {
    if (visited.has(id)) return;
    visited.add(id);
    for (const dep of nodes.get(id)?.deps ?? [])
      if (dep.dep_kinds.some((k) => k.kind === null)) visit(dep.pkg);
  };
  visit(host.id);
  const components = [...visited]
    .map((id) => byId.get(id)!)
    .filter((p) => p.source !== null)
    .sort(
      (a, b) =>
        a.name.localeCompare(b.name) || a.version.localeCompare(b.version),
    )
    .map((p) => ({
      name: p.name,
      version: p.version,
      source: p.source,
      license: p.license,
      repository: p.repository,
      manifestSha256: sha256(readFileSync(p.manifest_path)),
      inclusion:
        "resolved target normal dependency; optional/proc-macro portions conservative, not measured link closure",
      texts: withRetained(p),
    }));
  const lock = JSON.parse(readFileSync("package-lock.json", "utf8")) as {
    packages: Record<
      string,
      {
        name?: string;
        version?: string;
        dev?: boolean;
        license?: string;
        integrity?: string;
      }
    >;
  };
  const npm = Object.entries(lock.packages)
    .filter(
      ([key, p]) =>
        key &&
        (!p.dev ||
          ["node_modules/vite", "node_modules/rolldown"].includes(key)),
    )
    .map(([key, p]) => ({
      name: p.name ?? key.split("node_modules/").at(-1),
      version: p.version,
      license: p.license,
      integrity: p.integrity,
      inclusion: p.dev
        ? "build-injected runtime helper provider"
        : "production dependency",
      texts: collect(key, null),
    }));
  return {
    schemaVersion: 1,
    kind: "resolved dependency inventory after package migration",
    cargoLockSha256: sha256(readFileSync("Cargo.lock")),
    packageLockSha256: sha256(readFileSync("package-lock.json")),
    target: "aarch64-apple-darwin",
    finalLinkedClosureVerified: false,
    components,
    npm,
    supplements: [
      "license-completion.json",
      "objc2-predecessor-notices.json",
      "mlx-native-provenance.json",
      "package-update-supplement.json",
    ],
    textSourceLimits:
      "Collected texts include explicitly labelled upstream and standardized supplements; nonempty texts do not prove original upstream notices are complete. See license-completion.json.",
    componentsWithoutCollectedTexts: components
      .filter((c) => !c.texts.length)
      .map((c) => ({ name: c.name, version: c.version })),
  };
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    const output = process.argv[2];
    check(
      output && process.argv.length === 3,
      "Usage: update-license-inventory.ts NEW_OUTPUT.json",
    );
    publishIdentical(output, canonicalJson(generate()));
  } catch (error) {
    console.error(
      error instanceof Error ? error.message : "License collection failed",
    );
    process.exitCode = 2;
  }
}
