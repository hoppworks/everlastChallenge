import {
  closeSync,
  constants,
  fsyncSync,
  fstatSync,
  linkSync,
  openSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";

export function parseJson(source: string): unknown {
  let i = 0;
  const fail = (): never => {
    throw new Error("Invalid JSON document");
  };
  const ws = () => {
    while (/[\t\n\r ]/.test(source[i] ?? "!")) i++;
  };
  const string = (): string => {
    if (source[i] !== '"') return fail();
    const start = i++;
    while (i < source.length) {
      const c = source[i++];
      if (c === "\\") {
        i++;
        continue;
      }
      if (c === '"') {
        try {
          return JSON.parse(source.slice(start, i)) as string;
        } catch {
          return fail();
        }
      }
    }
    return fail();
  };
  const value = (depth: number): unknown => {
    if (depth > 256) return fail();
    ws();
    const c = source[i];
    if (c === '"') return string();
    if (c === "{") {
      i++;
      ws();
      const out: Record<string, unknown> = {};
      const keys = new Set<string>();
      if (source[i] === "}") {
        i++;
        return out;
      }
      for (;;) {
        ws();
        const key = string();
        if (keys.has(key))
          throw new Error("JSON contains duplicate object keys");
        keys.add(key);
        ws();
        if (source[i++] !== ":") return fail();
        Object.defineProperty(out, key, {
          value: value(depth + 1),
          enumerable: true,
          writable: true,
          configurable: true,
        });
        ws();
        const next = source[i++];
        if (next === "}") return out;
        if (next !== ",") return fail();
      }
    }
    if (c === "[") {
      i++;
      ws();
      const out: unknown[] = [];
      if (source[i] === "]") {
        i++;
        return out;
      }
      for (;;) {
        out.push(value(depth + 1));
        ws();
        const next = source[i++];
        if (next === "]") return out;
        if (next !== ",") return fail();
      }
    }
    for (const [word, result] of [
      ["true", true],
      ["false", false],
      ["null", null],
    ] as const) {
      if (source.startsWith(word, i)) {
        i += word.length;
        return result;
      }
    }
    const match = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(
      source.slice(i),
    );
    if (!match) return fail();
    i += match[0].length;
    const n = Number(match[0]);
    if (!Number.isFinite(n))
      throw new Error("JSON contains a non-finite number");
    return n;
  };
  const result = value(0);
  ws();
  if (i !== source.length) fail();
  return result;
}
export function readBytes(file: string, maxBytes: number): Buffer {
  if (!Number.isSafeInteger(maxBytes) || maxBytes < 0)
    throw new Error("Invalid read limit");
  // Regular files behave unchanged; a FIFO must not wait for a writer before fstat rejects it.
  const fd = openSync(file, constants.O_RDONLY | constants.O_NONBLOCK);
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.size > maxBytes)
      throw new Error("Input type or size exceeds limit");
    const buffer = Buffer.alloc(stat.size + 1);
    const count = readFileBounded(fd, buffer);
    if (count !== stat.size)
      throw new Error("Input size changed while reading");
    return buffer.subarray(0, count);
  } finally {
    closeSync(fd);
  }
}
export function readJson(file: string, maxBytes = 16 * 1024 * 1024): unknown {
  return parseJson(
    new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(
      readBytes(file, maxBytes),
    ),
  );
}
import { readSync } from "node:fs";
function readFileBounded(fd: number, buffer: Buffer): number {
  let n = 0;
  while (n < buffer.length) {
    const read = readSync(fd, buffer, n, buffer.length - n, null);
    if (!read) break;
    n += read;
  }
  return n;
}
export function writeNew(file: string, value: unknown): void {
  const body =
    JSON.stringify(
      value,
      (_key, item: unknown) => {
        if (typeof item === "number" && !Number.isFinite(item))
          throw new Error("Non-finite output");
        return item;
      },
      2,
    ) + "\n";
  writeNewBytes(file, body);
}
export function writeNewBytes(file: string, body: string | Buffer): void {
  const temp = path.join(path.dirname(file), `.report-${randomUUID()}.tmp`);
  let fd: number | undefined;
  let created = false;
  const failures: unknown[] = [];
  try {
    fd = openSync(temp, "wx", 0o600);
    created = true;
    writeFileSync(fd, body);
    fsyncSync(fd);
    closeSync(fd);
    fd = undefined;
    linkSync(temp, file);
  } catch (error) {
    failures.push(error);
  }
  if (fd !== undefined) {
    try {
      closeSync(fd);
    } catch (error) {
      failures.push(error);
    }
  }
  if (created) {
    try {
      unlinkSync(temp);
    } catch (error) {
      failures.push(error);
    }
  }
  if (failures.length)
    throw new AggregateError(failures, "Report publication or cleanup failed");
}
export function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Expected an object");
  return value as Record<string, unknown>;
}
