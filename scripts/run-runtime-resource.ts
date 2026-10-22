/** Bounded resource workload owner. Resource observations do not qualify energy use. */
import { spawnSync } from "node:child_process";
import { lstatSync, mkdirSync, realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { check } from "./tooling/audio-data.ts";
import { hashFile } from "./tooling/files.ts";
import { readJson, record, writeNew } from "./tooling/io.ts";
import { owned } from "./controlled-probe.ts";
function observation(command: string[]) {
  const r = spawnSync(command[0]!, command.slice(1), {
    timeout: 5000,
    maxBuffer: 65536,
    encoding: "utf8",
  });
  return r.error || r.status !== 0
    ? { value: null, error: r.error?.message ?? `exit ${r.status}` }
    : { value: r.stdout.trim(), error: null };
}
export async function run(
  args: string[],
  signal?: AbortSignal,
): Promise<number> {
  check(args.length === 7, "Expected seven arguments");
  const [
    binaryRaw,
    modelRaw,
    selection,
    wavRaw,
    referenceRaw,
    parametersRaw,
    outputRaw,
  ] = args as [string, string, string, string, string, string, string];
  check(
    ["reference", "small", "mlx-reference-encoder"].includes(selection),
    "Invalid model selection",
  );
  const [binary, model, wav, reference, parameters] = [
    binaryRaw,
    modelRaw,
    wavRaw,
    referenceRaw,
    parametersRaw,
  ].map((p) => realpathSync(p)) as [string, string, string, string, string];
  const output = path.resolve(outputRaw),
    hostPath = output + ".host.json",
    completionPath = output + ".completion.json",
    folder = output + ".owner";
  for (const p of [output, hostPath, completionPath, folder]) {
    let exists = true;
    try {
      lstatSync(p);
    } catch (error) {
      if (error instanceof Error && "code" in error && error.code === "ENOENT")
        exists = false;
      else throw error;
    }
    check(!exists, "Output path already exists");
  }
  const command = [
    "/usr/bin/time",
    "-l",
    binary,
    model,
    selection,
    wav,
    reference,
    parameters,
    output,
  ];
  const hashes = await Promise.all(
    [fileURLToPath(import.meta.url), binary, parameters, wav, reference].map(
      hashFile,
    ),
  );
  writeNew(hostPath, {
    schemaVersion: 2,
    recordedBeforeChildLaunchUtc: new Date().toISOString(),
    command,
    sourceRevision: process.env.EVERLAST_BENCHMARK_REVISION ?? null,
    launcherSha256: hashes[0]!.sha256,
    binarySha256: hashes[1]!.sha256,
    parameterDocumentSha256: hashes[2]!.sha256,
    fixtureSha256: hashes[3]!.sha256,
    referenceSha256: hashes[4]!.sha256,
    cpu: observation(["/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"]),
    memoryBytes: observation(["/usr/sbin/sysctl", "-n", "hw.memsize"]),
    macOS: observation(["/usr/bin/sw_vers", "-productVersion"]),
    macOSBuild: observation(["/usr/bin/sw_vers", "-buildVersion"]),
    rustc: observation(["rustc", "--version"]),
    limits: [
      "Power source is not a measured energy profile",
      "time -l is an OS process observation, not live heap evidence",
      "Native inference is terminated only through its owned process group",
      "Host commands have five-second timeouts",
    ],
  });
  mkdirSync(folder, { mode: 0o700 });
  const result = await owned({
    command,
    folder,
    revision: process.env.EVERLAST_BENCHMARK_REVISION ?? "unrecorded",
    ...(signal ? { signal } : {}),
  });
  let reportError: string | null = null;
  try {
    const report = record(readJson(output, 8 * 1024 * 1024));
    check(
      report.kind === "runtime-lifecycle-resource-workload" &&
        report.behaviorPassed === true &&
        report.resourceQualified === false,
      "Unexpected resource report",
    );
  } catch (error) {
    reportError = error instanceof Error ? error.message : "Report invalid";
  }
  const effectiveExitCode = result.qualified && !reportError ? 0 : 1;
  writeNew(completionPath, {
    schemaVersion: 2,
    completedUtc: new Date().toISOString(),
    exitCode: result.exitCode,
    effectiveExitCode,
    reportError,
    ownerCompletionSha256: (
      await hashFile(path.join(folder, "owner-completion.json"))
    ).sha256,
    hostReportSha256: (await hashFile(hostPath)).sha256,
    resourceQualified: false,
  });
  return effectiveExitCode;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const control = new AbortController(),
    stop = () => control.abort();
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
  run(process.argv.slice(2), control.signal)
    .then((code) => {
      process.exitCode = code;
    })
    .catch(() => {
      console.error("Resource run failed; retain available receipts");
      process.exitCode = 2;
    })
    .finally(() => {
      process.off("SIGINT", stop);
      process.off("SIGTERM", stop);
    });
}
