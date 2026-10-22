import fs from "node:fs";
import { measureRust, checkFileThresholds } from "./coverage.mjs";
const policy = JSON.parse(fs.readFileSync("config/coverage.json", "utf8"));
if (!policy.hostMinimum)
  throw new Error(
    "Host coverage baseline must be recorded before qualification",
  );
const measured = measureRust({
  manifest: "src-tauri/Cargo.toml",
  directory: "coverage/host",
  sourceRoot: "src-tauri/src",
  cargoArgs: [
    "--lib",
    "--features",
    "measurement,test-control",
    "--",
    "--test-threads=1",
  ],
  selectFiles: Object.keys(policy.hostMinimum),
});
console.log(JSON.stringify(measured, null, 2));
const errors = checkFileThresholds(measured, policy.hostMinimum);
if (errors.length) throw new Error(errors.join("; "));
