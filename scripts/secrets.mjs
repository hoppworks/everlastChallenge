import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Snapshot current working-tree source, including untracked non-ignored files.
// Index-only differences need an index check; committed revisions need a history scan.
// Tracked files remain in scope even if a later ignore rule matches them.
export function stageSource(root, destination) {
  const files = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    { cwd: root, encoding: "utf8" },
  )
    .split("\0")
    .filter(Boolean);
  for (const file of new Set(files)) {
    const relative = path.normalize(file);
    if (
      path.isAbsolute(relative) ||
      relative === ".." ||
      relative.startsWith(`..${path.sep}`)
    )
      throw new Error("Invalid repository path");
    const source = path.join(root, relative);
    let stat;
    try {
      stat = fs.lstatSync(source);
    } catch (error) {
      if (error.code === "ENOENT") continue; // A tracked deletion has no bytes to publish.
      throw error;
    }
    if (!stat.isFile())
      throw new Error(`Expected regular source file: ${file}`);
    const target = path.join(destination, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(source, target);
  }
}

function verifyScanner() {
  const version = execFileSync("gitleaks", ["version"], {
    encoding: "utf8",
  }).trim();
  if (version !== "8.30.1")
    throw new Error(`Expected pinned gitleaks 8.30.1, got ${version}`);
}

export function scanHistory(root) {
  verifyScanner();
  // Scan every commit reachable from this checkout, not unrelated local branches.
  execFileSync("git", ["rev-parse", "--verify", "HEAD"], {
    cwd: root,
    stdio: "pipe",
  });
  execFileSync(
    "gitleaks",
    [
      "git",
      "--no-banner",
      "--redact",
      "--ignore-gitleaks-allow",
      "--config",
      path.join(root, ".gitleaks.toml"),
      "--log-opts=HEAD",
      root,
    ],
    { stdio: "inherit" },
  );
}

export function scanSource(root) {
  verifyScanner();
  const directory = fs.mkdtempSync(
    path.join(os.tmpdir(), "everlast-publishable-source-"),
  );
  try {
    stageSource(root, directory);
    execFileSync(
      "gitleaks",
      [
        "dir",
        "--no-banner",
        "--redact",
        "--ignore-gitleaks-allow",
        "--config",
        path.join(root, ".gitleaks.toml"),
        directory,
      ],
      { stdio: "inherit" },
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length === 0) scanSource(process.cwd());
  else if (args.length === 1 && args[0] === "--history")
    scanHistory(process.cwd());
  else throw new Error("Usage: node scripts/secrets.mjs [--history]");
}
