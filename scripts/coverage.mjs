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
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  });
}

export function resolveRustCoverageTools(compiler = run) {
  const version = compiler("rustc", ["-vV"]);
  const host = /^host: ([a-zA-Z0-9_.-]+)$/mu.exec(version)?.[1];
  if (!host) throw new Error("Cannot identify the selected compiler host");
  const sysroot = compiler("rustc", ["--print", "sysroot"]).trim();
  if (!path.isAbsolute(sysroot))
    throw new Error("The selected compiler sysroot must be absolute");
  const directory = path.join(sysroot, "lib", "rustlib", host, "bin");
  return {
    profdata: path.join(directory, "llvm-profdata"),
    cov: path.join(directory, "llvm-cov"),
  };
}

export function measureRust({
  manifest,
  directory,
  sourceRoot,
  cargoArgs = ["--tests"],
  selectFiles,
}) {
  const tools = resolveRustCoverageTools();
  for (const tool of Object.values(tools))
    if (!fs.existsSync(tool))
      throw new Error("Selected Rust toolchain needs llvm-tools-preview");
  const output = path.resolve(directory);
  fs.rmSync(output, { recursive: true, force: true });
  fs.mkdirSync(output, { recursive: true });
  const environment = {
    ...process.env,
    // Instrumentation includes coverage mappings; retaining all dead code also
    // links unused Swift FFI from Tauri's host build helpers on macOS.
    RUSTFLAGS: "-C instrument-coverage",
    LLVM_PROFILE_FILE: `${output}/run-%p-%m.profraw`,
  };
  let messages;
  try {
    messages = run(
      "cargo",
      [
        "test",
        "--locked",
        "--manifest-path",
        manifest,
        "--message-format=json",
        ...cargoArgs,
      ],
      { env: environment },
    );
  } catch (error) {
    for (const line of String(error.stdout ?? "").split("\n")) {
      if (!line.startsWith("{")) continue;
      const message = JSON.parse(line);
      if (message.reason === "compiler-message")
        console.error(message.message.rendered);
    }
    throw error;
  }
  fs.writeFileSync(path.join(output, "cargo-output.jsonl"), messages);
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
  run(tools.profdata, ["merge", "-sparse", ...profiles, "-o", merged]);
  const report = JSON.parse(
    run(tools.cov, [
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
  if (!files.length) throw new Error("No requested source measured");
  if (selectFiles) {
    const summaries = Object.fromEntries(
      selectFiles.map((name) => {
        const match = files.find(
          (file) =>
            path.resolve(file.filename) === path.resolve(sourceRoot, name),
        );
        if (!match || !match.summary.lines.count)
          throw new Error(`No coverage for ${name}`);
        return [
          name,
          {
            lines: match.summary.lines.percent,
            count: match.summary.lines.count,
            covered: match.summary.lines.covered,
          },
        ];
      }),
    );
    fs.writeFileSync(
      path.join(output, "selected-summary.json"),
      JSON.stringify(summaries, null, 2) + "\n",
    );
    return summaries;
  }
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
