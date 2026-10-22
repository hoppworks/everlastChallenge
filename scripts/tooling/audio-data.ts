export const RATE = 16000;
export const MAX_FRAMES = 300 * RATE;
const MAX_MEMBER_BYTES = 20 * 1024 * 1024;
export interface CorpusRow {
  sentenceId: number;
  filename: string;
  reference: string;
  frames: number;
  targetFrames?: number;
}
export function check(ok: unknown, message: string): asserts ok {
  if (!ok) throw new Error(message);
}
export function roundEven(n: number): number {
  const low = Math.floor(n);
  return n - low === 0.5 ? (low % 2 === 0 ? low : low + 1) : Math.round(n);
}
/** Match Python's frozen splitlines protocol without trimming literal fields. */
export function splitLines(text: string): string[] {
  const breaks = new Set([10, 11, 12, 13, 28, 29, 30, 133, 8232, 8233]);
  const rows: string[] = [];
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (!breaks.has(text.charCodeAt(i))) continue;
    rows.push(text.slice(start, i));
    if (text.charCodeAt(i) === 13 && text.charCodeAt(i + 1) === 10) i++;
    start = i + 1;
  }
  if (start < text.length) rows.push(text.slice(start));
  return rows;
}
export function parseTsv(data: Buffer): CorpusRow[] {
  check(data.length <= 2 * 1024 * 1024, "TSV exceeds metadata bound");
  const text = new TextDecoder("utf-8", {
    fatal: true,
    ignoreBOM: true,
  }).decode(data);
  const lines = splitLines(text);
  check(lines.length > 0 && lines.length <= 10000, "TSV row count invalid");
  const seen = new Set<string>();
  return lines.map((line) => {
    const f = line.split("\t");
    check(f.length === 7, "TSV must contain seven columns");
    check(
      /^[0-9]{1,10}$/.test(f[0]!) && /^[0-9]{1,7}$/.test(f[5]!),
      "TSV ID or frames invalid",
    );
    const id = Number(f[0]),
      frames = Number(f[5]);
    check(
      id <= 0xffffffff && frames >= 1 && frames <= MAX_FRAMES,
      "TSV bounds exceeded",
    );
    check(
      /^[0-9]{1,64}\.wav$/.test(f[1]!) && !seen.has(f[1]!),
      "Unsafe or duplicate filename",
    );
    check(
      f[2] && Buffer.byteLength(f[2]) <= 256 * 1024,
      "Reference empty or too large",
    );
    seen.add(f[1]!);
    return { sentenceId: id, filename: f[1]!, reference: f[2]!, frames };
  });
}
export function canonical(rows: CorpusRow[]): CorpusRow[] {
  const seen = new Set<number>();
  return [...rows]
    .sort(
      (a, b) =>
        a.sentenceId - b.sentenceId ||
        (a.filename < b.filename ? -1 : a.filename > b.filename ? 1 : 0),
    )
    .filter((r) => {
      if (r.frames < 4 * RATE || r.frames > 30 * RATE || seen.has(r.sentenceId))
        return false;
      seen.add(r.sentenceId);
      return true;
    });
}
export function selectCases(
  rows: CorpusRow[],
  count: number,
  central = false,
): CorpusRow[] {
  const eligible = canonical(rows);
  check(eligible.length >= count, "Too few distinct eligible sentences");
  const selected = eligible.slice(0, count).map((r) => ({ ...r }));
  const used = new Set<number>();
  if (central)
    for (const sec of [5, 10, 15]) {
      const r = eligible.find(
        (r) =>
          !used.has(r.sentenceId) &&
          r.frames >= (sec - 1) * RATE &&
          r.frames <= sec * RATE,
      );
      check(r, "Missing central duration band");
      used.add(r.sentenceId);
      let existing = selected.find((x) => x.sentenceId === r.sentenceId);
      if (!existing) {
        existing = { ...r };
        selected.push(existing);
      }
      existing.targetFrames = sec * RATE;
    }
  return selected;
}
function wavParts(data: Buffer): { format: number; payload: Buffer } {
  check(
    data.length >= 12 &&
      data.length <= MAX_MEMBER_BYTES &&
      data.toString("ascii", 0, 4) === "RIFF" &&
      data.toString("ascii", 8, 12) === "WAVE" &&
      data.readUInt32LE(4) + 8 === data.length,
    "Invalid RIFF boundary",
  );
  let offset = 12,
    format: Buffer | undefined,
    payload: Buffer | undefined,
    chunks = 0;
  while (offset < data.length) {
    check(++chunks <= 256 && offset + 8 <= data.length, "Invalid WAV chunks");
    const id = data.toString("ascii", offset, offset + 4),
      size = data.readUInt32LE(offset + 4),
      start = offset + 8,
      end = start + size;
    check(end + (size & 1) <= data.length, "Truncated WAV");
    if (id === "fmt ") {
      check(!format && size >= 16, "Duplicate or invalid WAV format");
      format = data.subarray(start, start + 16);
    } else if (id === "data") {
      check(!payload, "Duplicate WAV data");
      payload = data.subarray(start, end);
    }
    offset = end + (size & 1);
  }
  check(format && payload, "Missing WAV format/data");
  const type = format.readUInt16LE(0),
    width = type === 3 ? 4 : 2;
  check(
    (type === 3 || type === 1) &&
      format.readUInt16LE(2) === 1 &&
      format.readUInt32LE(4) === RATE &&
      format.readUInt32LE(8) === RATE * width &&
      format.readUInt16LE(12) === width &&
      format.readUInt16LE(14) === width * 8,
    "WAV must be mono 16000 Hz",
  );
  return { format: type, payload };
}
export function decodeFloatWav(data: Buffer, frames: number): Buffer {
  check(
    Number.isInteger(frames) && frames > 0 && frames <= MAX_FRAMES,
    "Invalid frame count",
  );
  const { format, payload } = wavParts(data);
  check(
    format === 3 && payload.length === frames * 4,
    "Float WAV frame count differs",
  );
  const out = Buffer.alloc(frames * 2);
  for (let i = 0; i < frames; i++) {
    const value = payload.readFloatLE(i * 4);
    check(Number.isFinite(value), "Non-finite sample");
    out.writeInt16LE(
      Math.max(-32768, Math.min(32767, roundEven(value * 32768))),
      i * 2,
    );
  }
  return out;
}
export function decodePcmWav(data: Buffer, frames?: number): Buffer {
  const { format, payload } = wavParts(data);
  check(
    format === 1 &&
      payload.length % 2 === 0 &&
      payload.length > 0 &&
      (frames === undefined || payload.length === frames * 2),
    "PCM WAV frame count differs",
  );
  return payload;
}
export function pcmWav(pcm: Buffer): Buffer {
  check(
    pcm.length % 2 === 0 && pcm.length > 0 && pcm.length / 2 <= MAX_FRAMES,
    "Invalid PCM frame count",
  );
  const h = Buffer.alloc(44);
  h.write("RIFF");
  h.writeUInt32LE(36 + pcm.length, 4);
  h.write("WAVEfmt ", 8);
  h.writeUInt32LE(16, 16);
  h.writeUInt16LE(1, 20);
  h.writeUInt16LE(1, 22);
  h.writeUInt32LE(RATE, 24);
  h.writeUInt32LE(RATE * 2, 28);
  h.writeUInt16LE(2, 32);
  h.writeUInt16LE(16, 34);
  h.write("data", 36);
  h.writeUInt32LE(pcm.length, 40);
  return Buffer.concat([h, pcm]);
}
export function padPcm(pcm: Buffer, frames: number): Buffer {
  check(
    pcm.length % 2 === 0 &&
      Number.isInteger(frames) &&
      frames >= pcm.length / 2 &&
      frames <= MAX_FRAMES,
    "Padding would truncate or exceed limits",
  );
  return Buffer.concat([pcm, Buffer.alloc(frames * 2 - pcm.length)]);
}
export function scalePcm(pcm: Buffer, gain: number): Buffer {
  check(pcm.length % 2 === 0 && Number.isFinite(gain), "Invalid PCM or gain");
  const out = Buffer.alloc(pcm.length);
  for (let i = 0; i < pcm.length; i += 2)
    out.writeInt16LE(
      Math.max(-32768, Math.min(32767, roundEven(pcm.readInt16LE(i) * gain))),
      i,
    );
  return out;
}
function noise(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    state >>>= 0;
    return (state >>> 16) - 32768;
  };
}
export function generatedPcm(
  kind: "silence" | "noise",
  frames = 5 * RATE,
  seed = 0x45564552,
  gain = 0.04,
): Buffer {
  check(
    Number.isInteger(frames) && frames >= 0 && frames <= MAX_FRAMES,
    "Invalid generated frame count",
  );
  const out = Buffer.alloc(frames * 2);
  if (kind === "silence") return out;
  check(kind === "noise", "Unknown synthetic kind");
  const next = noise(seed);
  for (let i = 0; i < frames; i++)
    out.writeInt16LE(roundEven(next() * gain), i * 2);
  return out;
}
export function constructMasked(
  clips: Buffer[],
  targetFrames = MAX_FRAMES,
): { pcm: Buffer; minimumRms: number } {
  const source = Buffer.concat(clips);
  check(
    source.length > 0 &&
      source.length % 2 === 0 &&
      source.length / 2 <= targetFrames &&
      targetFrames <= MAX_FRAMES &&
      targetFrames % 320 === 0,
    "Invalid stress audio bounds",
  );
  const output = Buffer.alloc(targetFrames * 2),
    next = noise(0x45563300);
  let energy = 0,
    minimum = Infinity;
  for (let i = 0; i < targetFrames; i++) {
    const sample = i * 2 < source.length ? source.readInt16LE(i * 2) : 0;
    const mixed = roundEven(sample * 0.8 + next() * 0.03);
    check(mixed >= -32768 && mixed <= 32767, "Stress audio would clip");
    output.writeInt16LE(mixed, i * 2);
    energy += mixed * mixed;
    if ((i + 1) % 320 === 0) {
      const rms = Math.sqrt(energy / 320) / 32768;
      check(rms >= 0.005, "Stress audio has a safe-energy pause");
      minimum = Math.min(minimum, rms);
      energy = 0;
    }
  }
  return { pcm: output, minimumRms: minimum };
}
