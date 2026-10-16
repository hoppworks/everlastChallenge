import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

export function inspectSource(file, source) {
  const errors = [];
  if (file.startsWith("src/") && !file.startsWith("src/ipc/")) {
    const tree = ts.createSourceFile(
      file,
      source,
      ts.ScriptTarget.Latest,
      true,
      file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );
    const checkModule = (node) => {
      if (
        !node ||
        !ts.isStringLiteralLike(node) ||
        node.text.startsWith("@tauri-apps/")
      ) {
        errors.push(
          `${file}: native or computed imports must stay at the IPC boundary`,
        );
      }
    };
    const visit = (node) => {
      if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) {
        if (node.moduleSpecifier) checkModule(node.moduleSpecifier);
      } else if (
        ts.isCallExpression(node) &&
        (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
          (ts.isIdentifier(node.expression) &&
            node.expression.text === "require"))
      ) {
        checkModule(node.arguments[0]);
      } else if (
        ts.isImportEqualsDeclaration(node) &&
        ts.isExternalModuleReference(node.moduleReference)
      ) {
        checkModule(node.moduleReference.expression);
      }
      ts.forEachChild(node, visit);
    };
    visit(tree);
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

function filesIn(directory, root) {
  if (!fs.existsSync(path.join(root, directory))) return [];
  return fs
    .readdirSync(path.join(root, directory), { withFileTypes: true })
    .flatMap((entry) => {
      const file = path.posix.join(directory, entry.name);
      return entry.isDirectory() ? filesIn(file, root) : [file];
    });
}

export function inspectProject(root = ".") {
  return ["src", "src-tauri", "crates"]
    .flatMap((directory) => filesIn(directory, root))
    .filter((file) => /\.(?:rs|tsx?|mjs)$/u.test(file))
    .flatMap((file) =>
      inspectSource(file, fs.readFileSync(path.join(root, file), "utf8")),
    );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const errors = inspectProject();
  errors.forEach((error) => console.error(error));
  if (errors.length) process.exitCode = 1;
}
