/** Independent CPU DFT reference. No model, MLX or Python runtime is used. */
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";
import { check } from "./tooling/audio-data.ts";
import { sha256 } from "./tooling/files.ts";
import { writeNew, readBytes } from "./tooling/io.ts";
const FILTER_SHA =
  "7450ae70723a5ef9d341e3cee628c7cb0177f36ce42c44b7ed2bf3325f0f6d4c";
const REVISION = "86098128c0b4f24f0e2aa2994de830614b474227";
function filters(archive: Buffer, count: number): number[] {
  check(
    archive.length <= 4 * 1024 * 1024 && sha256(archive) === FILTER_SHA,
    "Filter archive identity differs",
  );
  const name = `mel_${count}.npy`;
  let at = 0;
  while (at + 30 <= archive.length && archive.readUInt32LE(at) === 0x04034b50) {
    const flags = archive.readUInt16LE(at + 6),
      method = archive.readUInt16LE(at + 8),
      compressed = archive.readUInt32LE(at + 18),
      size = archive.readUInt32LE(at + 22);
    const nameBytes = archive.readUInt16LE(at + 26),
      extraBytes = archive.readUInt16LE(at + 28),
      start = at + 30 + nameBytes + extraBytes;
    check(
      (flags & 9) === 0 &&
        compressed !== 0xffffffff &&
        start + compressed <= archive.length &&
        size < 1024 * 1024,
      "Unsupported ZIP member",
    );
    if (archive.toString("utf8", at + 30, at + 30 + nameBytes) === name) {
      const packed = archive.subarray(start, start + compressed);
      check(method === 0 || method === 8, "Unsupported ZIP method");
      const data =
        method === 0
          ? packed
          : inflateRawSync(packed, { maxOutputLength: 1024 * 1024 });
      check(
        data.length === size &&
          data
            .subarray(0, 8)
            .equals(Buffer.from([0x93, 78, 85, 77, 80, 89, 1, 0])),
        "Invalid NPY header",
      );
      const offset = 10 + data.readUInt16LE(8),
        header = data.toString("ascii", 10, offset);
      check(
        /^\{'descr': '<f4', 'fortran_order': False, 'shape': \((80|128), 201\),? \}\s*$/.test(
          header,
        ) &&
          header.includes(`(${count}, 201)`) &&
          data.length === offset + count * 201 * 4,
        "Unexpected filter shape",
      );
      return Array.from({ length: count * 201 }, (_, i) =>
        data.readFloatLE(offset + i * 4),
      );
    }
    at = start + compressed;
  }
  throw new Error("Filter bank missing");
}
export function oracle(
  samples: number[],
  count: number,
  padding: number,
  bank: number[],
): number[] {
  check(
    samples.length >= 401 &&
      samples.length + padding <= 32000 &&
      padding >= 0 &&
      Number.isInteger(padding) &&
      bank.length === count * 201 &&
      samples.every(Number.isFinite) &&
      bank.every(Number.isFinite),
    "Invalid oracle input",
  );
  const signal = samples.concat(Array<number>(padding).fill(0));
  const window = Array.from(
    { length: 400 },
    (_, i) => 0.5 * (1 - Math.cos((2 * Math.PI * i) / 400)),
  );
  const real = Array.from({ length: 201 }, (_, k) =>
    Array.from({ length: 400 }, (_, i) =>
      Math.cos((-2 * Math.PI * k * i) / 400),
    ),
  );
  const imag = Array.from({ length: 201 }, (_, k) =>
    Array.from({ length: 400 }, (_, i) =>
      Math.sin((-2 * Math.PI * k * i) / 400),
    ),
  );
  const spectrum: number[] = [];
  for (let frame = 0; frame < Math.floor(signal.length / 160); frame++) {
    const values = window.map((w, i) => {
      let index = frame * 160 + i - 200;
      if (index < 0) index = -index;
      if (index >= signal.length) index = 2 * signal.length - 2 - index;
      return signal[index]! * w;
    });
    const power = real.map((basis, k) => {
      let r = 0,
        im = 0;
      for (let i = 0; i < 400; i++) {
        r += values[i]! * basis[i]!;
        im += values[i]! * imag[k]![i]!;
      }
      return Math.hypot(r, im) ** 2;
    });
    for (let m = 0; m < count; m++) {
      let sum = 0;
      for (let k = 0; k < 201; k++) sum += power[k]! * bank[m * 201 + k]!;
      spectrum.push(Math.log10(Math.max(1e-10, sum)));
    }
  }
  const floor = Math.max(...spectrum) - 8;
  return spectrum.map((value) => Math.fround((Math.max(value, floor) + 4) / 4));
}
export function generate(source: string): unknown {
  const data = readBytes(source, 4 * 1024 * 1024);
  let noise = 7;
  const tones = Array.from({ length: 1600 }, (_, i) => {
    noise = (Math.imul(1664525, noise) + 1013904223) >>> 0;
    return Math.fround(
      0.4 * Math.sin((2 * Math.PI * 443.3 * i) / 16000) +
        0.2 * Math.cos((2 * Math.PI * 1207.7 * i) / 16000) +
        0.03 * (noise / 2 ** 31 - 1),
    );
  });
  const impulse = Array<number>(641).fill(0);
  impulse[0] = 0.75;
  impulse[320] = -0.5;
  impulse[640] = 0.25;
  const inputs = [
    {
      name: "off-bin-tones-and-broadband",
      samples: tones,
      mels: 128,
      padding: 0,
    },
    {
      name: "boundary-impulses-with-right-padding",
      samples: impulse,
      mels: 80,
      padding: 320,
    },
    {
      name: "silence-floor",
      samples: Array<number>(1600).fill(0),
      mels: 128,
      padding: 0,
    },
  ];
  return {
    schemaVersion: 1,
    method:
      "independent direct DFT, periodic Hann400, hop160, reflect center, final STFT frame removed, exact official Slaney banks, Whisper log clamp",
    source: `https://github.com/openai/whisper/tree/${REVISION}`,
    filterSha256: FILTER_SHA,
    generatorSha256: sha256(readFileSync(fileURLToPath(import.meta.url))),
    cases: inputs.map((input) => ({
      ...input,
      shape: [
        Math.floor((input.samples.length + input.padding) / 160),
        input.mels,
      ],
      expected: oracle(
        input.samples,
        input.mels,
        input.padding,
        filters(data, input.mels),
      ),
      maximumAbsoluteError: 0.0002,
    })),
  };
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const [source, output, ...extra] = process.argv.slice(2);
  try {
    check(
      source && output && !extra.length,
      "Usage: generate-whisper-mel-oracle.ts FILTERS.npz NEW_OUTPUT.json",
    );
    writeNew(output, generate(source));
  } catch {
    console.error(
      "Mel oracle generation failed; verify the pinned archive and output path",
    );
    process.exitCode = 2;
  }
}
