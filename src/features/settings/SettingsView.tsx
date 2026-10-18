import { useEffect, useRef, useState } from "react";
import type { Snapshot, SettingsDraft } from "../../ipc";
import type { AppViewProps } from "../app/AppView";
import "./styles.css";

export interface SettingsViewProps {
  preferences: Snapshot["preferences"];
  setup: AppViewProps["setup"];
  permissions: AppViewProps["permissions"];
  busy: boolean;
  onSave: (draft: SettingsDraft) => void;
  onRefresh: () => void;
  onMicrophone: () => void;
  onAccessibility: () => void;
  onClose: () => void;
}
const phaseLabels: Record<AppViewProps["setup"]["phase"], string> = {
  Missing: "Noch nicht eingerichtet",
  Checking: "Wird geprüft",
  Downloading: "Wird geladen",
  Loading: "Wird gestartet",
  Cancelling: "Einrichtung wird abgebrochen",
  Cancelled: "Einrichtung abgebrochen",
  Ready: "Für die lokale Erkennung bereit",
  Failed: "Einrichtung fehlgeschlagen",
};
const permissionLabels: Record<
  AppViewProps["permissions"]["microphone"],
  string
> = {
  granted: "Freigegeben",
  denied: "Nicht freigegeben",
  notDetermined: "Noch nicht angefragt",
  restricted: "Vom System eingeschränkt",
  unavailable: "Nicht verfügbar",
};
const microphoneHelp: Partial<
  Record<AppViewProps["permissions"]["microphone"], string>
> = {
  denied:
    "Öffne Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon und aktiviere Everlast Dictation.",
  restricted:
    "Der Mikrofonzugriff ist durch System- oder Verwaltungsvorgaben eingeschränkt. Prüfe diese Vorgaben oder wende dich an die Geräteverwaltung.",
  unavailable:
    "Der Mikrofonstatus konnte nicht ermittelt werden. Kehre zur App zurück, um ihn erneut zu prüfen.",
};
const accessibilityHelp: Partial<
  Record<AppViewProps["permissions"]["accessibility"], string>
> = {
  denied:
    "Öffne Systemeinstellungen → Datenschutz & Sicherheit → Bedienungshilfen und aktiviere Everlast Dictation.",
  restricted:
    "Die Bedienungshilfen sind durch System- oder Verwaltungsvorgaben eingeschränkt. Prüfe diese Vorgaben oder wende dich an die Geräteverwaltung.",
  unavailable:
    "Der Status der Bedienungshilfen konnte nicht ermittelt werden. Kehre zur App zurück, um ihn erneut zu prüfen.",
};
const keys = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split(
  "",
) as SettingsDraft["hotkey"]["key"][];
const modifiers = [
  ["option", "Option"],
  ["command", "Command"],
  ["control", "Control"],
  ["shift", "Shift"],
] as const;
function identity(value: SettingsDraft): string {
  return JSON.stringify(value);
}

export function SettingsView(props: SettingsViewProps) {
  const { preferences, setup, permissions } = props;
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    heading.current?.focus();
  }, []);
  const [baseline, setBaseline] = useState(identity(preferences.current));
  const [draft, setDraft] = useState(preferences.current);
  const current = identity(preferences.current);
  // Reconcile actual settings changes, not each telemetry snapshot object.
  if (baseline !== current) {
    setBaseline(current);
    setDraft(preferences.current);
  }
  const disabled = props.busy || preferences.needsRestart;
  const valid =
    draft.hotkey.option || draft.hotkey.command || draft.hotkey.control;
  const dirty = identity(draft) !== current;
  const devices = preferences.devices.filter(
    (device, index, all) =>
      all.findIndex((entry) => entry.name === device.name) === index,
  );
  const selectedMissing =
    draft.device !== null &&
    !devices.some((device) => device.name === draft.device);
  return (
    <section
      className="dictation-app settings-view"
      aria-labelledby="settings-heading"
    >
      <header className="settings-header">
        <h2 id="settings-heading" ref={heading} tabIndex={-1}>
          Einstellungen
        </h2>
        <button type="button" onClick={props.onClose}>
          Schließen
        </button>
      </header>
      {preferences.needsRestart && (
        <p role="alert">
          Die Einstellungen konnten nicht vollständig angewendet werden. Bitte
          starte die App neu, bevor du sie änderst.
        </p>
      )}
      {preferences.error && <p role="alert">{preferences.error}</p>}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!disabled && valid && dirty) props.onSave(draft);
        }}
      >
        <fieldset disabled={disabled}>
          <legend>Aufnahme</legend>
          <label className="settings-field">
            Mikrofon
            <select
              value={draft.device ?? ""}
              onChange={(event) =>
                setDraft({ ...draft, device: event.target.value || null })
              }
            >
              <option value="">Systemstandard</option>
              {selectedMissing && (
                <option value={draft.device ?? ""} disabled>
                  {draft.device} (nicht verfügbar)
                </option>
              )}
              {devices.map((device) => {
                const ambiguous =
                  preferences.devices.filter(
                    (entry) => entry.name === device.name,
                  ).length > 1;
                const available = device.available && !ambiguous;
                return (
                  <option
                    key={device.name}
                    value={device.name}
                    disabled={!available}
                  >
                    {device.name}
                    {available ? "" : " (nicht eindeutig)"}
                  </option>
                );
              })}
            </select>
          </label>
          <button
            type="button"
            disabled={disabled || preferences.refreshing}
            onClick={props.onRefresh}
          >
            {preferences.refreshing
              ? "Mikrofone werden gesucht …"
              : "Mikrofone aktualisieren"}
          </button>
          <label className="settings-field">
            Sprache
            <select
              value={draft.language}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  language: event.target.value as SettingsDraft["language"],
                })
              }
            >
              {!preferences.languages.includes(draft.language) && (
                <option value={draft.language} disabled>
                  {draft.language === "german" ? "Deutsch" : "Englisch"} (nicht
                  verfügbar)
                </option>
              )}
              {preferences.languages.map((language) => (
                <option key={language} value={language}>
                  {language === "german" ? "Deutsch" : "Englisch"}
                </option>
              ))}
            </select>
          </label>
        </fieldset>
        <fieldset disabled={disabled} aria-describedby="physical-key-help">
          <legend>Tastenkürzel</legend>
          <label className="settings-field">
            Physische Taste (US-Position)
            <select
              value={draft.hotkey.key}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  hotkey: {
                    ...draft.hotkey,
                    key: event.target.value as SettingsDraft["hotkey"]["key"],
                  },
                })
              }
            >
              {keys.map((key) => {
                const entry = preferences.keyLabels.find(
                  (entry) => entry.key === key,
                );
                const label = entry
                  ? draft.hotkey.command
                    ? entry.command
                    : entry.plain
                  : "Layout unbekannt";
                return (
                  <option key={key} value={key}>
                    {label} (US-Position {key})
                  </option>
                );
              })}
            </select>
          </label>
          <div className="settings-modifiers">
            {modifiers.map(([key, label]) => (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={draft.hotkey[key]}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      hotkey: { ...draft.hotkey, [key]: event.target.checked },
                    })
                  }
                />
                {label}
              </label>
            ))}
          </div>
          <p id="physical-key-help" className="settings-help">
            Die Beschriftung folgt dem aktiven Layout. Gespeichert bleibt die
            physische US-Position; ein Layoutwechsel ändert das Kürzel nicht.
          </p>
          {!valid && (
            <p role="alert">
              Wähle mindestens Option, Command oder Control. Shift allein reicht
              nicht aus.
            </p>
          )}
        </fieldset>
        <button
          className="primary-action"
          type="submit"
          disabled={disabled || !valid || !dirty}
        >
          Einstellungen speichern
        </button>
      </form>
      <section className="settings-detail" aria-labelledby="settings-model">
        <h3 id="settings-model">Sprachmodell</h3>
        <p>{setup.model.name}</p>
        <p>{phaseLabels[setup.phase]}</p>
        {setup.error && <p role="alert">{setup.error}</p>}
      </section>
      <section
        className="settings-detail"
        aria-labelledby="settings-permissions"
      >
        <h3 id="settings-permissions">Berechtigungen</h3>
        <p>
          Aufnahme und Spracherkennung erfolgen lokal. Zum automatischen
          Einfügen werden zusätzlich die Bedienungshilfen benötigt.
        </p>
        <dl>
          <div>
            <dt>Mikrofon</dt>
            <dd>{permissionLabels[permissions.microphone]}</dd>
          </div>
          <div>
            <dt>Bedienungshilfen</dt>
            <dd>{permissionLabels[permissions.accessibility]}</dd>
          </div>
        </dl>
        {microphoneHelp[permissions.microphone] && (
          <p className="settings-help" id="microphone-permission-help">
            {microphoneHelp[permissions.microphone]}
          </p>
        )}
        {accessibilityHelp[permissions.accessibility] && (
          <p className="settings-help" id="accessibility-permission-help">
            {accessibilityHelp[permissions.accessibility]}
          </p>
        )}
        {(permissions.microphone === "denied" ||
          permissions.microphone === "restricted" ||
          permissions.accessibility === "denied" ||
          permissions.accessibility === "restricted") && (
          <p className="settings-help">
            Kehre nach der Änderung zu Everlast Dictation zurück. Die App prüft
            den Status dann erneut.
          </p>
        )}
        {(permissions.microphone === "notDetermined" ||
          permissions.accessibility === "notDetermined") && (
          <div className="app-actions">
            {permissions.microphone === "notDetermined" && (
              <button
                type="button"
                disabled={disabled}
                onClick={props.onMicrophone}
              >
                Mikrofonzugriff anfragen
              </button>
            )}
            {permissions.accessibility === "notDetermined" && (
              <button
                type="button"
                disabled={disabled}
                onClick={props.onAccessibility}
              >
                Bedienungshilfen freigeben
              </button>
            )}
          </div>
        )}
      </section>
    </section>
  );
}
