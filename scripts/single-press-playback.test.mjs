import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  existsSync,
  lstatSync,
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
  inspectPcm16Audio,
} from "./single-press-playback.mjs";

// Synthetic four-frame PCM: structural fixture, never a public speech acceptance proof.
const SYNTHETIC_WAV_HEX =
  "524946462c00000057415645666d74201000000001000100803e0000007d000002001000646174610800000000000100ffffff7f";
const SYNTHETIC_SHA256 =
  "fbf8615769d3c5bb7f1ac9b85e636ee6862f8b5409e511466cf3e12a168d9839";
function syntheticWav() {
  return Buffer.from(SYNTHETIC_WAV_HEX, "hex");
}
function controlledInspection(t) {
  const directory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-player-")),
  );
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const fixturePath = path.join(directory, "controlled.wav");
  writeFileSync(fixturePath, syntheticWav(), { mode: 0o600 });
  return {
    fixturePath,
    inspectFixture(filename) {
      assert.equal(filename, fixturePath);
      const info = lstatSync(filename);
      // Controlled public-inspection result isolates child lifecycle from the real asset.
      return {
        path: filename,
        sha256: PUBLIC_AUDIO_SHA256,
        frames: 4800000,
        durationMillis: 300000,
        identity: {
          dev: info.dev,
          ino: info.ino,
          size: info.size,
          mtimeMs: info.mtimeMs,
        },
      };
    },
  };
}
function modified(change, action, fullDuration = false) {
  const directory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "one-press-audio-")),
  );
  try {
    let bytes = syntheticWav();
    if (fullDuration) {
      bytes = Buffer.alloc(9600044);
      syntheticWav().copy(bytes, 0, 0, 44);
      bytes.writeUInt32LE(bytes.length - 8, 4);
      bytes.writeUInt32LE(9600000, 40);
    }
    change(bytes);
    const filename = path.join(directory, "synthetic.wav");
    writeFileSync(filename, bytes, { mode: 0o600 });
    action(filename);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("synthetic PCM parser checks known literal bytes without authenticating public speech", () => {
  assert.deepEqual(inspectPcm16Audio(syntheticWav()), {
    sha256: SYNTHETIC_SHA256,
    frames: 4,
    durationMillis: 0.25,
  });
  const changed = syntheticWav();
  changed[changed.length - 1] ^= 1;
  assert.notEqual(inspectPcm16Audio(changed).sha256, SYNTHETIC_SHA256);
});
test(
  "actual public fixture has pinned bytes, PCM format and 300-second sample count",
  {
    skip: existsSync(PUBLIC_AUDIO_PATH)
      ? false
      : "ignored public speech fixture unavailable; structural and lifecycle regressions remain active",
  },
  () => {
    const result = validatePublicAudio();
    assert.equal(result.sha256, PUBLIC_AUDIO_SHA256);
    assert.equal(result.frames, 4800000);
    assert.equal(result.durationMillis, 300000);
  },
);
test("wrong sample count is rejected as duration before public hash checking", () => {
  modified(
    () => {},
    (filename) =>
      assert.throws(
        () => validatePublicAudio(filename),
        /audio-duration-invalid/,
      ),
  );
});
test("production validator rejects synthetic 300-second audio despite valid PCM shape", () => {
  modified(
    () => {},
    (filename) =>
      assert.throws(() => validatePublicAudio(filename), /audio-hash-mismatch/),
    true,
  );
});

async function playerFixture(
  t,
  { spawnError = false, ignoresTerm = false, ignoresKill = false } = {},
) {
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
    ...controlledInspection(t),
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

test("start returns after owned spawn, and successful completion makes stop idempotent", async (t) => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture(t);
  const audio = createPlaybackAdapter({
    fixturePath: f.fixturePath,
    inspectFixture: f.inspectFixture,
    spawnPlayer: f.spawnPlayer,
    clock: f.clock,
  });
  await audio.start(frozen);
  assert.equal(f.calls[0].program, "/usr/bin/afplay");
  assert.deepEqual(f.calls[0].args, [f.fixturePath]);
  assert.equal(f.calls[0].options.shell, false);
  f.advance(300000);
  f.child.finish(0);
  assert.equal((await audio.stop()).completed, true);
  assert.equal((await audio.stop()).completed, true);
  assert.equal(f.calls.filter((value) => typeof value === "string").length, 0);
});

test("codec, channel, rate and RIFF/chunk bounds are rejected independently of fixture availability", () => {
  for (const [change, reason] of [
    [(bytes) => bytes.writeUInt16LE(3, 20), /audio-format-invalid/],
    [(bytes) => bytes.writeUInt16LE(2, 22), /audio-format-invalid/],
    [(bytes) => bytes.writeUInt32LE(48000, 24), /audio-format-invalid/],
    [(bytes) => bytes.writeUInt32LE(0, 4), /audio-riff-invalid/],
    [(bytes) => bytes.writeUInt32LE(0xffffffff, 40), /audio-chunk-invalid/],
  ])
    modified(change, (filename) =>
      assert.throws(() => inspectPcm16Audio(readFileSync(filename)), reason),
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
    const target = path.join(directory, "synthetic.wav");
    writeFileSync(target, syntheticWav(), { mode: 0o600 });
    symlinkSync(target, link);
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

test("spawn failure, nonzero exit and early exit zero remain failures at stop", async (t) => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  for (const mode of ["spawn", "nonzero", "early-zero"]) {
    const f = await playerFixture(t, { spawnError: mode === "spawn" });
    const audio = createPlaybackAdapter({
      fixturePath: f.fixturePath,
      inspectFixture: f.inspectFixture,
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

test("early stop terminates only owned child, escalates boundedly, and cannot pass", async (t) => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture(t, { ignoresTerm: true });
  const audio = createPlaybackAdapter({
    fixturePath: f.fixturePath,
    inspectFixture: f.inspectFixture,
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

test("unconfirmed termination never reports successful cleanup", async (t) => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture(t, { ignoresTerm: true, ignoresKill: true });
  const audio = createPlaybackAdapter({
    fixturePath: f.fixturePath,
    inspectFixture: f.inspectFixture,
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

test("frozen mismatch and repeated start never spawn a second child", async (t) => {
  const { createPlaybackAdapter } = await import("./single-press-playback.mjs");
  const f = await playerFixture(t);
  const audio = createPlaybackAdapter({
    fixturePath: f.fixturePath,
    inspectFixture: f.inspectFixture,
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
  async (t) => {
    const { spawn } = await import("node:child_process");
    const { createPlaybackAdapter } =
      await import("./single-press-playback.mjs");
    let owned;
    const fixture = controlledInspection(t);
    const audio = createPlaybackAdapter({
      ...fixture,
      spawnPlayer(program, args, options) {
        assert.equal(program, "/usr/bin/afplay");
        assert.deepEqual(args, [fixture.fixturePath]);
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
