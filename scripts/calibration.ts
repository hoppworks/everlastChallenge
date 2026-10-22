/** Freeze calibration-only inputs; acceptance cases are never evaluated here. */
import { spawnSync } from "node:child_process";
import { lstatSync, realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  check,
  decodePcmWav,
  generatedPcm,
  pcmWav,
  RATE,
  MAX_FRAMES,
} from "./tooling/audio-data.ts";
import {
  canonicalJson,
  publishIdentical,
  sha256,
  verifyExisting,
} from "./tooling/files.ts";
import { readBytes, parseJson, record } from "./tooling/io.ts";
import type { AudioCase } from "./corpus.ts";
const ROOT = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const BASE_SHA =
  "734199a55fe70bd0cf0dbc7df2fb3ea7afc7de14ed4daf20aa3d0a5672f0c60b";
const ROOM_SHA =
  "0f2131bd208b604498f32f0151e98ca7440a603d1309a25879c04a232ecba047";
const FFMPEG = "/opt/homebrew/bin/ffmpeg",
  FFMPEG_SHA =
    "c8173e9755795978bce8e104f8d7044fabe7d6d84044aba37d3648c6f4e4e1a8";
const FFMPEG_VERSION =
  "ffmpeg version 9.0.2 Copyright (c) 2000-2026 the FFmpeg developers";
export function decodeRoom(data: Buffer) {
  check(
    data.length === 10002720 && sha256(data) === ROOM_SHA,
    "Room source pin differs",
  );
  check(
    sha256(readBytes(FFMPEG, 128 * 1024 * 1024)) === FFMPEG_SHA,
    "Frozen FFmpeg differs",
  );
  const version = spawnSync(FFMPEG, ["-version"], {
    timeout: 5000,
    maxBuffer: 65536,
  });
  check(
    !version.error &&
      version.status === 0 &&
      version.stdout.toString().split("\n")[0] === FFMPEG_VERSION,
    "Frozen decoder version differs",
  );
  const result = spawnSync(
    FFMPEG,
    [
      "-nostdin",
      "-hide_banner",
      "-loglevel",
      "error",
      "-fflags",
      "+bitexact",
      "-i",
      "pipe:0",
      "-map",
      "0:a:0",
      "-t",
      "5",
      "-vn",
      "-ac",
      "1",
      "-ar",
      String(RATE),
      "-map_metadata",
      "-1",
      "-flags:a",
      "+bitexact",
      "-f",
      "s16le",
      "-acodec",
      "pcm_s16le",
      "pipe:1",
    ],
    { input: data, timeout: 30000, maxBuffer: 1024 * 1024 },
  );
  check(
    !result.error &&
      result.status === 0 &&
      result.stdout.length === 5 * RATE * 2,
    "Room decode failed",
  );
  return {
    pcm: result.stdout,
    decoder: { version: FFMPEG_VERSION, sha256: FFMPEG_SHA, path: FFMPEG },
  };
}
export function calibrationCases(input: unknown, root: string) {
  const base = record(input);
  check(
    base.schemaVersion === 1 &&
      base.unicodeVersion === "13.0.0" &&
      Array.isArray(base.cases),
    "Invalid manifest",
  );
  const selected = base.cases
    .map(record)
    .filter((c) => c.split === "calibration")
    .map((c) => structuredClone(c));
  check(selected.length === 34, "Unexpected calibration count");
  const ids = new Set<string>(),
    decoded = new Map<string, Buffer>(),
    first = new Map<string, Record<string, unknown>>();
  for (const c of selected) {
    check(
      typeof c.id === "string" &&
        !ids.has(c.id) &&
        ["de", "en"].includes(String(c.language)) &&
        ["clean", "challenging"].includes(String(c.class)) &&
        typeof c.reference === "string",
      "Invalid calibration case",
    );
    ids.add(c.id);
    const audio = record(c.audio);
    check(
      typeof audio.path === "string" &&
        /^artifacts\/corpus\/audio\/[^/]+\.wav$/.test(audio.path) &&
        !audio.path.includes("..") &&
        Number.isInteger(audio.frames) &&
        Number(audio.frames) > 0 &&
        Number(audio.frames) <= MAX_FRAMES &&
        audio.sampleRate === RATE,
      "Unsafe audio metadata",
    );
    const file = path.join(root, audio.path),
      relative = path.relative(realpathSync(root), realpathSync(file));
    check(
      !relative.startsWith("..") &&
        !path.isAbsolute(relative) &&
        !lstatSync(file).isSymbolicLink() &&
        lstatSync(file).size <= 12 * 1024 * 1024,
      "Audio path escapes root",
    );
    const wav = readBytes(file, 12 * 1024 * 1024);
    check(sha256(wav) === audio.sha256, "Audio hash differs");
    const pcm = decodePcmWav(wav);
    check(pcm.length === Number(audio.frames) * 2, "Audio frames differ");
    decoded.set(c.id, pcm);
    if (
      c.class === "clean" &&
      record(c.source).dataset === "google/fleurs" &&
      !first.has(String(c.language))
    )
      first.set(String(c.language), c);
  }
  check(
    first.has("de") && first.has("en"),
    "Clean calibration sentence missing",
  );
  return { base, selected, ids, decoded, first };
}
export function build(
  sourceManifest: string,
  roomSource: string,
  output: string,
  root = ROOT,
) {
  const raw = readBytes(sourceManifest, 16 * 1024 * 1024);
  check(
    raw.length <= 16 * 1024 * 1024 && sha256(raw) === BASE_SHA,
    "Full manifest pin differs",
  );
  const { base, selected, ids, decoded, first } = calibrationCases(
    parseJson(
      new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(raw),
    ),
    root,
  );
  const room = readBytes(roomSource, 10 * 1024 * 1024);
  const { pcm: roomPcm, decoder } = decodeRoom(room);
  const cases: unknown[] = [...selected],
    pending: Array<{ file: string; wav: Buffer }> = [];
  const append = (
    id: string,
    language: string,
    reference: string,
    tags: string[],
    source: Record<string, unknown>,
    pcm: Buffer,
  ) => {
    check(!ids.has(id), "Duplicate added ID");
    ids.add(id);
    const wav = pcmWav(pcm),
      relative = `artifacts/corpus/audio/${id}.wav`;
    const c: AudioCase = {
      id,
      language,
      reference,
      tags,
      source,
      class: "challenging",
      split: "calibration",
      audio: {
        path: relative,
        sha256: sha256(wav),
        frames: pcm.length / 2,
        sampleRate: RATE,
      },
    };
    cases.push(c);
    pending.push({ file: path.join(root, relative), wav });
  };
  for (const [language, original] of first) {
    for (const kind of ["zero", "noise"])
      append(
        `calibration-${language}-${kind}-5s`,
        language,
        "",
        ["synthetic", "not-room-noise", kind],
        {
          dataset: "synthetic",
          revision: "calibration-v2",
          split: "calibration",
          sha256: null,
          transformations: [
            {
              operation: "generate_" + kind,
              frames: 5 * RATE,
              generator: kind === "noise" ? "xorshift32-v1" : "zero",
              seed: kind === "noise" ? 0x45564332 : null,
              uniformAmplitude: kind === "noise" ? 0.04 : 0,
            },
          ],
        },
        kind === "zero"
          ? Buffer.alloc(5 * RATE * 2)
          : generatedPcm("noise", 5 * RATE, 0x45564332, 0.04),
      );
    const audio = record(original.audio);
    check(
      Array.isArray(original.tags) &&
        original.tags.every((x) => typeof x === "string"),
      "Invalid tags",
    );
    append(
      `calibration-${language}-trailing-silence`,
      language,
      String(original.reference),
      [...original.tags, "trailing-silence"],
      {
        dataset: "google/fleurs",
        revision: base.revision,
        split: "dev",
        baseCaseId: original.id,
        sha256: audio.sha256,
        transformations: [
          {
            operation: "copy_verified_pcm16_and_append_zero_frames",
            sourceFrames: audio.frames,
            addedFrames: 3 * RATE,
            addedSeconds: 3,
          },
        ],
      },
      Buffer.concat([
        decoded.get(String(original.id))!,
        Buffer.alloc(3 * RATE * 2),
      ]),
    );
    append(
      `calibration-${language}-room-5s`,
      language,
      "",
      ["natural-room-tone", "publisher-no-speech-label", "listening-pending"],
      {
        dataset: "Freesound",
        revision: "456207",
        split: "calibration",
        title: "Empty Office Room Tone",
        author: "richwise",
        license: "CC0-1.0",
        url: "https://cdn.freesound.org/previews/456/456207_1481531-hq.mp3",
        sha256: ROOM_SHA,
        bytes: 10002720,
        sampleRate: 48000,
        channels: 2,
        durationSeconds: 472.052,
        referenceProvenance:
          "publisher-described unoccupied office; independent listening pending",
        transformations: [
          {
            operation: "decode_first_five_seconds",
            decoder,
            input: "verified RAM bytes via stdin",
            startSeconds: 0,
            endSeconds: 5,
            sampleRate: RATE,
            channels: 1,
            format: "PCM16",
            bitexact: true,
            gainChange: false,
          },
        ],
      },
      roomPcm,
    );
  }
  check(
    Array.isArray(base.requiredCoveragePending ?? []) &&
      Array.isArray(base.sources ?? []),
    "Invalid provenance",
  );
  const report = {
    schemaVersion: 1,
    unicodeVersion: "13.0.0",
    dataset: "calibration-v2",
    revision: base.revision,
    sourceManifest: {
      path: "tests/fixtures/corpus/manifest.json",
      sha256: BASE_SHA,
      revision: base.revision,
    },
    sources: base.sources ?? [],
    selectionRules: {
      inheritedSplit: "calibration",
      inheritedCases: 34,
      addedCases: 8,
      noiseSeed: 0x45564332,
      noiseUniformAmplitude: 0.04,
      tailSelection:
        "first clean FLEURS case per language in full manifest order; append exactly three seconds of zeros",
      roomSelection:
        "fixed Freesound456207 HQ MP3, decoded interval [0,5), no gain change",
      acceptanceIncluded: false,
    },
    requiredCoveragePending: [
      ...((base.requiredCoveragePending ?? []) as unknown[]),
      "independent_roomtone_listening",
    ],
    cases,
  };
  const frozen = canonicalJson(report);
  check(
    cases.length === 42 && Buffer.byteLength(frozen) <= 16 * 1024 * 1024,
    "Calibration output bound",
  );
  check(
    ![sourceManifest, roomSource, ...pending.map((x) => x.file)].some(
      (p) => path.resolve(p) === path.resolve(output),
    ),
    "Output aliases input",
  );
  verifyExisting(output, frozen);
  for (const { file, wav } of pending) verifyExisting(file, wav);
  for (const { file, wav } of pending) publishIdentical(file, wav);
  publishIdentical(output, frozen);
  return report;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    const args = process.argv.slice(2);
    const get = (name: string) => {
      const i = args.indexOf(name);
      check(i >= 0 && args[i + 1], "Missing argument");
      return args[i + 1]!;
    };
    check(args.length === 6, "Invalid arguments");
    build(get("--source-manifest"), get("--room-source"), get("--output"));
    console.log("Calibration frozen; acceptance was not evaluated");
  } catch {
    console.error("Calibration source, decoder or output validation failed");
    process.exitCode = 2;
  }
}
