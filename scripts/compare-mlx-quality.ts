/** Pair-specific semantic review gates; no positive WER regression is allowed. */
import path from "node:path";
import { fileURLToPath } from "node:url";
import { evaluate } from "./tooling/quality.ts";
import { canonicalJson, sha256 } from "./tooling/files.ts";
import { readBytes, parseJson, record, writeNew } from "./tooling/io.ts";
export function compare(
  cases: unknown,
  baseline: unknown,
  candidate: unknown,
  reviews: unknown = {},
) {
  const reference = evaluate(cases, baseline),
    selected = evaluate(cases, candidate);
  const reviewed = record(reviews),
    original = new Map(
      (baseline as unknown[]).map((x) => {
        const r = record(x);
        return [r.id, r];
      }),
    ),
    changed = new Map(
      (candidate as unknown[]).map((x) => {
        const r = record(x);
        return [r.id, r];
      }),
    );
  let passed = true,
    checked = true;
  const groups = reference.groups.map((before, i) => {
    const after = selected.groups[i]!;
    const ok =
      before.complete &&
      after.complete &&
      before.refWords === after.refWords &&
      after.edits <= before.edits &&
      after.unwantedTextCount <= before.unwantedTextCount;
    passed &&= ok;
    return {
      split: before.split,
      language: before.language,
      class: before.class,
      baselineEdits: before.edits,
      candidateEdits: after.edits,
      refWords: before.refWords,
      passed: ok,
    };
  });
  const required = [];
  for (const [i, raw] of (cases as unknown[]).entries()) {
    const c = record(raw),
      before = reference.cases[i]!,
      after = selected.cases[i]!;
    if (
      (after.unwantedText === true && before.unwantedText !== true) ||
      ((before.hypWords ?? 0) > 0 &&
        after.hypWords === 0 &&
        before.refWords > 0)
    )
      passed = false;
    if (canonicalJson(original.get(c.id)) === canonicalJson(changed.get(c.id)))
      continue;
    const digest = sha256(
      canonicalJson({
        case: c,
        baseline: original.get(c.id) ?? null,
        candidate: changed.get(c.id) ?? null,
      }),
    );
    const review = record(reviewed[String(c.id)] ?? {});
    const accepted =
      review.pairSha256 === digest &&
      review.accepted === true &&
      typeof review.note === "string" &&
      review.note.trim().length > 0;
    checked &&= accepted;
    required.push({ id: c.id, pairSha256: digest, accepted });
  }
  return {
    schemaVersion: 2,
    reviewHashFormat: "sorted-json-pretty-v2",
    automaticGatePassed: passed,
    positiveWerDeltaAllowed: 0,
    groups,
    reviewsRequired: required,
    semanticReviewComplete: checked,
    qualified: passed && checked,
  };
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    const [manifestPath, baselinePath, candidatePath, ...args] =
      process.argv.slice(2);
    if (!manifestPath || !baselinePath || !candidatePath)
      throw new Error("Missing inputs");
    const outputIndex = args.indexOf("--output"),
      reviewIndex = args.indexOf("--reviews"),
      output = args[outputIndex + 1];
    if (outputIndex < 0 || !output || args.length !== (reviewIndex < 0 ? 2 : 4))
      throw new Error("Invalid options");
    const load = (file: string) => {
      const bytes = readBytes(file, 16 * 1024 * 1024);
      return {
        value: parseJson(
          new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(
            bytes,
          ),
        ),
        sha256: sha256(bytes),
      };
    };
    const manifestInput = load(manifestPath),
      baselineInput = load(baselinePath),
      candidateInput = load(candidatePath);
    const reviewInput = reviewIndex < 0 ? null : load(args[reviewIndex + 1]!);
    const manifest = record(manifestInput.value);
    if (manifest.schemaVersion !== 1 || manifest.unicodeVersion !== "13.0.0")
      throw new Error("Invalid manifest");
    const result = compare(
      manifest.cases,
      baselineInput.value,
      candidateInput.value,
      reviewInput?.value ?? {},
    );
    writeNew(output, {
      ...result,
      inputs: {
        manifest: manifestInput.sha256,
        baseline: baselineInput.sha256,
        candidate: candidateInput.sha256,
        ...(reviewInput ? { reviews: reviewInput.sha256 } : {}),
      },
    });
    if (!result.qualified) process.exitCode = 2;
  } catch {
    console.error(
      "Quality comparison failed; check inputs and use a new report path",
    );
    process.exitCode = 2;
  }
}
