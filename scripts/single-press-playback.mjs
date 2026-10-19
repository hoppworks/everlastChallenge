import {
  constants,
  closeSync,
  fstatSync,
  lstatSync,
  openSync,
  readSync,
  realpathSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const PUBLIC_AUDIO_PATH = path.join(
  root,
  "artifacts",
  "single-press-audio-v1",
  "public-speech-300s.wav",
);
export const PUBLIC_AUDIO_SHA256 =
  "da19d0d22bf6f26f1cbdba612da4b2af73372524ed2e88d1b1a34b76b8439691";
const MAX_BYTES = 32 * 1024 * 1024;
function reject(code) {
  throw Object.assign(new Error(code), { code });
}
function identity(info) {
  return {
    dev: info.dev,
    ino: info.ino,
    size: info.size,
    mtimeMs: info.mtimeMs,
  };
}
function same(info, frozen) {
  return (
    info.isFile() &&
    info.uid === process.getuid() &&
    info.dev === frozen.dev &&
    info.ino === frozen.ino &&
    info.size === frozen.size &&
    info.mtimeMs === frozen.mtimeMs
  );
}

/** Structural PCM contract only; this does not authenticate the public speech fixture. */
export function inspectPcm16Audio(input) {
  if (!Buffer.isBuffer(input) || input.length < 44 || input.length > MAX_BYTES)
    reject("audio-file-invalid");
  if (
    input.toString("ascii", 0, 4) !== "RIFF" ||
    input.toString("ascii", 8, 12) !== "WAVE" ||
    input.readUInt32LE(4) !== input.length - 8
  )
    reject("audio-riff-invalid");
  let format;
  let dataBytes;
  let chunks = 0;
  for (let offset = 12; offset < input.length; ) {
    if (++chunks > 1024) reject("audio-chunk-limit");
    if (offset + 8 > input.length) reject("audio-chunk-invalid");
    const tag = input.toString("ascii", offset, offset + 4);
    const size = input.readUInt32LE(offset + 4);
    const begin = offset + 8;
    const end = begin + size;
    if (end > input.length || end + (size % 2) > input.length)
      reject("audio-chunk-invalid");
    if (tag === "fmt ") {
      if (format || size !== 16) reject("audio-format-invalid");
      format = {
        codec: input.readUInt16LE(begin),
        channels: input.readUInt16LE(begin + 2),
        rate: input.readUInt32LE(begin + 4),
        byteRate: input.readUInt32LE(begin + 8),
        align: input.readUInt16LE(begin + 12),
        bits: input.readUInt16LE(begin + 14),
      };
    } else if (tag === "data") {
      if (dataBytes !== undefined) reject("audio-data-invalid");
      dataBytes = size;
    }
    offset = end + (size % 2);
  }
  if (
    !format ||
    format.codec !== 1 ||
    format.channels !== 1 ||
    format.rate !== 16000 ||
    format.byteRate !== 32000 ||
    format.align !== 2 ||
    format.bits !== 16 ||
    dataBytes === undefined ||
    dataBytes % 2 !== 0
  )
    reject("audio-format-invalid");
  const sha256 = createHash("sha256").update(input).digest("hex");
  return { sha256, frames: dataBytes / 2, durationMillis: dataBytes / 32 };
}

export function validatePublicAudio(filename = PUBLIC_AUDIO_PATH) {
  if (
    !path.isAbsolute(filename) ||
    path.normalize(filename) !== filename ||
    realpathSync(path.dirname(filename)) !== path.dirname(filename)
  )
    reject("audio-path-invalid");
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const info = fstatSync(fd);
    if (
      !info.isFile() ||
      info.uid !== process.getuid() ||
      info.size < 44 ||
      info.size > MAX_BYTES
    )
      reject("audio-file-invalid");
    const bytes = Buffer.alloc(info.size + 1);
    let used = 0;
    while (used < bytes.length) {
      const count = readSync(fd, bytes, used, bytes.length - used, used);
      if (!count) break;
      used += count;
    }
    if (used !== info.size || !same(fstatSync(fd), identity(info)))
      reject("audio-file-changed");
    const input = bytes.subarray(0, used);
    const { sha256, frames, durationMillis } = inspectPcm16Audio(input);
    if (frames !== 4800000) reject("audio-duration-invalid");
    if (sha256 !== PUBLIC_AUDIO_SHA256) reject("audio-hash-mismatch");
    return {
      path: filename,
      sha256,
      frames,
      durationMillis,
      identity: identity(info),
    };
  } finally {
    closeSync(fd);
  }
}

const defaultClock = {
  now: () => performance.now(),
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
};
const MIN_COMPLETION_MILLIS = 299000;
/** Only this fixed public fixture and owned child handle are used. No device/volume change. */
export function createPlaybackAdapter({
  fixturePath = PUBLIC_AUDIO_PATH,
  spawnPlayer = spawn,
  inspectFixture = validatePublicAudio,
  clock = defaultClock,
} = {}) {
  let state = null;
  const waitClosed = async (milliseconds) => {
    const deadline = clock.now() + milliseconds;
    while (!state.closed && clock.now() < deadline)
      await clock.sleep(Math.min(25, deadline - clock.now()));
  };
  const failed = (code) => {
    state.failure ??= code;
    return Object.assign(new Error(state.failure), {
      code: state.failure,
      cleanupConfirmed: state.closed,
    });
  };
  const stopOwned = async () => {
    // Give a near-complete 300-second playback time to finish normally.
    if (
      !state.closed &&
      !state.failure &&
      state.startedAt !== null &&
      clock.now() - state.startedAt >= MIN_COMPLETION_MILLIS
    )
      await waitClosed(1500);
    for (const [signal, grace] of [
      ["SIGTERM", 1000],
      ["SIGKILL", 1000],
    ]) {
      if (state.closed) break;
      state.failure ??= "playback-incomplete";
      if (!state.exited) {
        try {
          state.child.kill(signal);
        } catch {
          state.failure = "playback-termination-failed";
        }
      }
      await waitClosed(grace);
    }
    if (!state.closed) {
      state.failure = "playback-termination-unconfirmed";
      throw failed(state.failure);
    }
    if (state.failure) throw failed(state.failure);
    if (state.exitCode !== 0 || state.exitSignal !== null)
      throw failed("playback-exit-failed");
    if (!same(lstatSync(state.fixture.path), state.fixture.identity))
      throw failed("audio-file-changed");
    return { completed: true, cleanupConfirmed: true };
  };
  const stop = () => {
    if (!state)
      return Promise.resolve({
        completed: false,
        cleanupConfirmed: true,
        notStarted: true,
      });
    state.stopPromise ??= stopOwned();
    return state.stopPromise;
  };
  return {
    async start(frozen) {
      if (state) reject("playback-already-attempted");
      if (
        frozen?.fixtureSha256 !== PUBLIC_AUDIO_SHA256 ||
        frozen.fixtureDurationMillis !== 300000
      )
        reject("playback-frozen-mismatch");
      const fixture = inspectFixture(fixturePath);
      if (
        fixture.sha256 !== PUBLIC_AUDIO_SHA256 ||
        fixture.frames !== 4800000 ||
        fixture.durationMillis !== 300000
      )
        reject("audio-inspection-invalid");
      if (!same(lstatSync(fixture.path), fixture.identity))
        reject("audio-file-changed");
      state = {
        fixture,
        child: null,
        spawned: false,
        exited: false,
        closed: false,
        startedAt: null,
        exitCode: null,
        exitSignal: null,
        failure: null,
        stopPromise: null,
      };
      try {
        state.child = spawnPlayer("/usr/bin/afplay", [fixture.path], {
          shell: false,
          stdio: ["ignore", "ignore", "ignore"],
        });
      } catch {
        state.closed = true;
        throw failed("playback-spawn-failed");
      }
      state.child.once("spawn", () => {
        state.spawned = true;
        state.startedAt = clock.now();
      });
      state.child.once("error", () => {
        state.failure ??= "playback-player-error";
      });
      state.child.once("exit", () => {
        state.exited = true;
      });
      state.child.once("close", (code, signal) => {
        state.closed = true;
        state.exitCode = code;
        state.exitSignal = signal ?? null;
        if (code !== 0 || signal) state.failure ??= "playback-exit-failed";
        else if (
          state.startedAt === null ||
          clock.now() - state.startedAt < MIN_COMPLETION_MILLIS
        )
          state.failure ??= "playback-ended-too-early";
      });
      const deadline = clock.now() + 5000;
      while (
        !state.spawned &&
        !state.failure &&
        !state.closed &&
        clock.now() < deadline
      )
        await clock.sleep(10);
      if (
        !state.spawned ||
        state.failure ||
        state.closed ||
        !Number.isSafeInteger(state.child.pid) ||
        state.child.pid <= 0
      ) {
        state.failure ??= "playback-spawn-unconfirmed";
        try {
          await stop();
        } catch {
          /* The retained failure is returned below. */
        }
        throw failed(state.failure);
      }
      return { started: true, processId: state.child.pid };
    },
    stop,
    status() {
      return state
        ? {
            attempted: true,
            spawned: state.spawned,
            processId: state.child?.pid ?? null,
            closed: state.closed,
            failure: state.failure,
          }
        : {
            attempted: false,
            spawned: false,
            processId: null,
            closed: true,
            failure: null,
          };
    },
  };
}
