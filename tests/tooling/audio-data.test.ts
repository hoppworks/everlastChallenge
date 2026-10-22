import assert from "node:assert/strict";
import { test } from "node:test";
import {
  decodeFloatWav,
  pcmWav,
  generatedPcm,
  roundEven,
  parseTsv,
  selectCases,
} from "../../scripts/tooling/audio-data.ts";
await test("PCM conversion uses ties-to-even and rejects malformed/nonfinite RIFF input", () => {
  assert.deepEqual(
    [0.5, 1.5, 2.5, -0.5, -1.5, -2.5].map(roundEven),
    [0, 2, 2, 0, -2, -2],
  );
  const data = Buffer.alloc(44 + 16);
  data.write("RIFF");
  data.writeUInt32LE(data.length - 8, 4);
  data.write("WAVEfmt ", 8);
  data.writeUInt32LE(16, 16);
  data.writeUInt16LE(3, 20);
  data.writeUInt16LE(1, 22);
  data.writeUInt32LE(16000, 24);
  data.writeUInt32LE(64000, 28);
  data.writeUInt16LE(4, 32);
  data.writeUInt16LE(32, 34);
  data.write("data", 36);
  data.writeUInt32LE(16, 40);
  [0, 0.5, -1, 1].forEach((x, i) => data.writeFloatLE(x, 44 + i * 4));
  const pcm = decodeFloatWav(data, 4);
  assert.deepEqual(
    Array.from({ length: 4 }, (_, i) => pcm.readInt16LE(i * 2)),
    [0, 16384, -32768, 32767],
  );
  data.writeFloatLE(NaN, 44);
  assert.throws(() => decodeFloatWav(data, 4), /finite/);
});
await test("frozen synthetic silence and WAV sizes are deterministic", () => {
  const pcm = generatedPcm("silence", 10);
  assert.equal(pcm.length, 20);
  assert.ok(pcm.every((x) => x === 0));
  assert.equal(pcmWav(pcm).readUInt32LE(40), 20);
  assert.throws(() => pcmWav(Buffer.alloc(3)));
});
await test("TSV preserves literal quotes and canonical selection never duplicates sentence IDs", () => {
  const row = (id: number, file: string, frames: number) =>
    `${id}\t${file}\t"a"\tx\tx\t${frames}\tx`;
  const data = Buffer.from(
    [
      row(2, "2.wav", 64000),
      row(1, "1.wav", 64000),
      row(1, "3.wav", 64000),
    ].join("\n"),
  );
  assert.deepEqual(
    selectCases(parseTsv(data), 2).map((x) => x.sentenceId),
    [1, 2],
  );
  assert.equal(parseTsv(data)[0]?.reference, '"a"');
  assert.throws(() => parseTsv(Buffer.from(row(1, "../x.wav", 64000))));
});
