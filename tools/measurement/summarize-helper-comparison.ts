/** Recompute published historical medians without running ASR. */
import { readFileSync } from "node:fs";
import { check } from "../../scripts/tooling/audio-data.ts";
import { parseJson, record } from "../../scripts/tooling/io.ts";
import { sha256 } from "../../scripts/tooling/files.ts";
export function summarize(raw: string) {
  const data = record(parseJson(raw));
  check(
    Array.isArray(data.rows) && Array.isArray(data.cases),
    "Invalid comparison",
  );
  const rows = data.rows.map(record);
  check(
    rows.length === 30 && new Set(rows.map((r) => r.slot)).size === 30,
    "Expected 30 distinct slots",
  );
  return {
    inputSha256: sha256(raw),
    cases: data.cases.map((value) => {
      const c = record(value);
      const groups: Record<
        string,
        { medianMs: number; minMs: number; maxMs: number }
      > = {};
      const transcripts = new Set();
      for (const side of ["everlast", "voicely"]) {
        const selected = rows.filter(
          (r) => r.caseId === c.id && r.side === side,
        );
        check(
          selected
            .map((r) => r.repetition)
            .sort()
            .join(",") === "1,2,3,4,5",
          "Expected five repetitions",
        );
        const times = selected
          .map((r) => {
            check(
              r.status === "ok" &&
                r.exitCode === 0 &&
                r.timeout === false &&
                r.processGroupAbsent === true &&
                r.descendantsAfterLeaderExit === false &&
                typeof r.wallMs === "number" &&
                Number.isFinite(r.wallMs) &&
                r.wallMs > 0,
              "Invalid measurement",
            );
            transcripts.add(r.transcriptSha256);
            return r.wallMs;
          })
          .sort((a, b) => a - b);
        groups[side] = {
          medianMs: times[2]!,
          minMs: times[0]!,
          maxMs: times[4]!,
        };
      }
      check(transcripts.size === 1, "Transcripts differ");
      return {
        caseId: c.id,
        everlast: groups.everlast!,
        voicely: groups.voicely!,
        reductionPercent:
          100 * (1 - groups.everlast!.medianMs / groups.voicely!.medianMs),
        ratio: groups.voicely!.medianMs / groups.everlast!.medianMs,
      };
    }),
  };
}
if (process.argv[1]?.endsWith("summarize-helper-comparison.ts")) {
  try {
    console.log(
      JSON.stringify(
        summarize(
          readFileSync(
            process.argv[2] ??
              "docs/benchmarks/helper-comparison-2026-10-20.json",
            "utf8",
          ),
        ),
        null,
        2,
      ),
    );
  } catch {
    console.error("Invalid historical comparison");
    process.exitCode = 2;
  }
}
