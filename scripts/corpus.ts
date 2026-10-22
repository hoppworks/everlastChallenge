import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonical,
  check,
  constructMasked,
  decodeFloatWav,
  generatedPcm,
  MAX_FRAMES,
  padPcm,
  parseTsv,
  pcmWav,
  RATE,
  scalePcm,
  selectCases,
  type CorpusRow,
} from "./tooling/audio-data.ts";
import {
  canonicalJson,
  publishIdentical,
  readArchive,
  sha256,
  verifyFile,
  verifyExisting,
} from "./tooling/files.ts";
import { readBytes } from "./tooling/io.ts";
export {
  parseTsv,
  selectCases,
  decodeFloatWav,
  padPcm,
  scalePcm,
  pcmWav,
  generatedPcm,
  constructMasked,
} from "./tooling/audio-data.ts";
export interface AudioCase {
  id: string;
  language: string;
  class: string;
  split: string;
  reference: string;
  tags: string[];
  source: Record<string, unknown>;
  audio?: { path: string; sha256: string; frames: number; sampleRate: number };
}
interface Pin {
  language: string;
  split: "dev" | "test";
  archiveBytes: number;
  archiveSha256: string;
  tsvBytes: number;
  tsvSha256: string;
}
const ROOT = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const PINS = JSON.parse(
  readFileSync(
    new URL("../config/corpus-sources.json", import.meta.url),
    "utf8",
  ),
) as Record<string, Pin>;
const REVISION = "70bb2e84b976b7e960aa89f1c648e09c59f894dd";
export async function build(
  sourceDirectory: string,
  manifestPath: string,
  root = ROOT,
): Promise<Record<string, unknown>> {
  const selected = new Map<string, CorpusRow[]>(),
    all = new Map<string, CorpusRow[]>(),
    sources: unknown[] = [];
  for (const key of Object.keys(PINS).sort()) {
    const p = PINS[key]!;
    const archive = path.join(sourceDirectory, key + ".tar.gz"),
      tsv = path.join(sourceDirectory, key + ".tsv");
    await verifyFile(archive, p.archiveBytes, p.archiveSha256);
    await verifyFile(tsv, p.tsvBytes, p.tsvSha256);
    const data = readBytes(tsv, 2 * 1024 * 1024);
    check(
      data.length === p.tsvBytes && sha256(data) === p.tsvSha256,
      "TSV changed",
    );
    const rows = parseTsv(data);
    all.set(key, rows);
    selected.set(
      key,
      selectCases(
        rows,
        p.split === "dev" ? 10 : p.language === "de" ? 40 : 30,
        p.split === "test",
      ),
    );
    sources.push({
      key,
      language: p.language,
      split: p.split,
      archive: {
        filename: key + ".tar.gz",
        bytes: p.archiveBytes,
        sha256: p.archiveSha256,
      },
      tsv: { filename: key + ".tsv", bytes: p.tsvBytes, sha256: p.tsvSha256 },
    });
  }
  for (const lang of ["de", "en"]) {
    const keys = [...selected.keys()].filter((k) => PINS[k]!.language === lang);
    const dev = new Set(
      selected
        .get(keys.find((k) => PINS[k]!.split === "dev")!)!
        .map((r) => r.sentenceId),
    );
    check(
      !selected
        .get(keys.find((k) => PINS[k]!.split === "test")!)!
        .some((r) => dev.has(r.sentenceId)),
      "Calibration and acceptance overlap",
    );
  }
  const stressRows: CorpusRow[] = [];
  let stressFrames = 0;
  for (const row of canonical(all.get("de_de-test")!)) {
    if (stressFrames + row.frames > MAX_FRAMES) break;
    stressRows.push(row);
    stressFrames += row.frames;
  }
  check(stressRows.length, "No complete stress clip");
  const decoded = new Map<string, { pcm: Buffer; sha: string }>();
  for (const [key, rows] of selected) {
    const required = new Map(rows.map((r) => [r.filename, r]));
    if (key === "de_de-test")
      for (const row of stressRows) required.set(row.filename, row);
    const p = PINS[key]!;
    const found = await readArchive(
      path.join(sourceDirectory, key + ".tar.gz"),
      { bytes: p.archiveBytes, sha256: p.archiveSha256 },
      (name) => required.has(path.posix.basename(name)),
    );
    const seen = new Set<string>();
    for (const member of found) {
      const name = path.posix.basename(member.name);
      check(
        member.name === p.split + "/" + name &&
          member.type === "file" &&
          !seen.has(name),
        "Unsafe or duplicate selected member",
      );
      seen.add(name);
      decoded.set(key + "/" + name, {
        pcm: decodeFloatWav(member.data, required.get(name)!.frames),
        sha: sha256(member.data),
      });
    }
    check(seen.size === required.size, "Selected audio missing");
  }
  const pending: Array<{ c: AudioCase; pcm: Buffer }> = [];
  for (const [key, rows] of selected) {
    const p = PINS[key]!;
    const base: Array<{ c: AudioCase; pcm: Buffer }> = [];
    for (const row of rows) {
      const d = decoded.get(key + "/" + row.filename)!;
      let pcm = d.pcm;
      const transformations: Record<string, unknown>[] = [
        {
          operation: "float32_to_pcm16",
          sampleRate: RATE,
          rounding: "nearest-even",
          scale: 32768,
          saturation: [-32768, 32767],
        },
      ];
      const tags = ["fleurs", "read-speech"];
      if (row.targetFrames !== undefined) {
        transformations.push({
          operation: "append_zero_frames",
          frames: row.targetFrames - row.frames,
        });
        pcm = padPcm(pcm, row.targetFrames);
        tags.push("central", `central-${row.targetFrames / RATE}s`);
      }
      const c: AudioCase = {
        id: `fleurs-${p.language}-${p.split}-${row.sentenceId}`,
        language: p.language,
        class: "clean",
        split: p.split === "dev" ? "calibration" : "acceptance",
        reference: row.reference,
        tags,
        source: {
          dataset: "google/fleurs",
          revision: REVISION,
          split: p.split,
          sentenceId: row.sentenceId,
          filename: row.filename,
          sha256: d.sha,
          transformations,
        },
      };
      base.push({ c, pcm });
      pending.push({ c, pcm });
    }
    for (const { c, pcm } of base.slice(0, 5)) {
      const quiet = structuredClone(c);
      quiet.id += "-quiet";
      quiet.class = "challenging";
      quiet.tags = quiet.tags
        .filter((t) => !t.startsWith("central"))
        .concat("quiet");
      quiet.source.transformations = [
        ...(quiet.source.transformations as unknown[]),
        { operation: "pcm16_gain", gain: 0.05, rounding: "nearest-even" },
      ];
      pending.push({ c: quiet, pcm: scalePcm(pcm, 0.05) });
    }
  }
  for (const lang of ["de", "en"])
    for (const kind of ["silence", "noise"] as const)
      pending.push({
        c: {
          id: `synthetic-${lang}-${kind}-5s`,
          language: lang,
          class: "challenging",
          split: "acceptance",
          reference: "",
          tags: ["synthetic", kind, "not-room-noise"],
          source: {
            dataset: "synthetic",
            revision: "corpus-builder-v1",
            split: "acceptance",
            sentenceId: null,
            filename: null,
            sha256: null,
            transformations: [
              {
                operation: "generate_" + kind,
                frames: 5 * RATE,
                generator: kind === "noise" ? "xorshift32-v1" : "zero",
                seed: kind === "noise" ? 0x45564552 : null,
                uniformAmplitude: kind === "noise" ? 0.04 : 0,
              },
            ],
          },
        },
        pcm: generatedPcm(kind),
      });
  const clips: Buffer[] = [];
  const clipSources: unknown[] = [];
  let at = 0;
  for (const row of stressRows) {
    const d = decoded.get("de_de-test/" + row.filename)!;
    clips.push(d.pcm);
    clipSources.push({
      sentenceId: row.sentenceId,
      filename: row.filename,
      sha256: d.sha,
      sourceStartFrame: 0,
      sourceEndFrame: row.frames,
      outputStartFrame: at,
      outputEndFrame: at + row.frames,
    });
    at += row.frames;
  }
  const stress = constructMasked(clips);
  pending.push({
    c: {
      id: "constructed-de-masked-300s",
      language: "de",
      class: "challenging",
      split: "acceptance",
      reference: stressRows.map((r) => r.reference).join(" "),
      tags: ["constructed", "natural-source", "masked-pauses", "max-duration"],
      source: {
        dataset: "google/fleurs+synthetic-noise",
        revision: REVISION,
        split: "test",
        sentenceId: null,
        filename: null,
        sha256: null,
        clips: clipSources,
        transformations: [
          {
            operation: "whole_clip_concat_masked_pauses",
            speechGain: 0.8,
            noiseUniformAmplitude: 0.03,
            noiseSeed: 0x45563300,
            generator: "xorshift32-v1",
            paddingStartFrame: at,
            paddingEndFrame: MAX_FRAMES,
            outputFrames: MAX_FRAMES,
            rmsWindowFrames: 320,
            requiredMinimumWindowRms: 0.005,
            measuredMinimumWindowRms: stress.minimumRms,
          },
        ],
      },
    },
    pcm: stress.pcm,
  });
  const outputs = pending.map(({ c, pcm }) => {
    const wav = pcmWav(pcm);
    const relative = "artifacts/corpus/audio/" + c.id + ".wav";
    c.audio = {
      path: relative,
      sha256: sha256(wav),
      frames: pcm.length / 2,
      sampleRate: RATE,
    };
    return { file: path.join(root, relative), wav };
  });
  const cases = pending.map((x) => x.c);
  check(
    cases.length <= 500 &&
      new Set(cases.map((c) => c.id)).size === cases.length,
    "Corpus cases invalid",
  );
  check(
    !outputs.some((x) => path.resolve(x.file) === path.resolve(manifestPath)),
    "Manifest collides with audio",
  );
  const report = {
    schemaVersion: 1,
    unicodeVersion: "13.0.0",
    dataset: "google/fleurs",
    revision: REVISION,
    sources,
    selectionRules: {
      sort: ["integer sentenceId", "filename"],
      deduplicate:
        "first filename per sentenceId after 4..30-second range filter",
      baseAcceptance: { de: 40, en: 30 },
      calibrationPerLanguage: 10,
      centralSecondsPerLanguage: [5, 10, 15],
      centralBand:
        "target-1 <= source seconds <= target; first unused canonical ID; zero-pad only",
      quietPerLanguageAndSplit: 5,
      quietGain: 0.05,
      syntheticNoiseAmplitude: 0.04,
      syntheticNoiseSeed: 0x45564552,
      stressSelection:
        "complete canonical DE test prefix until the next whole clip would exceed the maximum; no cuts",
      maxFrames: MAX_FRAMES,
      reusedReferencesAreNotIndependentCoverage: true,
    },
    requiredCoveragePending: [
      "natural_de_utterances_0.2_to_1s",
      "natural_en_utterances_0.2_to_1s",
      "natural_continuous_de_300s_without_safe_pause",
      "natural_room_noise",
      "independent_listening_and_anchor_annotation_numbers_names_negations",
      "real_microphone_hotkey_end_to_end",
    ],
    cases,
  };
  // Validate every existing destination before creating any new output.
  for (const { file, wav } of outputs) verifyExisting(file, wav);
  verifyExisting(manifestPath, canonicalJson(report));
  for (const { file, wav } of outputs) publishIdentical(file, wav);
  publishIdentical(manifestPath, canonicalJson(report));
  return report;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const args = process.argv.slice(2);
  const source = args[args.indexOf("--source") + 1],
    manifest = args[args.indexOf("--manifest") + 1];
  if (
    !args.includes("--source") ||
    !args.includes("--manifest") ||
    !source ||
    !manifest
  ) {
    console.error("Usage: corpus.ts --source DIRECTORY --manifest FILE");
    process.exitCode = 2;
  } else
    build(source, manifest)
      .then(() => console.log("corpus: frozen; coverage remains partial"))
      .catch(() => {
        console.error("corpus: invalid source or output");
        process.exitCode = 2;
      });
}
