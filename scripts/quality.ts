import path from "node:path";
import { fileURLToPath } from "node:url";
import { realpathSync } from "node:fs";
import { evaluate, UNICODE_VERSION } from "./tooling/quality.ts";
import { readJson, record, writeNew } from "./tooling/io.ts";
export { evaluate, normalize } from "./tooling/quality.ts";
export function main(args: string[]): number {
  try {
    if (args.length !== 4 || args[2] !== "--output")
      throw new Error("Usage: quality.ts MANIFEST RESULTS --output REPORT");
    const [manifestFile, resultFile, , output] = args as [
      string,
      string,
      string,
      string,
    ];
    const manifest = record(readJson(manifestFile));
    if (manifest.schemaVersion !== 1)
      throw new Error("Manifest schemaVersion must be 1");
    if (manifest.unicodeVersion !== UNICODE_VERSION)
      throw new Error("Manifest unicodeVersion must be 13.0.0");
    const canonical = (p: string) => {
      try {
        return realpathSync(p);
      } catch {
        return path.resolve(p);
      }
    };
    if (
      [manifestFile, resultFile].some((p) => canonical(p) === canonical(output))
    )
      throw new Error("Report output must differ from input paths");
    const report = evaluate(manifest.cases, readJson(resultFile));
    writeNew(output, report);
    console.log(
      `quality: evaluated ${report.cases.length} cases in ${report.groups.length} groups; ${report.cases.filter((x) => x.status !== "ok").length} failed/missing; ${report.cases.filter((x) => x.unwantedText).length} unwanted silence outputs`,
    );
    return 0;
  } catch {
    console.error(
      "quality: invalid input, output already exists, or file I/O failed",
    );
    return 2;
  }
}
if (
  process.argv[1] &&
  fileURLToPath(import.meta.url) === path.resolve(process.argv[1])
)
  process.exitCode = main(process.argv.slice(2));
