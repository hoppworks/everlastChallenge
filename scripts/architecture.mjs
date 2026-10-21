import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import ts from "typescript";

export function inspectFrontend(sources) {
  const errors = [];
  const edges = new Map();
  for (const [file, source] of Object.entries(sources)) {
    const tree = ts.createSourceFile(
      file,
      source,
      ts.ScriptTarget.Latest,
      true,
    );
    const links = [];
    const check = (node) => {
      if (!node || !ts.isStringLiteralLike(node)) {
        errors.push(`${file}: computed import is forbidden`);
        return;
      }
      const name = node.text;
      if (!name.startsWith(".")) {
        if (
          !["react", "react-dom/client"].includes(name) &&
          !(file.startsWith("src/ipc/") && name.startsWith("@tauri-apps/"))
        )
          errors.push(
            `${file}: external import ${name} is not permitted in this layer`,
          );
        return;
      }
      const base = path.posix.normalize(
        path.posix.join(path.posix.dirname(file), name),
      );
      if (!base.startsWith("src/"))
        errors.push(`${file}: import escapes frontend source`);
      if (name.endsWith(".css")) return;
      const target = [
        base,
        `${base}.ts`,
        `${base}.tsx`,
        `${base}/index.ts`,
        `${base}/index.tsx`,
      ].find((p) => p in sources);
      if (!target) {
        errors.push(`${file}: unresolved local module ${name}`);
        return;
      }
      if (file.startsWith("src/ipc/") && !target.startsWith("src/ipc/"))
        errors.push(`${file}: IPC must not depend on UI`);
      if (
        file.startsWith("src/ipc/generated/") &&
        !target.startsWith("src/ipc/generated/")
      )
        errors.push(`${file}: generated contracts must be self-contained`);
      if (
        file.startsWith("src/features/") &&
        !target.startsWith("src/features/") &&
        !target.startsWith("src/ipc/")
      )
        errors.push(`${file}: feature must not depend on app composition`);
      links.push(target);
    };
    const visit = (node) => {
      if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) {
        if (node.moduleSpecifier) check(node.moduleSpecifier);
      } else if (
        ts.isCallExpression(node) &&
        (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
          (ts.isIdentifier(node.expression) &&
            node.expression.text === "require"))
      )
        check(node.arguments[0]);
      else if (
        ts.isImportEqualsDeclaration(node) &&
        ts.isExternalModuleReference(node.moduleReference)
      )
        check(node.moduleReference.expression);
      ts.forEachChild(node, visit);
    };
    visit(tree);
    edges.set(file, links);
  }
  const visiting = new Set(),
    done = new Set();
  const walk = (file) => {
    if (visiting.has(file)) {
      errors.push(`${file}: circular module dependency`);
      return;
    }
    if (done.has(file)) return;
    visiting.add(file);
    for (const target of edges.get(file) ?? []) walk(target);
    visiting.delete(file);
    done.add(file);
  };
  for (const file of edges.keys()) walk(file);
  return errors;
}

const permittedCore = new Set(["serde", "thiserror", "rubato", "ts-rs"]);
export function inspectWorkspace(metadata) {
  const errors = [];
  const members = new Set(metadata.workspace_members);
  const packages = metadata.packages.filter((pkg) => members.has(pkg.id));
  const byName = new Map(packages.map((pkg) => [pkg.name, pkg]));
  const visited = new Set();
  const visit = (pkg, feature) => {
    const key = `${pkg.name}/${feature}`;
    if (visited.has(key)) return;
    visited.add(key);
    const split = feature.indexOf("/");
    const leaf = split < 0 ? feature : feature.slice(split + 1);
    if (["measurement", "test-control", "isolated"].includes(leaf))
      errors.push(
        `production feature graph exposes experimental control: ${key}`,
      );
    if (split >= 0) {
      const alias = feature.slice(0, split).replace(/\?$/, "");
      const dep = pkg.dependencies.find(
        (dep) => (dep.rename ?? dep.name) === alias && dep.kind !== "dev",
      );
      const target = dep && byName.get(dep.name);
      if (target) visit(target, leaf);
    }
    for (const next of pkg.features?.[feature] ?? []) visit(pkg, next);
  };
  for (const pkg of packages) {
    for (const dependency of pkg.dependencies.filter(
      (dep) => dep.kind !== "dev",
    )) {
      if (pkg.name === "dictation-core" && !permittedCore.has(dependency.name))
        errors.push(`dictation-core: forbidden dependency ${dependency.name}`);
      if (
        pkg.name === "macos-integration" &&
        [
          "dictation-core",
          "everlast-dictation",
          "tauri",
          "cpal",
          "whisper-rs",
          "mlxrs",
        ].includes(dependency.name)
      )
        errors.push(
          `macos-integration: forbidden upward or inference dependency ${dependency.name}`,
        );
    }
    visit(pkg, "default");
    for (const dependency of pkg.dependencies.filter(
      (dep) => dep.kind !== "dev",
    )) {
      for (const feature of dependency.features ?? []) {
        if (["measurement", "test-control", "isolated"].includes(feature))
          errors.push(
            `${pkg.name}: dependency ${dependency.name} enables experimental control ${feature}`,
          );
        const target = byName.get(dependency.name);
        if (target) visit(target, feature);
      }
    }
  }
  return errors;
}
export function inspectNativeInputs(actual, upstream, synchronized) {
  const errors = [];
  const watched = new Set(synchronized);
  for (const [file, digest] of Object.entries(upstream)) {
    if (!(file in actual))
      errors.push(
        `native source removed without a supported sync operation: ${file}`,
      );
    else if (actual[file] !== digest && !watched.has(file))
      errors.push(`modified native source is not synchronized: ${file}`);
  }
  for (const file of Object.keys(actual))
    if (!(file in upstream) && !watched.has(file))
      errors.push(`new native source is not synchronized: ${file}`);
  for (const file of watched)
    if (!(file in actual) || file.includes("..") || path.isAbsolute(file))
      errors.push(`invalid synchronized native input: ${file}`);
  return errors;
}
function files(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink())
      throw new Error(`Unexpected source symlink: ${file}`);
    return entry.isDirectory() ? files(file) : [file];
  });
}
export function inspectArchitecture() {
  const sources = Object.fromEntries(
    files("src")
      .filter((f) => /\.(ts|tsx)$/.test(f) && !/\.test\./.test(f))
      .map((f) => [f, fs.readFileSync(f, "utf8")]),
  );
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      ["metadata", "--no-deps", "--format-version", "1", "--locked"],
      { encoding: "utf8" },
    ),
  );
  const prefix = "vendor/whisper-rs-sys/whisper.cpp/";
  const actual = Object.fromEntries(
    files(prefix.slice(0, -1)).map((f) => [
      f.slice(prefix.length),
      createHash("sha256").update(fs.readFileSync(f)).digest("hex"),
    ]),
  );
  const upstream = JSON.parse(
    fs.readFileSync("config/whisper-upstream-hashes.json", "utf8"),
  ).files;
  const synchronized = fs
    .readFileSync("vendor/whisper-rs-sys/native-inputs.txt", "utf8")
    .trim()
    .split(/\r?\n/);
  return [
    ...inspectFrontend(sources),
    ...inspectWorkspace(metadata),
    ...inspectNativeInputs(actual, upstream, synchronized),
  ];
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const errors = inspectArchitecture();
  errors.forEach((error) => console.error(error));
  if (errors.length) process.exitCode = 1;
}
