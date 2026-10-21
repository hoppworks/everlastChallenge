import { execFileSync } from "node:child_process";
execFileSync("git", ["config", "--local", "core.hooksPath", ".githooks"], {
  stdio: "inherit",
});
console.log("Installed local noninteractive pre-commit checks (.githooks).");
