import { execFileSync } from "node:child_process";

const version = execFileSync("gitleaks", ["version"], {
  encoding: "utf8",
}).trim();
if (version !== "8.30.1")
  throw new Error(`Expected pinned gitleaks 8.30.1, got ${version}`);
execFileSync(
  "gitleaks",
  [
    "dir",
    "--no-banner",
    "--redact",
    "--ignore-gitleaks-allow",
    "--config",
    ".gitleaks.toml",
    ".",
  ],
  { stdio: "inherit" },
);
