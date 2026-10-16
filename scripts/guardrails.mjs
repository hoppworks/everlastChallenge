import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function inspectSource(file, source) {
  const errors = [];
  if (
    file.startsWith("src/") &&
    !file.startsWith("src/ipc/") &&
    /['"]@tauri-apps\//u.test(source)
  ) {
    errors.push(`${file}: native imports must stay at the IPC boundary`);
  }
  if (
    file.endsWith(".rs") &&
    !file.startsWith("crates/macos-integration/") &&
    /\bunsafe\s*(?:\{|fn\b|impl\b|extern\b|trait\b)/u.test(source)
  ) {
    errors.push(`${file}: unsafe outside the native crate`);
  }
  if (
    file.endsWith(".rs") &&
    /(?:\.\s*(?:unwrap|expect)\s*\(|\bpanic\s*!)/u.test(source)
  ) {
    errors.push(`${file}: production panic shortcut; return a typed error`);
  }
  if (/\b(?:v8|c8|istanbul)\s+ignore\b/u.test(source)) {
    errors.push(
      `${file}: inline coverage ignore forbidden; use reasoned versioned configuration`,
    );
  }
  for (const line of source.split("\n")) {
    if (/\boxlint-disable/u.test(line) && !/--\s*\S.+/u.test(line)) {
      errors.push(`${file}: lint exception needs a reason after --`);
    }
  }
  for (const attribute of source.matchAll(
    /#!?\[\s*(?:allow|expect)\s*\(([\s\S]*?)\)\s*\]/gu,
  )) {
    if (!/reason\s*=\s*"[^"\n]+"/u.test(attribute[1])) {
      errors.push(`${file}: Rust lint exception needs a reason`);
    }
  }
  return errors;
}

function filesIn(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.posix.join(directory, entry.name);
    return entry.isDirectory() ? filesIn(file) : [file];
  });
}

export function inspectProject() {
  return ["src", "src-tauri/src", "crates"]
    .flatMap(filesIn)
    .filter((file) => /\.(?:rs|tsx?|mjs)$/u.test(file))
    .flatMap((file) => inspectSource(file, fs.readFileSync(file, "utf8")));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const errors = inspectProject();
  errors.forEach((error) => console.error(error));
  if (errors.length) process.exitCode = 1;
}
