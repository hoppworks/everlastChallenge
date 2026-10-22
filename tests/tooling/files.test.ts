import assert from "node:assert/strict";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import * as tar from "tar-stream";
import {
  publishIdentical,
  readArchive,
  sha256,
  verifyExisting,
} from "../../scripts/tooling/files.ts";

async function archive(type: tar.Header["type"] = "file"): Promise<Buffer> {
  const pack = tar.pack();
  pack.entry(
    {
      name: "test/clip.wav",
      type,
      ...(type === "symlink" ? { linkname: "/private/target" } : {}),
    },
    type === "file" ? Buffer.from("wave") : Buffer.alloc(0),
  );
  pack.finalize();
  const chunks: Buffer[] = [];
  for await (const chunk of pack) {
    assert.ok(Buffer.isBuffer(chunk));
    chunks.push(chunk);
  }
  return gzipSync(Buffer.concat(chunks));
}

await test("archive selection verifies the complete compressed hash and rejects selected links", async () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "archive-test-"));
  try {
    const file = path.join(dir, "source.tar.gz");
    const data = await archive();
    writeFileSync(file, data);
    const pin = { bytes: data.length, sha256: sha256(data) };
    assert.equal(
      (await readArchive(file, pin, () => true))[0]?.data.toString(),
      "wave",
    );
    await assert.rejects(
      readArchive(file, { ...pin, sha256: "0".repeat(64) }, () => true),
      /SHA256/,
    );
    await assert.rejects(
      readArchive(file, pin, () => true, 3),
      /type\/size/,
    );
    const link = await archive("symlink");
    writeFileSync(file, link);
    await assert.rejects(
      readArchive(
        file,
        { bytes: link.length, sha256: sha256(link) },
        () => true,
      ),
      /type\/size/,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

await test("frozen outputs preserve identical files and reject changed or redirected destinations", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "frozen-test-"));
  try {
    const file = path.join(dir, "result");
    publishIdentical(file, "ok");
    publishIdentical(file, "ok");
    assert.throws(() => publishIdentical(file, "new"), /differs/);
    const link = path.join(dir, "link");
    symlinkSync(file, link);
    assert.throws(() => verifyExisting(link, "ok"), /differs/);
    assert.equal(readFileSync(file, "utf8"), "ok");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
