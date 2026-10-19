import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  realpathSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import path from "node:path";
import os from "node:os";
import {
  PUBLIC_AUDIO_PATH,
  PUBLIC_AUDIO_SHA256,
  validatePublicAudio,
} from "./single-press-playback.mjs";

function modified(change, action) {
  const directory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-audio-")),
  );
  try {
    const bytes = readFileSync(PUBLIC_AUDIO_PATH);
    change(bytes);
    const filename = path.join(directory, "public.wav");
    writeFileSync(filename, bytes, { mode: 0o600 });
    action(filename);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("actual fixture has exact pinned bytes, PCM format and 300-second sample count", () => {
  const result = validatePublicAudio();
  assert.equal(result.sha256, PUBLIC_AUDIO_SHA256);
  assert.equal(result.frames, 4800000);
  assert.equal(result.durationMillis, 300000);
});

test("wrong sample count is rejected as duration before hash checking", () => {
  modified(
    (bytes) => {
      // Preserve RIFF size while turning two data bytes into a valid padded JUNK chunk.
      bytes.writeUInt32LE(9599990, 40);
      bytes.write("JUNK", bytes.length - 10, "ascii");
      bytes.writeUInt32LE(2, bytes.length - 6);
    },
    (filename) =>
      assert.throws(
        () => validatePublicAudio(filename),
        /audio-duration-invalid/,
      ),
  );
});

async function playerFixture({
  spawnError = false,
  ignoresTerm = false,
  ignoresKill = false,
} = {}) {
  const { EventEmitter } = await import("node:events");
  let elapsed = 0;
  const calls = [];
  const child = new EventEmitter();
  child.pid = 987654321;
  child.exitCode = null;
  child.signalCode = null;
  child.finish = (code = 0, signal = null) => {
    child.exitCode = code;
    child.signalCode = signal;
    child.emit("exit", code, signal);
    child.emit("close", code, signal);
  };
  child.kill = (signal) => {
    calls.push(signal);
    if (
      (signal === "SIGTERM" && !ignoresTerm) ||
      (signal === "SIGKILL" && !ignoresKill)
    )
      child.finish(null, signal);
    return true;
  };
  const spawnPlayer = (program, args, options) => {
    calls.push({ program, args, options });
    queueMicrotask(() => {
      if (spawnError) {
        child.emit("error", new Error("controlled failure"));
        child.finish(null);
      } else child.emit("spawn");
    });
    return child;
  };
  return {
    child,
    calls,
    spawnPlayer,
    clock: {
      now: () => elapsed,
      sleep: async (ms) => {
        elapsed += ms;
      },
    },
    advance: (ms) => {
      elapsed += ms;
    },
  };
}
const frozen = {
  fixtureSha256: PUBLIC_AUDIO_SHA256,
  fixtureDurationMillis: 300000,
};

test("start returns after owned spawn, and successful completion makes stop idempotent", async () => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture();
  const audio = createPlaybackAdapter({
    spawnPlayer: f.spawnPlayer,
    clock: f.clock,
  });
  await audio.start(frozen);
  assert.equal(f.calls[0].program, "/usr/bin/afplay");
  assert.deepEqual(f.calls[0].args, [PUBLIC_AUDIO_PATH]);
  assert.equal(f.calls[0].options.shell, false);
  f.advance(300000);
  f.child.finish(0);
  assert.equal((await audio.stop()).completed, true);
  assert.equal((await audio.stop()).completed, true);
  assert.equal(f.calls.filter((value) => typeof value === "string").length, 0);
});

test("codec, channel, rate, RIFF/chunk bounds and payload changes are rejected", () => {
  for (const [change, reason] of [
    [(bytes) => bytes.writeUInt16LE(3, 20), /audio-format-invalid/],
    [(bytes) => bytes.writeUInt16LE(2, 22), /audio-format-invalid/],
    [(bytes) => bytes.writeUInt32LE(48000, 24), /audio-format-invalid/],
    [(bytes) => bytes.writeUInt32LE(0, 4), /audio-riff-invalid/],
    [(bytes) => bytes.writeUInt32LE(0xffffffff, 40), /audio-chunk-invalid/],
    [
      (bytes) => {
        bytes[bytes.length - 1] ^= 1;
      },
      /audio-hash-mismatch/,
    ],
  ])
    modified(change, (filename) =>
      assert.throws(() => validatePublicAudio(filename), reason),
    );
});

test("symlink and oversized fixture are rejected before player admission", async () => {
  const { symlinkSync, openSync, ftruncateSync, closeSync } =
    await import("node:fs");
  const directory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-audio-guard-")),
  );
  try {
    const link = path.join(directory, "link.wav");
    symlinkSync(PUBLIC_AUDIO_PATH, link);
    assert.throws(() => validatePublicAudio(link));
    const big = path.join(directory, "big.wav");
    const fd = openSync(big, "wx", 0o600);
    ftruncateSync(fd, 32 * 1024 * 1024 + 1);
    closeSync(fd);
    assert.throws(() => validatePublicAudio(big), /audio-file-invalid/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("spawn failure, nonzero exit and early exit zero remain failures at stop", async () => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  for (const mode of ["spawn", "nonzero", "early-zero"]) {
    const f = await playerFixture({ spawnError: mode === "spawn" });
    const audio = createPlaybackAdapter({
      spawnPlayer: f.spawnPlayer,
      clock: f.clock,
    });
    if (mode === "spawn")
      await assert.rejects(audio.start(frozen), /playback-player-error/);
    else {
      await audio.start(frozen);
      f.advance(mode === "nonzero" ? 300000 : 1000);
      f.child.finish(mode === "nonzero" ? 7 : 0);
    }
    await assert.rejects(audio.stop(), /playback-/);
    await assert.rejects(audio.stop(), /playback-/);
    assert.equal(audio.status().closed, true);
  }
});

test("early stop terminates only owned child, escalates boundedly, and cannot pass", async () => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture({ ignoresTerm: true });
  const audio = createPlaybackAdapter({
    spawnPlayer: f.spawnPlayer,
    clock: f.clock,
  });
  await audio.start(frozen);
  const before = f.clock.now();
  await assert.rejects(
    audio.stop(),
    (error) =>
      error.code === "playback-incomplete" && error.cleanupConfirmed === true,
  );
  assert.deepEqual(
    f.calls.filter((value) => typeof value === "string"),
    ["SIGTERM", "SIGKILL"],
  );
  assert.ok(f.clock.now() - before <= 2000);
  await assert.rejects(audio.stop());
  assert.equal(f.calls.filter((value) => value === "SIGKILL").length, 1);
});

test("unconfirmed termination never reports successful cleanup", async () => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture({ ignoresTerm: true, ignoresKill: true });
  const audio = createPlaybackAdapter({
    spawnPlayer: f.spawnPlayer,
    clock: f.clock,
  });
  await audio.start(frozen);
  await assert.rejects(
    audio.stop(),
    (error) =>
      error.code === "playback-termination-unconfirmed" &&
      error.cleanupConfirmed === false,
  );
  assert.equal(audio.status().closed, false);
});

test("frozen mismatch and repeated start never spawn a second child", async () => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture();
  const audio = createPlaybackAdapter({
    spawnPlayer: f.spawnPlayer,
    clock: f.clock,
  });
  await assert.rejects(
    audio.start({ ...frozen, fixtureDurationMillis: 1 }),
    /playback-frozen-mismatch/,
  );
  assert.equal(f.calls.length, 0);
  await audio.start(frozen);
  await assert.rejects(audio.start(frozen), /playback-already-attempted/);
  await assert.rejects(audio.stop());
  assert.equal(f.calls.filter((value) => typeof value === "object").length, 1);
});

test(
  "benign owned Node child is actually terminated and reaped; afplay is never launched",
  { timeout: 8000 },
  async () => {
    const { spawn } = await import("node:child_process");
    const { createPlaybackAdapter } =
      await import("./single-press-playback.mjs");
    let owned;
    const audio = createPlaybackAdapter({
      spawnPlayer(program, args, options) {
        assert.equal(program, "/usr/bin/afplay");
        assert.deepEqual(args, [PUBLIC_AUDIO_PATH]);
        owned = spawn(
          process.execPath,
          ["-e", "setInterval(() => {}, 1000)"],
          options,
        );
        return owned;
      },
    });
    try {
      await audio.start(frozen);
      await assert.rejects(
        audio.stop(),
        (error) =>
          error.code === "playback-incomplete" &&
          error.cleanupConfirmed === true,
      );
      assert.equal(audio.status().closed, true);
      assert.ok(owned.exitCode !== null || owned.signalCode !== null);
    } finally {
      if (owned && owned.exitCode === null && owned.signalCode === null)
        owned.kill("SIGKILL");
    }
  },
);
