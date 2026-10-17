import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Snapshot } from "./generated/Snapshot";
export type { Snapshot } from "./generated/Snapshot";

type Obj = Record<string, unknown>;
function object(value: unknown, keys: string[]): value is Obj {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => key in value)
  );
}
function text(value: unknown, max = 4096): value is string {
  return (
    typeof value === "string" &&
    value.length <= max &&
    new TextEncoder().encode(value).byteLength <= max
  );
}
function number(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}
function optionalText(value: unknown): boolean {
  return value === null || text(value);
}
function oneOf(value: unknown, options: string[]): boolean {
  return typeof value === "string" && options.includes(value);
}
const setupPhases = [
  "Missing",
  "Checking",
  "Downloading",
  "Loading",
  "Cancelling",
  "Cancelled",
  "Ready",
  "Failed",
];
const sessionPhases = [
  "Idle",
  "Starting",
  "Recording",
  "Finalizing",
  "Delivering",
  "Cancelling",
];
const permissions = [
  "granted",
  "denied",
  "notDetermined",
  "restricted",
  "unavailable",
];
export function decodeSnapshot(value: unknown): Snapshot {
  if (
    !object(value, [
      "revision",
      "running",
      "setup",
      "session",
      "permissions",
      "labels",
      "settingsBusy",
      "settingsRequest",
      "error",
    ])
  )
    throw new Error("Ungültiger Anwendungsstatus");
  const { setup, session, labels, permissions: access } = value;
  if (
    !number(value.revision) ||
    typeof value.running !== "boolean" ||
    typeof value.settingsBusy !== "boolean" ||
    !number(value.settingsRequest) ||
    value.settingsRequest > 4_294_967_295 ||
    !optionalText(value.error) ||
    !object(setup, [
      "phase",
      "model",
      "downloadedBytes",
      "totalBytes",
      "canCancel",
      "error",
    ]) ||
    !oneOf(setup.phase, setupPhases) ||
    !number(setup.downloadedBytes) ||
    !number(setup.totalBytes) ||
    setup.downloadedBytes > setup.totalBytes ||
    typeof setup.canCancel !== "boolean" ||
    !optionalText(setup.error) ||
    !object(setup.model, ["name", "downloadBytes", "sourceUrl"]) ||
    !text(setup.model.name) ||
    !number(setup.model.downloadBytes) ||
    !text(setup.model.sourceUrl) ||
    !setup.model.sourceUrl.startsWith(
      "https://huggingface.co/ggerganov/whisper.cpp/resolve/",
    ) ||
    !object(session, ["phase", "id", "text", "outcome", "maximumWarning"]) ||
    !oneOf(session.phase, sessionPhases) ||
    !(session.id === null || (number(session.id) && session.id > 0)) ||
    !text(session.text, 262144) ||
    typeof session.maximumWarning !== "boolean" ||
    !object(labels, ["device", "language", "hotkey"]) ||
    !text(labels.device) ||
    !text(labels.language) ||
    !text(labels.hotkey) ||
    !object(access, ["microphone", "accessibility"]) ||
    !oneOf(access.microphone, permissions) ||
    !oneOf(access.accessibility, permissions)
  )
    throw new Error("Ungültiger Anwendungsstatus");
  if (session.outcome !== null) {
    const outcome = session.outcome;
    if (
      !object(outcome, ["kind"]) ||
      !oneOf(outcome.kind, ["local", "empty", "sent", "cancelled"])
    ) {
      if (
        !object(outcome, ["kind", "message"]) ||
        !oneOf(outcome.kind, ["held", "failed"]) ||
        !text(outcome.message)
      )
        throw new Error("Ungültiges Diktatergebnis");
    }
  }
  return value as unknown as Snapshot;
}
export async function connect(
  receive: (value: Snapshot) => void,
  onError: (error: unknown) => void = () => {},
): Promise<() => void> {
  let revision = -1;
  let active = true;
  const accept = (input: unknown) => {
    if (!active) return;
    const value = decodeSnapshot(input);
    if (active && value.revision > revision) {
      revision = value.revision;
      receive(value);
    }
  };
  const unlisten = await listen<unknown>("dictation:snapshot", (event) => {
    if (!active) return;
    try {
      accept(event.payload);
    } catch (error) {
      onError(error);
    }
  });
  try {
    accept(await invoke<unknown>("app_snapshot"));
  } catch (error) {
    active = false;
    unlisten();
    throw error;
  }
  return () => {
    active = false;
    unlisten();
  };
}
export function command(
  name:
    | "model_download"
    | "model_cancel"
    | "recording_start"
    | "transcript_clear"
    | "transcript_copy"
    | "microphone_request"
    | "accessibility_request",
): Promise<void> {
  return invoke(name);
}
export function sessionCommand(
  name: "recording_stop" | "recording_cancel",
  session: number,
): Promise<void> {
  if (!Number.isSafeInteger(session) || session <= 0)
    return Promise.reject(new Error("Ungültige Sitzungskennung"));
  return invoke(name, { session });
}
