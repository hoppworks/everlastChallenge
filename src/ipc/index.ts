import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Snapshot } from "./generated/Snapshot";
export type { Snapshot } from "./generated/Snapshot";
import type { SettingsDraft } from "./generated/SettingsDraft";
export type { SettingsDraft } from "./generated/SettingsDraft";

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
function deviceName(value: unknown): value is string {
  return (
    text(value, 512) &&
    value.length > 0 &&
    value.trim() === value &&
    Array.from(value).every((character) => {
      const code = character.charCodeAt(0);
      return code >= 32 && (code < 127 || code > 159);
    })
  );
}
function draft(value: unknown): value is SettingsDraft {
  if (
    !object(value, [
      "device",
      "language",
      "hotkey",
      "refinement",
      ...(typeof value === "object" && value !== null && "model" in value
        ? ["model"]
        : []),
    ])
  )
    return false;
  if ("model" in value && !oneOf(value.model, ["mlx_reference_encoder"]))
    return false;
  const hotkey = value.hotkey;
  return (
    typeof value.refinement === "boolean" &&
    (value.device === null || deviceName(value.device)) &&
    oneOf(value.language, ["german", "english"]) &&
    object(hotkey, ["key", "option", "command", "control", "shift"]) &&
    typeof hotkey.key === "string" &&
    /^[A-Z]$/.test(hotkey.key) &&
    typeof hotkey.option === "boolean" &&
    typeof hotkey.command === "boolean" &&
    typeof hotkey.control === "boolean" &&
    typeof hotkey.shift === "boolean" &&
    (hotkey.option || hotkey.command || hotkey.control)
  );
}
function preferences(value: unknown): boolean {
  if (
    !object(value, [
      "current",
      "devices",
      "error",
      "refreshing",
      "needsRestart",
      "recoveryRequired",
      "languages",
      "keyLabels",
    ])
  )
    return false;
  return (
    Array.isArray(value.keyLabels) &&
    value.keyLabels.length === 26 &&
    value.keyLabels.every(
      (entry, index) =>
        object(entry, ["key", "plain", "command"]) &&
        entry.key === "ABCDEFGHIJKLMNOPQRSTUVWXYZ"[index] &&
        text(entry.plain, 128) &&
        entry.plain.length > 0 &&
        text(entry.command, 128) &&
        entry.command.length > 0 &&
        Array.from(entry.plain + entry.command).every((character) => {
          const code = character.charCodeAt(0);
          return code >= 32 && (code < 127 || code > 159);
        }),
    ) &&
    draft(value.current) &&
    Array.isArray(value.devices) &&
    value.devices.length <= 128 &&
    value.devices.every(
      (device) =>
        object(device, ["name", "available"]) &&
        deviceName(device.name) &&
        typeof device.available === "boolean",
    ) &&
    optionalText(value.error) &&
    typeof value.refreshing === "boolean" &&
    typeof value.needsRestart === "boolean" &&
    typeof value.recoveryRequired === "boolean" &&
    Array.isArray(value.languages) &&
    value.languages.length > 0 &&
    value.languages.length <= 2 &&
    value.languages.every((language) =>
      oneOf(language, ["german", "english"]),
    ) &&
    new Set(value.languages).size === value.languages.length
  );
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
      "refinement",
      "session",
      "permissions",
      "labels",
      "settingsBusy",
      "settingsRequest",
      "preferences",
      "error",
    ])
  )
    throw new Error("Ungültiger Anwendungsstatus");
  const { setup, session, labels, permissions: access } = value;
  if (
    !refinement(value.refinement) ||
    !preferences(value.preferences) ||
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
    !object(session, [
      "phase",
      "id",
      "text",
      "outcome",
      "maximumWarning",
      "maximumReached",
      "lastHeldSession",
      "elapsedMillis",
      "levelPermille",
    ]) ||
    !oneOf(session.phase, sessionPhases) ||
    !(session.id === null || (number(session.id) && session.id > 0)) ||
    !text(session.text, 262144) ||
    typeof session.maximumWarning !== "boolean" ||
    typeof session.maximumReached !== "boolean" ||
    !(
      session.lastHeldSession === null ||
      (number(session.lastHeldSession) && session.lastHeldSession > 0)
    ) ||
    !number(session.elapsedMillis) ||
    session.elapsedMillis > 300_000 ||
    !number(session.levelPermille) ||
    session.levelPermille > 1_000 ||
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
function refinement(value: unknown): boolean {
  if (!object(value, ["setup", "sessionId", "original", "status", "message"]))
    return false;
  const setup = value.setup;
  return (
    object(setup, [
      "available",
      "phase",
      "probePassed",
      "downloadedBytes",
      "totalBytes",
      "canCancel",
      "error",
    ]) &&
    typeof setup.available === "boolean" &&
    typeof setup.probePassed === "boolean" &&
    (!setup.probePassed || setup.phase === "Ready") &&
    oneOf(setup.phase, setupPhases) &&
    number(setup.downloadedBytes) &&
    number(setup.totalBytes) &&
    setup.downloadedBytes <= setup.totalBytes &&
    typeof setup.canCancel === "boolean" &&
    optionalText(setup.error) &&
    (value.sessionId === null ||
      (number(value.sessionId) && value.sessionId > 0)) &&
    (value.original === null || text(value.original, 262144)) &&
    optionalText(value.message) &&
    oneOf(value.status, [
      "Disabled",
      "Processing",
      "Skipped",
      "Unchanged",
      "Edited",
      "Failed",
      "Held",
      "Cancelled",
    ])
  );
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
    | "refinement_download"
    | "refinement_cancel_setup"
    | "recording_start"
    | "microphone_request"
    | "accessibility_request"
    | "devices_refresh"
    | "settings_reset",
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

export function saveSettings(settings: SettingsDraft): Promise<void> {
  if (!draft(settings))
    return Promise.reject(new Error("Ungültige Einstellungen"));
  return invoke("settings_save", { settings });
}

export function clearTranscript(expectedText: string): Promise<void> {
  if (!text(expectedText, 262144))
    return Promise.reject(new Error("Ungültiger Text"));
  return invoke("transcript_clear", { expectedText });
}

/** Copy the displayed snapshot, never a later runtime result. */
export function copyTranscript(value: string): Promise<void> {
  if (!text(value, 262144) || value.length === 0)
    return Promise.reject(
      new Error("Der Text ist leer oder zu lang zum Kopieren."),
    );
  return invoke("transcript_copy", { text: value });
}
