import net from "node:net";
import { lstatSync, realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const OPS = ["status", "start", "stop", "cancel"];
const PHASES = [
  "Idle",
  "Starting",
  "Recording",
  "Finalizing",
  "Delivering",
  "Cancelling",
];
const OUTCOMES = ["local", "empty", "sent", "cancelled", "held", "failed"];
const MAX_REPLY = 16384;
const positive = (value) => Number.isSafeInteger(value) && value > 0;
function exact(value, keys) {
  return (
    value &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.hasOwn(value, key))
  );
}
function protocol() {
  return new Error("Invalid test-control protocol reply");
}
function parseReply(bytes, op) {
  let value;
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    if (!text.endsWith("\n") || text.slice(0, -1).includes("\n"))
      throw protocol();
    value = JSON.parse(text);
  } catch {
    throw protocol();
  }
  if (
    !exact(value, ["schemaVersion", "ok", "op", "snapshot", "error"]) ||
    value.schemaVersion !== 1 ||
    typeof value.ok !== "boolean" ||
    value.op !== op ||
    !(
      value.error === null ||
      (typeof value.error === "string" && value.error.length <= 4096)
    ) ||
    (value.ok
      ? value.error !== null || value.snapshot === null
      : value.error === null)
  )
    throw protocol();
  const state = value.snapshot;
  if (state !== null) {
    if (
      !exact(state, [
        "setupReady",
        "microphoneGranted",
        "accessibilityGranted",
        "phase",
        "sessionId",
        "outcome",
        "publicSentenceMatches",
        "transcriptCharacters",
        "actionError",
        "controlledSessionId",
        "controlledStartPending",
      ]) ||
      ![
        state.setupReady,
        state.microphoneGranted,
        state.accessibilityGranted,
        state.publicSentenceMatches,
        state.actionError,
        state.controlledStartPending,
      ].every((field) => typeof field === "boolean") ||
      !(
        state.controlledSessionId === null ||
        positive(state.controlledSessionId)
      ) ||
      !PHASES.includes(state.phase) ||
      !(state.sessionId === null || positive(state.sessionId)) ||
      !(state.outcome === null || OUTCOMES.includes(state.outcome)) ||
      !Number.isSafeInteger(state.transcriptCharacters) ||
      state.transcriptCharacters < 0 ||
      state.transcriptCharacters > 262144
    )
      throw protocol();
  }
  // Only the explicitly validated status is returned. Server error strings are
  // never echoed: they may originate from lower-level/native operations.
  return {
    schemaVersion: 1,
    ok: value.ok,
    op,
    snapshot: state,
    error: value.ok ? null : "Control command rejected",
  };
}

export async function requestControl({
  controlDir,
  op,
  session,
  timeoutMs = 3000,
} = {}) {
  if (
    !OPS.includes(op) ||
    (["stop", "cancel"].includes(op)
      ? !positive(session)
      : session !== undefined)
  )
    throw new Error(
      "Expected status/start, or stop/cancel with a positive safe session ID",
    );
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 10 || timeoutMs > 30000)
    throw new Error("Timeout must be 10..30000 milliseconds");
  if (typeof controlDir !== "string" || !path.isAbsolute(controlDir))
    throw new Error("An explicit absolute control directory is required");
  const directory = lstatSync(controlDir);
  if (
    !directory.isDirectory() ||
    directory.isSymbolicLink() ||
    realpathSync(controlDir) !== controlDir ||
    directory.uid !== process.getuid() ||
    (directory.mode & 0o077) !== 0
  )
    throw new Error("Expected a private owned canonical control directory");
  const socketPath = path.join(controlDir, "control.sock");
  const socketInfo = lstatSync(socketPath);
  if (
    !socketInfo.isSocket() ||
    socketInfo.uid !== process.getuid() ||
    (socketInfo.mode & 0o077) !== 0
  )
    throw new Error("Expected a private owned Unix control socket");
  const request = `${JSON.stringify({ schemaVersion: 1, op, ...(session === undefined ? {} : { session }) })}\n`;
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    let bytes = Buffer.alloc(0);
    let settled = false;
    const finish = (error, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.destroy();
      if (error) reject(error);
      else resolve(value);
    };
    const timer = setTimeout(
      () =>
        finish(
          new Error(
            "Test-control timeout; command outcome unknown, not retried",
          ),
        ),
      timeoutMs,
    );
    socket.once("connect", () => socket.write(request));
    socket.on("data", (chunk) => {
      if (bytes.length + chunk.length > MAX_REPLY) {
        finish(protocol());
        return;
      }
      bytes = Buffer.concat([bytes, chunk]);
    });
    socket.once("end", () => {
      if (bytes.length === 0) {
        finish(
          new Error(
            "Test-control closed without reply; command outcome unknown",
          ),
        );
        return;
      }
      try {
        finish(null, parseReply(bytes, op));
      } catch (error) {
        finish(error);
      }
    });
    socket.once("error", () =>
      finish(
        new Error("Test-control connection failed; command outcome unknown"),
      ),
    );
    socket.once("close", () => {
      if (!settled)
        finish(
          new Error(
            "Test-control closed without reply; command outcome unknown",
          ),
        );
    });
  });
}

export async function main(args, env = process.env) {
  let controlDir = env.EVERLAST_TEST_CONTROL_DIR;
  let timeoutMs = 3000;
  const positionals = [];
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--control-dir" || arg === "--timeout-ms") {
      const value = args[++index];
      if (!value) throw new Error("Missing option value");
      if (arg === "--control-dir") controlDir = value;
      else if (/^\d+$/.test(value)) timeoutMs = Number(value);
      else throw new Error("Invalid timeout");
    } else if (arg.startsWith("--")) throw new Error("Unknown option");
    else positionals.push(arg);
  }
  const [op, rawSession] = positionals;
  if (
    positionals.length > 2 ||
    (rawSession !== undefined && !/^[1-9]\d*$/.test(rawSession))
  )
    throw new Error(
      "Usage: test-control.mjs [--control-dir ABS] status|start|stop SID|cancel SID [--timeout-ms N]",
    );
  return requestControl({
    controlDir,
    op,
    session: rawSession === undefined ? undefined : Number(rawSession),
    timeoutMs,
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const result = await main(process.argv.slice(2));
    console.log(JSON.stringify(result));
    if (!result.ok) process.exitCode = 1;
  } catch {
    console.error(
      "Test-control failed; no automatic retry. Check explicit directory, command, session and current status.",
    );
    process.exitCode = 1;
  }
}
