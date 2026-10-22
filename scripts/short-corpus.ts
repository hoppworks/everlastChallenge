/** Fixed short-speech corpus from publisher classes, with no inference-derived labels. */
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { check, pcmWav, RATE, splitLines } from "./tooling/audio-data.ts";
import {
  canonicalJson,
  hashFile,
  publishIdentical,
  readArchive,
  sha256,
  verifyExisting,
} from "./tooling/files.ts";
import { readJson, record } from "./tooling/io.ts";
import type { AudioCase } from "./corpus.ts";
const ROOT = path.resolve(fileURLToPath(new URL("..", import.meta.url))),
  FFMPEG = "/opt/homebrew/bin/ffmpeg";
const PIN = {
  bytes: 337342212,
  sha256: "4980128f02deba417ad3592e37166b8af603e200e298dd6108c20c4ffaa2e610",
};
const REFERENCES: Record<string, Record<number, string>> = {
  de: { 8: "acht", 9: "neun" },
  en: { 8: "eight", 9: "nine" },
};
interface Row {
  path: string;
  language: string;
  speaker: number;
  trial: number;
  digit: number;
  frames: number;
  sampleRate: number;
  channels: number;
  bitsPerSample: number;
  split?: string;
  reference?: string;
}
export function parseFilename(
  name: string,
): Omit<Row, "frames" | "sampleRate" | "channels" | "bitsPerSample"> {
  const match =
    /^audio\/lang-(german|english)_speaker-(0[0-9]|1[01])_trial-(0|[1-9][0-9]{0,4})_digit-([0-9])\.flac$/.exec(
      name,
    );
  check(match, "Unsafe HD filename");
  return {
    path: name,
    language: match[1] === "german" ? "de" : "en",
    speaker: Number(match[2]),
    trial: Number(match[3]),
    digit: Number(match[4]),
  };
}
export function flacInfo(data: Buffer) {
  check(
    data.length >= 42 &&
      data.toString("ascii", 0, 4) === "fLaC" &&
      (data[4]! & 127) === 0 &&
      data.readUIntBE(5, 3) === 34,
    "Invalid FLAC STREAMINFO",
  );
  const value = data.readBigUInt64BE(18),
    result = {
      sampleRate: Number(value >> 44n),
      channels: Number((value >> 41n) & 7n) + 1,
      bitsPerSample: Number((value >> 36n) & 31n) + 1,
      frames: Number(value & ((1n << 36n) - 1n)),
    };
  check(
    result.sampleRate === 48000 &&
      result.channels === 1 &&
      result.bitsPerSample === 16 &&
      result.frames >= 1 &&
      result.frames <= 14400000,
    "Invalid HD source format",
  );
  return result;
}
export function selectCases(rows: Row[]): Row[] {
  const train = new Set(
      rows.filter((r) => r.split === "calibration").map((r) => r.speaker),
    ),
    chosen: Row[] = [];
  for (const language of ["de", "en"])
    for (const split of ["calibration", "acceptance"]) {
      const used = new Set<number>();
      for (const digit of [8, 9]) {
        const candidates = rows.filter(
          (r) =>
            r.language === language &&
            r.split === split &&
            r.digit === digit &&
            r.frames >= 9600 &&
            r.frames <= 48000,
        );
        candidates.sort(
          (a, b) =>
            Number(split === "acceptance" && train.has(a.speaker)) -
              Number(split === "acceptance" && train.has(b.speaker)) ||
            Number(used.has(a.speaker)) - Number(used.has(b.speaker)) ||
            (a.path < b.path ? -1 : a.path > b.path ? 1 : 0),
        );
        check(candidates[0], "Required HD case missing");
        used.add(candidates[0].speaker);
        chosen.push({
          ...candidates[0],
          reference: REFERENCES[language]![digit]!,
        });
      }
    }
  return chosen;
}
export async function decoderIdentity() {
  const hash = await hashFile(FFMPEG);
  const result = spawnSync(FFMPEG, ["-version"], {
    timeout: 10000,
    maxBuffer: 65536,
  });
  check(
    !result.error && result.status === 0 && result.stdout.length,
    "FFmpeg version query failed",
  );
  return {
    version: new TextDecoder("utf-8", { fatal: true })
      .decode(result.stdout)
      .split("\n")[0]!,
    sha256: hash.sha256,
    path: FFMPEG,
  };
}
export function normalizeFlac(data: Buffer, info: Row): Buffer {
  const result = spawnSync(
    FFMPEG,
    [
      "-hide_banner",
      "-loglevel",
      "error",
      "-fflags",
      "+bitexact",
      "-i",
      "pipe:0",
      "-map",
      "0:a:0",
      "-map_metadata",
      "-1",
      "-ac",
      "1",
      "-ar",
      String(RATE),
      "-c:a",
      "pcm_s16le",
      "-flags:a",
      "+bitexact",
      "-fflags",
      "+bitexact",
      "-f",
      "s16le",
      "pipe:1",
    ],
    { input: data, timeout: 30000, maxBuffer: 2 * 1024 * 1024 },
  );
  check(
    !result.error && result.status === 0 && result.stdout.length % 2 === 0,
    "FLAC conversion failed",
  );
  const frames = result.stdout.length / 2;
  check(
    frames >= 3200 &&
      frames <= 16000 &&
      Math.abs(
        frames -
          Math.floor(
            (info.frames * RATE + info.sampleRate / 2) / info.sampleRate,
          ),
      ) <= 1,
    "FLAC duration differs",
  );
  return pcmWav(result.stdout);
}
export function validateBase(
  input: unknown,
): Record<string, unknown> & { cases: Record<string, unknown>[] } {
  const base = record(input);
  check(
    base.schemaVersion === 1 &&
      base.unicodeVersion === "13.0.0" &&
      Array.isArray(base.cases),
    "Invalid base manifest",
  );
  const cases = base.cases.map(record);
  check(
    cases.length <= 492 &&
      cases.every((c) => typeof c.id === "string" && c.id.length > 0) &&
      new Set(cases.map((c) => c.id)).size === cases.length,
    "Invalid or duplicate case IDs",
  );
  check(
    Array.isArray(base.sources ?? []) &&
      Array.isArray(base.requiredCoveragePending ?? []) &&
      ((base.requiredCoveragePending ?? []) as unknown[]).every(
        (x) => typeof x === "string",
      ),
    "Invalid provenance",
  );
  record(base.selectionRules ?? {});
  return { ...base, cases };
}
export async function build(
  basePath: string,
  source: string,
  output: string,
  root = ROOT,
): Promise<number> {
  check(
    ![basePath, source].some((p) => path.resolve(p) === path.resolve(output)),
    "Output aliases input",
  );
  const base = validateBase(readJson(basePath));
  const rows: Row[] = [],
    lists = new Map<string, Set<string>>(),
    seen = new Set<string>();
  await readArchive(
    source,
    PIN,
    () => true,
    2 * 1024 * 1024,
    12000,
    (member) => {
      check(!seen.has(member.name), "Duplicate archive member");
      seen.add(member.name);
      if (member.name === "audio" && member.type === "directory") return;
      check(member.type === "file" && member.size > 0, "Unsafe HD member");
      if (["train_filenames.txt", "test_filenames.txt"].includes(member.name)) {
        const names = splitLines(
          new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(
            member.data,
          ),
        ).map((n) => "audio/" + n);
        check(
          names.length &&
            names.length <= 12000 &&
            new Set(names).size === names.length,
          "Invalid HD split list",
        );
        names.forEach(parseFilename);
        lists.set(member.name, new Set(names));
      } else if (member.name !== "README.md")
        rows.push({ ...parseFilename(member.name), ...flacInfo(member.data) });
    },
  );
  const train = lists.get("train_filenames.txt"),
    test = lists.get("test_filenames.txt");
  check(
    train &&
      test &&
      ![...train].some((n) => test.has(n)) &&
      train.size + test.size === rows.length &&
      rows.every((r) => train.has(r.path) || test.has(r.path)),
    "HD split coverage differs",
  );
  for (const r of rows)
    r.split = train.has(r.path) ? "calibration" : "acceptance";
  const selected = selectCases(rows),
    decoded = new Map<string, Buffer>();
  await readArchive(
    source,
    PIN,
    (n) => selected.some((r) => r.path === n),
    2 * 1024 * 1024,
    12000,
    (member) => {
      check(
        !decoded.has(member.name) && member.type === "file",
        "Duplicate selected member",
      );
      decoded.set(member.name, member.data);
    },
  );
  const decoder = await decoderIdentity(),
    cases: AudioCase[] = [],
    files: Array<{ file: string; wav: Buffer }> = [];
  for (const r of selected) {
    const original = decoded.get(r.path);
    check(original, "Missing selected FLAC");
    const wav = normalizeFlac(original, r),
      id = `hd-${r.language}-${r.split}-s${String(r.speaker).padStart(2, "0")}-t${r.trial}-d${r.digit}`,
      relative = `artifacts/corpus/audio/${id}.wav`;
    cases.push({
      id,
      language: r.language,
      class: "clean",
      split: r.split!,
      reference: r.reference!,
      tags: ["natural-short", "digit-anchor", "independent-class-label"],
      audio: {
        path: relative,
        sha256: sha256(wav),
        frames: (wav.length - 44) / 2,
        sampleRate: RATE,
      },
      source: {
        dataset: "Heidelberg Digits",
        version: "1.0",
        split: r.split === "calibration" ? "train" : "test",
        filename: r.path,
        sha256: sha256(original),
        frames: r.frames,
        sampleRate: r.sampleRate,
        channels: r.channels,
        bitsPerSample: r.bitsPerSample,
        speaker: r.speaker,
        digit: r.digit,
        referenceProvenance:
          "filename digit/language class mapped to fixed word before inference",
        transformations: [
          {
            operation: "full-file decode and resample",
            decoder,
            sampleRate: RATE,
            channels: 1,
            format: "PCM16",
            bitexact: true,
            mapMetadata: -1,
            trim: false,
            gainChange: false,
          },
        ],
      },
    });
    files.push({ file: path.join(root, relative), wav });
  }
  check(base.cases.length + cases.length <= 500, "Too many cases");
  const combined = [...base.cases, ...cases];
  check(
    new Set(combined.map((c) => record(c).id)).size === combined.length,
    "Duplicate case IDs",
  );
  const sources = base.sources ?? [],
    rules = record(base.selectionRules ?? {}),
    pending = base.requiredCoveragePending ?? [];
  check(
    Array.isArray(sources) &&
      Array.isArray(pending) &&
      pending.every((x) => typeof x === "string"),
    "Invalid provenance",
  );
  const report = {
    ...base,
    cases: combined,
    sources: [
      ...sources,
      {
        sourceId: "heidelberg-digits-v1.0",
        dataset: "Heidelberg Digits original audio",
        version: "1.0",
        url: "https://zenkelab.org/datasets/shd/hd_audio.tar.gz",
        archiveBytes: PIN.bytes,
        archiveSha256: PIN.sha256,
        license: "CC-BY-4.0",
        licenseEvidence: "archive README.md: spoken digits dataset",
        attribution:
          "Copyright 2019 Benjamin Cramer, Yannik Stradmann, Johannes Schemmel, Friedemann Zenke; doi:10.1109/TNNLS.2020.3044364",
        decoder,
      },
    ],
    selectionRules: {
      ...rules,
      heidelbergDigits: {
        digits: [8, 9],
        references: REFERENCES,
        durationSeconds: [0.2, 1.0],
        splitLists: {
          calibration: "train_filenames.txt",
          acceptance: "test_filenames.txt",
        },
        order:
          "language, split, digit; prefer acceptance speakers absent from train, then unused speaker per pair, then canonical filename",
        trim: false,
        referenceProvenance:
          "publisher filename digit/language classes; fixed word mapping before ASR; no model-generated labels",
      },
    },
    requiredCoveragePending: [
      ...new Set([
        ...pending.filter(
          (x) =>
            ![
              "natural_de_utterances_0.2_to_1s",
              "natural_en_utterances_0.2_to_1s",
            ].includes(x as string),
        ),
        "exact_30s_duration_cases",
      ]),
    ],
  };
  const frozen = canonicalJson(report);
  check(Buffer.byteLength(frozen) <= 16 * 1024 * 1024, "Manifest too large");
  verifyExisting(output, frozen);
  for (const { file, wav } of files) {
    check(path.resolve(output) !== path.resolve(file), "Output aliases audio");
    verifyExisting(file, wav);
  }
  for (const { file, wav } of files) publishIdentical(file, wav);
  publishIdentical(output, frozen);
  return cases.length;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const [base, ...args] = process.argv.slice(2),
    source = args[args.indexOf("--source") + 1],
    output = args[args.indexOf("--output") + 1];
  if (
    !base ||
    args.length !== 4 ||
    !args.includes("--source") ||
    !args.includes("--output") ||
    !source ||
    !output
  ) {
    console.error(
      "Usage: short-corpus.ts BASE --source ARCHIVE --output NEW_MANIFEST",
    );
    process.exitCode = 2;
  } else
    build(base, source, output)
      .then((n) => console.log(`short-corpus: ${n} fixed cases appended`))
      .catch(() => {
        console.error(
          "Short corpus source, decoder or output validation failed",
        );
        process.exitCode = 2;
      });
}
