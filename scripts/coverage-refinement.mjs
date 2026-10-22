import fs from "node:fs";
import { measureRust, checkFileThresholds } from "./coverage.mjs";

const policy = JSON.parse(fs.readFileSync("config/coverage.json", "utf8"));
if (!policy.refinementMinimum)
  throw new Error("Refinement coverage baseline is missing");
const measured = measureRust({
  manifest: "crates/text-refinement-worker/Cargo.toml",
  directory: "coverage/refinement",
  sourceRoot: ".",
  cargoArgs: [
    "--no-default-features",
    "--features",
    "test-fixture",
    "-p",
    "text-refinement-worker",
    "-p",
    "text-refinement-protocol",
    "--tests",
    "--",
    "--test-threads=1",
  ],
  selectFiles: Object.keys(policy.refinementMinimum),
});
console.log(JSON.stringify(measured, null, 2));
const errors = checkFileThresholds(measured, policy.refinementMinimum);
if (errors.length) throw new Error(errors.join("; "));
