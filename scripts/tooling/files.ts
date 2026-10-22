import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
  linkSync,
} from "node:fs";
import { open, stat } from "node:fs/promises";
import path from "node:path";
import { Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import { createGunzip } from "node:zlib";
import * as tar from "tar-stream";
import { check } from "./audio-data.ts";

export const sha256 = (data: Uint8Array | string): string =>
  createHash("sha256").update(data).digest("hex");
export function canonicalJson(value: unknown): string {
  const sorted = (v: unknown): unknown =>
    Array.isArray(v)
      ? v.map(sorted)
      : v && typeof v === "object"
        ? Object.fromEntries(
            Object.entries(v)
              .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
              .map(([k, x]) => [k, sorted(x)]),
          )
        : v;
  return JSON.stringify(sorted(value), null, 2) + "\n";
}
export function publishIdentical(file: string, data: Buffer | string): void {
  const bytes = Buffer.isBuffer(data) ? data : Buffer.from(data);
  mkdirSync(path.dirname(file), { recursive: true });
  check(
    !lstatSync(path.dirname(file)).isSymbolicLink(),
    "Output directory is redirected",
  );
  const verify = () => {
    const st = lstatSync(file);
    check(
      st.isFile() &&
        !st.isSymbolicLink() &&
        st.size === bytes.length &&
        readFileSync(file).equals(bytes),
      "Frozen output differs",
    );
  };
  try {
    lstatSync(file);
    verify();
    return;
  } catch (e) {
    if (!(e instanceof Error && "code" in e && e.code === "ENOENT")) throw e;
  }
  const temp = path.join(path.dirname(file), `.frozen-${randomUUID()}`);
  let fd: number | undefined,
    created = false;
  try {
    fd = openSync(temp, "wx", 0o600);
    created = true;
    writeFileSync(fd, bytes);
    fsyncSync(fd);
    closeSync(fd);
    fd = undefined;
    try {
      linkSync(temp, file);
    } catch (e) {
      if (e instanceof Error && "code" in e && e.code === "EEXIST") verify();
      else throw e;
    }
  } finally {
    if (fd !== undefined) closeSync(fd);
    if (created) unlinkSync(temp);
  }
}
export async function hashFile(
  file: string,
): Promise<{ bytes: number; sha256: string }> {
  const source = await open(file, "r");
  try {
    const h = createHash("sha256");
    let bytes = 0;
    for await (const chunk of source.createReadStream({
      autoClose: false,
      start: 0,
    })) {
      h.update(chunk);
      bytes += chunk.length;
    }
    return { bytes, sha256: h.digest("hex") };
  } finally {
    await source.close();
  }
}
export async function verifyFile(
  file: string,
  bytes: number,
  digest: string,
): Promise<void> {
  const got = await hashFile(file);
  check(
    got.bytes === bytes && got.sha256 === digest,
    "Pinned source size/hash differs",
  );
}
export interface ArchiveMember {
  name: string;
  type: string;
  size: number;
  data: Buffer;
}
export async function readArchive(
  file: string,
  expected: { bytes: number; sha256: string },
  select: (name: string) => boolean,
  maxMemberBytes = 20 * 1024 * 1024,
  maxEntries = 12000,
  consume?: (member: ArchiveMember) => void,
): Promise<ArchiveMember[]> {
  const handle = await open(file, "r");
  try {
    const initial = await handle.stat();
    check(
      initial.isFile() && initial.size === expected.bytes,
      "Archive size/type differs",
    );
    const extractor = tar.extract();
    const result: ArchiveMember[] = [];
    let count = 0,
      consumed = 0,
      retained = 0,
      expanded = 0;
    const hash = createHash("sha256");
    extractor.on("entry", (header, stream, next) => {
      stream.on("error", (error) => extractor.destroy(error));
      if (++count > maxEntries) {
        extractor.destroy(new Error("Archive entry count exceeded"));
        return;
      }
      let wanted: boolean;
      try {
        wanted = select(header.name);
      } catch (error) {
        extractor.destroy(
          error instanceof Error ? error : new Error("Invalid selection"),
        );
        return;
      }
      const size = header.size ?? 0;
      expanded += size;
      if (
        !Number.isSafeInteger(expanded) ||
        expanded > 2 * 1024 * 1024 * 1024
      ) {
        extractor.destroy(new Error("Expanded archive exceeds bound"));
        return;
      }
      const chunks: Buffer[] = [];
      let bytes = 0;
      if (
        wanted &&
        ((header.type !== "file" && header.type !== "directory") ||
          !Number.isSafeInteger(size) ||
          size < 0 ||
          size > maxMemberBytes ||
          Object.keys(header.pax ?? {}).some((key) =>
            key.toLowerCase().includes("sparse"),
          ))
      ) {
        extractor.destroy(new Error("Archive member type/size invalid"));
        return;
      }
      if (wanted && !consume) {
        retained += size;
        if (retained > 512 * 1024 * 1024) {
          extractor.destroy(new Error("Archive selection too large"));
          return;
        }
      }
      stream.on("data", (data: unknown) => {
        if (!Buffer.isBuffer(data)) {
          extractor.destroy(new Error("Invalid archive chunk"));
          return;
        }
        if (wanted) {
          bytes += data.length;
          if (bytes > maxMemberBytes)
            extractor.destroy(new Error("Archive member too large"));
          else chunks.push(data);
        }
      });
      stream.on("end", () => {
        if (wanted) {
          if (bytes !== size) {
            extractor.destroy(new Error("Truncated archive member"));
            return;
          }
          const member = {
            name: header.name,
            type: header.type ?? "file",
            size,
            data: Buffer.concat(chunks),
          };
          try {
            if (consume) consume(member);
            else result.push(member);
          } catch (error) {
            extractor.destroy(
              error instanceof Error ? error : new Error("Invalid member"),
            );
            return;
          }
        }
        next();
      });
      stream.resume();
    });
    const hashed = new Transform({
      transform(chunk: Buffer, _encoding, done) {
        consumed += chunk.length;
        if (consumed > expected.bytes) {
          done(new Error("Archive grew while reading"));
          return;
        }
        hash.update(chunk);
        done(null, chunk);
      },
    });
    await pipeline(
      handle.createReadStream({ autoClose: false, start: 0 }),
      hashed,
      createGunzip(),
      extractor,
    );
    const now = await stat(file);
    check(
      now.dev === initial.dev && now.ino === initial.ino,
      "Archive path changed",
    );
    check(
      consumed === expected.bytes && hash.digest("hex") === expected.sha256,
      "Archive SHA256 differs",
    );
    return result;
  } finally {
    await handle.close();
  }
}
/** Preflight all destinations before publishing any member of a frozen corpus. */
export function verifyExisting(file: string, data: Buffer | string): void {
  const bytes = Buffer.isBuffer(data) ? data : Buffer.from(data);
  try {
    const st = lstatSync(file);
    check(
      st.isFile() &&
        !st.isSymbolicLink() &&
        st.size === bytes.length &&
        readFileSync(file).equals(bytes),
      "Frozen output differs",
    );
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT"))
      throw error;
  }
}
