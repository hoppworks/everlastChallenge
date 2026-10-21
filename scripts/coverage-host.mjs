import fs from "node:fs";
import { measureRust, checkThresholds } from "./coverage.mjs";
const policy = JSON.parse(fs.readFileSync("config/coverage.json", "utf8"));
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
  selectFiles: [
    "native_service.rs",
    "controller.rs",
    "runtime.rs",
    "setup.rs",
    "desktop.rs",
  ],
});
console.log(JSON.stringify(measured, null, 2));
if (!policy.hostMinimum)
  throw new Error(
    "Host coverage baseline must be recorded before qualification",
  );
for (const [file, minimum] of Object.entries(policy.hostMinimum)) {
  const errors = checkThresholds(measured[file] ?? {}, { lines: minimum });
  if (errors.length) throw new Error(`${file}: ${errors.join("; ")}`);
}
