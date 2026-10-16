import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export function checkThresholds(actual, required) {
  return Object.entries(required).flatMap(([metric, minimum]) => {
    if (!Number.isFinite(actual[metric]))
      return [`${metric}: missing coverage measurement`];
    return actual[metric] < minimum
      ? [`${metric}: ${actual[metric]} below ${minimum}`]
      : [];
  });
}

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
    ...options,
  });
}

export function measureRust({ manifest, directory, sourceRoot }) {
  const output = path.resolve(directory);
  fs.rmSync(output, { recursive: true, force: true });
  fs.mkdirSync(output, { recursive: true });
  const environment = {
    ...process.env,
    RUSTFLAGS: "-C instrument-coverage -C link-dead-code",
    LLVM_PROFILE_FILE: `${output}/run-%p-%m.profraw`,
  };
  const messages = run(
    "cargo",
    ["test", "--manifest-path", manifest, "--tests", "--message-format=json"],
    { env: environment },
  );
  const objects = messages
    .split("\n")
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line))
    .filter(
      (message) =>
        message.reason === "compiler-artifact" &&
        message.profile.test &&
        message.executable,
    )
    .map((message) => message.executable);
  const profiles = fs
    .readdirSync(output)
    .filter((file) => file.endsWith(".profraw"))
    .map((file) => path.join(output, file));
  if (!objects.length || !profiles.length)
    throw new Error("No instrumented Rust test artifacts");
  const merged = path.join(output, "merged.profdata");
  run("xcrun", [
    "llvm-profdata",
    "merge",
    "-sparse",
    ...profiles,
    "-o",
    merged,
  ]);
  const report = JSON.parse(
    run("xcrun", [
      "llvm-cov",
      "export",
      "--summary-only",
      objects[0],
      ...objects.slice(1).flatMap((object) => ["-object", object]),
      `-instr-profile=${merged}`,
    ]),
  );
  const files = report.data
    .flatMap((data) => data.files)
    .filter((file) =>
      path
        .resolve(file.filename)
        .startsWith(path.resolve(sourceRoot) + path.sep),
    );
  if (!files.length) throw new Error("No core source measured");
  const totals = files.reduce(
    (sum, file) => ({
      count: sum.count + file.summary.lines.count,
      covered: sum.covered + file.summary.lines.covered,
    }),
    { count: 0, covered: 0 },
  );
  if (!totals.count) throw new Error("No executable core lines measured");
  return { lines: (100 * totals.covered) / totals.count };
}

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(file) : [file];
  });
}

function main() {
  const policy = JSON.parse(fs.readFileSync("config/coverage.json", "utf8"));
  const core = walk("crates/dictation-core/src").some((file) =>
    /\bfn\s+\w+/u.test(fs.readFileSync(file, "utf8")),
  );
  if (core) {
    const measurement = measureRust({
      manifest: "crates/dictation-core/Cargo.toml",
      directory: "coverage/rust",
      sourceRoot: "crates/dictation-core/src",
    });
    console.log("Core coverage:", measurement);
    const errors = checkThresholds(measurement, {
      lines: policy.coreLineMinimum,
    });
    if (errors.length) throw new Error(errors.join("\n"));
  } else console.log("Core coverage: not applicable; no executable core yet.");
  const excluded = new Set(policy.exclusions.map((item) => item.path));
  const frontend = walk("src").some(
    (file) =>
      /\.tsx?$/u.test(file) && !file.includes(".test.") && !excluded.has(file),
  );
  if (frontend)
    run("node_modules/.bin/vitest", ["run", "--coverage"], {
      stdio: "inherit",
    });
  else
    console.log(
      "Frontend coverage: not applicable; only root bootstrap exists.",
    );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
