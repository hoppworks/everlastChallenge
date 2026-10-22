import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  mkdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  parseTsv,
  selectCases,
  splitLines,
  generatedPcm,
  constructMasked,
  pcmWav,
  padPcm,
  scalePcm,
} from "../../scripts/tooling/audio-data.ts";
import {
  parseFilename,
  validateBase,
  flacInfo,
  selectCases as selectShort,
} from "../../scripts/short-corpus.ts";
import { calibrationCases } from "../../scripts/calibration.ts";
import { sha256 } from "../../scripts/tooling/files.ts";
await test("central selection preserves original frames and never cuts clips", () => {
  const rows = [
    [4, "4.wav", 14.5],
    [1, "2.wav", 7],
    [1, "1.wav", 4.5],
    [2, "3.wav", 9.5],
    [0, "0.wav", 3],
    [5, "5.wav", 31],
  ].flatMap(([id, file, seconds]) =>
    parseTsv(
      Buffer.from(`${id}\t${file}\ttext\tx\tx\t${Number(seconds) * 16000}\tx`),
    ),
  );
  const selected = selectCases(rows, 2, true);
  assert.deepEqual(
    selected.map((r) => [r.sentenceId, r.filename, r.frames, r.targetFrames]),
    [
      [1, "1.wav", 72000, 80000],
      [2, "3.wav", 152000, 160000],
      [4, "4.wav", 232000, 240000],
    ],
  );
  assert.throws(() => padPcm(Buffer.alloc(8), 3));
  assert.throws(() => selectCases(rows, 20));
});
await test("line parsing retains literal spaces and pinned control separators", () => {
  assert.deepEqual(splitLines("a\r\nb\u001cc \n"), ["a", "b", "c "]);
  assert.deepEqual(splitLines(""), []);
  assert.throws(() =>
    parseTsv(Buffer.from("\ufeff1\t1.wav\ttext\tx\tx\t64000\tx")),
  );
});
await test("synthetic generation and masked stress audio remain bounded and deterministic", () => {
  const noise = generatedPcm("noise", 640);
  assert.deepEqual(noise, generatedPcm("noise", 640));
  assert.notDeepEqual(noise, Buffer.alloc(1280));
  const source = Buffer.alloc(8);
  [100, -100, 200, -200].forEach((x, i) => source.writeInt16LE(x, i * 2));
  assert.deepEqual(
    [...Array(4)].map((_, i) => scalePcm(source, 0.05).readInt16LE(i * 2)),
    [5, -5, 10, -10],
  );
  const stress = constructMasked([source], 640);
  assert.equal(stress.pcm.length, 1280);
  assert.ok(stress.minimumRms >= 0.005);
  assert.throws(() => constructMasked([source], 319));
});
await test("short corpus validates filenames and FLAC format and selects independent fixed labels", () => {
  assert.throws(() =>
    parseFilename("../audio/lang-german_speaker-01_trial-0_digit-8.flac"),
  );
  const header = Buffer.alloc(42);
  header.write("fLaC");
  header[4] = 0x80;
  header.writeUIntBE(34, 5, 3);
  header.writeBigUInt64BE((48000n << 44n) | (15n << 36n) | 24000n, 18);
  assert.equal(flacInfo(header).frames, 24000);
  header[4] = 1;
  assert.throws(() => flacInfo(header));
  const rows: Parameters<typeof selectShort>[0] = [];
  for (const language of ["german", "english"])
    for (const split of ["calibration", "acceptance"])
      for (const digit of [8, 9]) {
        const speaker = split === "calibration" ? "00" : "04";
        rows.push({
          ...parseFilename(
            `audio/lang-${language}_speaker-${speaker}_trial-0_digit-${digit}.flac`,
          ),
          frames: 24000,
          sampleRate: 48000,
          channels: 1,
          bitsPerSample: 16,
          split,
        });
      }
  const selected = selectShort(rows);
  assert.deepEqual(
    selected.map((r) => r.reference),
    ["acht", "neun", "acht", "neun", "eight", "nine", "eight", "nine"],
  );
  assert.throws(() => selectShort(rows.slice(1)));
});
await test("calibration rejects malformed or redirected inherited audio and ignores acceptance cases", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "calibration-"));
  try {
    mkdirSync(path.join(dir, "artifacts/corpus/audio"), { recursive: true });
    const wav = pcmWav(Buffer.alloc(320));
    const file = path.join(dir, "artifacts/corpus/audio/base.wav");
    writeFileSync(file, wav);
    const cases = Array.from({ length: 34 }, (_, i) => ({
      id: String(i),
      split: "calibration",
      language: i % 2 ? "de" : "en",
      class: "clean",
      reference: "test",
      tags: [],
      source: { dataset: "google/fleurs" },
      audio: {
        path: "artifacts/corpus/audio/base.wav",
        frames: 160,
        sampleRate: 16000,
        sha256: sha256(wav),
      },
    }));
    const base = {
      schemaVersion: 1,
      unicodeVersion: "13.0.0",
      cases: [...cases, { id: "acceptance", split: "acceptance" }],
    };
    assert.equal(calibrationCases(base, dir).selected.length, 34);
    const broken = { ...base, cases: structuredClone(cases) };
    broken.cases[0]!.audio!.sha256 = "0".repeat(64);
    assert.throws(() => calibrationCases(broken, dir));
    const link = path.join(dir, "artifacts/corpus/audio/link.wav");
    symlinkSync(file, link);
    const redirected = { ...base, cases: structuredClone(cases) };
    redirected.cases[0]!.audio!.path = "artifacts/corpus/audio/link.wav";
    assert.throws(() => calibrationCases(redirected, dir));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

await test("short corpus rejects invalid inherited case identifiers before archive work", () => {
  for (const id of [null, 1, "", undefined])
    assert.throws(() =>
      validateBase({
        schemaVersion: 1,
        unicodeVersion: "13.0.0",
        cases: [{ id }],
      }),
    );
  assert.throws(() =>
    validateBase({
      schemaVersion: 1,
      unicodeVersion: "13.0.0",
      cases: [{ id: "a" }, { id: "a" }],
    }),
  );
});
