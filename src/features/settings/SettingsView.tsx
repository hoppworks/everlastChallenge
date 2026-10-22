import { useEffect, useRef, useState } from "react";
import type { Snapshot, SettingsDraft } from "../../ipc";
import type { AppViewProps } from "../app/AppView";
import "./styles.css";

export interface SettingsViewProps {
  refinement: Snapshot["refinement"]["setup"];
  onDownloadRefinement: () => void;
  onCancelRefinementSetup: () => void;
  preferences: Snapshot["preferences"];
  setup: AppViewProps["setup"];
  permissions: AppViewProps["permissions"];
  busy: boolean;
  permissionsBusy?: boolean;
  error?: string;
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
    "Öffne Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon und aktiviere Everlast Coding Challenge.",
  restricted:
    "Der Mikrofonzugriff ist durch System- oder Verwaltungsvorgaben eingeschränkt. Prüfe diese Vorgaben oder wende dich an die Geräteverwaltung.",
  unavailable:
    "Der Mikrofonstatus konnte nicht ermittelt werden. Kehre zur App zurück, um ihn erneut zu prüfen.",
};
const accessibilityHelp: Partial<
  Record<AppViewProps["permissions"]["accessibility"], string>
> = {
  denied:
    "Öffne Systemeinstellungen → Datenschutz & Sicherheit → Bedienungshilfen und aktiviere Everlast Coding Challenge.",
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
  const { preferences, setup, permissions, refinement } = props;
  const [area, setArea] = useState<
    "Aufnahme" | "Berechtigungen" | "Modell" | "Textbereinigung"
  >(
    permissions.microphone !== "granted" ||
      permissions.accessibility !== "granted"
      ? "Berechtigungen"
      : "Aufnahme",
  );
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
  const permissionsDisabled =
    (props.permissionsBusy ?? props.busy) || preferences.needsRestart;
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
    <section className="settings-view" aria-labelledby="settings-heading">
      <header className="settings-header">
        <button
          className="settings-back"
          type="button"
          aria-label="Zurück zum Diktat"
          onClick={props.onClose}
        >
          <svg
            viewBox="0 0 24 24"
            width="20"
            height="20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            aria-hidden="true"
          >
            <path d="m14 6-6 6 6 6" />
          </svg>
        </button>
        <h2 id="settings-heading" ref={heading} tabIndex={-1}>
          Einstellungen
        </h2>
      </header>
      <nav className="settings-navigation" aria-label="Einstellungsbereiche">
        {(
          ["Aufnahme", "Textbereinigung", "Berechtigungen", "Modell"] as const
        ).map((name) => (
          <button
            type="button"
            key={name}
            aria-current={area === name ? "page" : undefined}
            onClick={() => setArea(name)}
          >
            {name}
          </button>
        ))}
      </nav>
      <div className="settings-content">
        {props.error && <p role="alert">{props.error}</p>}
        {preferences.needsRestart && (
          <p role="alert">
            Bitte starte die App neu, um die Einstellungen wieder vollständig
            anzuwenden. Bis dahin sind Aufnahme und weitere Änderungen gesperrt.
          </p>
        )}
        {preferences.error && <p role="alert">{preferences.error}</p>}
        {area === "Aufnahme" && (
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
                      {draft.language === "german" ? "Deutsch" : "Englisch"}{" "}
                      (nicht verfügbar)
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
            <details className="settings-advanced">
              <summary>Tastenkürzel ändern</summary>
              <fieldset
                disabled={disabled}
                aria-describedby="physical-key-help"
              >
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
                          key: event.target
                            .value as SettingsDraft["hotkey"]["key"],
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
                            hotkey: {
                              ...draft.hotkey,
                              [key]: event.target.checked,
                            },
                          })
                        }
                      />
                      {label}
                    </label>
                  ))}
                </div>
                <p id="physical-key-help" className="settings-help">
                  Die Beschriftung folgt dem aktiven Layout. Gespeichert bleibt
                  die physische US-Position; ein Layoutwechsel ändert das Kürzel
                  nicht.
                </p>
                {!valid && (
                  <p role="alert">
                    Wähle mindestens Option, Command oder Control. Shift allein
                    reicht nicht aus.
                  </p>
                )}
              </fieldset>
            </details>
            <div className="settings-save">
              <button
                className="primary-action"
                type="submit"
                disabled={disabled || !valid || !dirty}
              >
                Einstellungen speichern
              </button>
              {dirty && (
                <span className="settings-help" role="status">
                  Ungespeicherte Änderungen
                </span>
              )}
            </div>
          </form>
        )}
        {area === "Textbereinigung" && (
          <section
            className="settings-detail"
            aria-labelledby="settings-refinement"
          >
            <h3 id="settings-refinement">Textbereinigung</h3>
            <p>
              Füllwörter, eindeutige Versprecher und Zeichensetzung lokal
              bereinigen.
            </p>
            <p className="settings-help">
              Bei einem Fehler bleibt das Original zum Kopieren erhalten. Es
              wird dann nichts automatisch eingefügt.
            </p>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                if (
                  !disabled &&
                  valid &&
                  dirty &&
                  (refinement.phase === "Ready" || !draft.refinement)
                )
                  props.onSave(draft);
              }}
            >
              <label className="refinement-switch">
                <input
                  type="checkbox"
                  role="switch"
                  aria-checked={draft.refinement}
                  checked={draft.refinement}
                  disabled={
                    disabled ||
                    (!draft.refinement && refinement.phase !== "Ready")
                  }
                  onChange={(event) =>
                    setDraft({ ...draft, refinement: event.target.checked })
                  }
                />
                Textbereinigung verwenden
              </label>
              {!refinement.available ? (
                <p role="status">
                  Textbereinigung ist in diesem Build nicht verfügbar.
                </p>
              ) : (
                <>
                  {refinement.phase === "Ready" ? (
                    <p className="settings-help">
                      Das Textmodell ist eingerichtet. Die Verarbeitung bleibt
                      auf diesem Mac.
                    </p>
                  ) : (
                    <p role="status">
                      {refinement.phase === "Downloading"
                        ? "Textmodell wird geladen"
                        : refinement.phase === "Checking"
                          ? "Textmodell wird geprüft"
                          : refinement.phase === "Cancelling"
                            ? "Einrichtung wird abgebrochen"
                            : refinement.phase === "Cancelled"
                              ? "Einrichtung abgebrochen"
                              : "Einmalige Einrichtung erforderlich"}
                    </p>
                  )}
                  {refinement.phase !== "Ready" && (
                    <p className="settings-help">
                      Download:{" "}
                      {new Intl.NumberFormat("de-DE", {
                        maximumFractionDigits: 1,
                      }).format(refinement.totalBytes / 1_000_000_000)}{" "}
                      GB. Diktieren ohne Bereinigung bleibt danach weiterhin
                      möglich.
                    </p>
                  )}
                  {refinement.phase !== "Ready" && (
                    <p className="settings-help">
                      Nach erfolgreicher Einrichtung wird die Textbereinigung
                      automatisch aktiviert.
                    </p>
                  )}
                  {refinement.phase === "Downloading" && (
                    <div className="download-progress">
                      <progress
                        aria-label="Textmodelldownload"
                        value={refinement.downloadedBytes}
                        max={refinement.totalBytes || 1}
                      />
                      <p>
                        {new Intl.NumberFormat("de-DE", {
                          maximumFractionDigits: 0,
                        }).format(refinement.downloadedBytes / 1_000_000)}{" "}
                        von{" "}
                        {new Intl.NumberFormat("de-DE", {
                          maximumFractionDigits: 0,
                        }).format(refinement.totalBytes / 1_000_000)}{" "}
                        MB
                      </p>
                    </div>
                  )}
                  {refinement.error && <p role="alert">{refinement.error}</p>}
                  {["Missing", "Failed", "Cancelled"].includes(
                    refinement.phase,
                  ) && (
                    <button
                      type="button"
                      disabled={disabled}
                      onClick={props.onDownloadRefinement}
                    >
                      {refinement.phase === "Failed"
                        ? "Erneut versuchen"
                        : "Einrichten und aktivieren"}
                    </button>
                  )}
                  {refinement.canCancel && (
                    <button
                      type="button"
                      onClick={props.onCancelRefinementSetup}
                    >
                      Einrichtung abbrechen
                    </button>
                  )}
                </>
              )}
              <div className="settings-save">
                <button
                  className="primary-action"
                  type="submit"
                  disabled={
                    disabled ||
                    !valid ||
                    !dirty ||
                    (draft.refinement && refinement.phase !== "Ready")
                  }
                >
                  Einstellungen speichern
                </button>
                {dirty && (
                  <span className="settings-help" role="status">
                    Ungespeicherte Änderungen
                  </span>
                )}
              </div>
            </form>
            <details className="settings-advanced">
              <summary>Modelldetails</summary>
              <p>Qwen3 4B · vollständig lokale Verarbeitung mit MLX</p>
              <p className="settings-help">
                Vorgeschlagene Änderungen werden vor der Übernahme geprüft. Der
                Wortlaut kann im Original nachgelesen werden.
              </p>
            </details>
          </section>
        )}
        {area === "Modell" && (
          <section className="settings-detail" aria-labelledby="settings-model">
            <h3 id="settings-model">Sprachmodell</h3>
            <p>{setup.model.name}</p>
            <p className="settings-help">
              MLX-Hybrid · optimierter Encoder, whisper.cpp-Decoder
            </p>
            <p>{phaseLabels[setup.phase]}</p>
            {setup.error && <p role="alert">{setup.error}</p>}
            <p className="settings-help">
              Die Spracherkennung läuft lokal auf diesem Mac.
            </p>
            <details className="settings-advanced">
              <summary>Modelldetails</summary>
              <dl>
                <div>
                  <dt>Installationsbedarf einschließlich lokaler Ableitung</dt>
                  <dd>
                    {new Intl.NumberFormat("de-DE", {
                      maximumFractionDigits: 1,
                    }).format(setup.model.downloadBytes / 1_000_000)}{" "}
                    MB
                  </dd>
                </div>
              </dl>
              <a href={setup.model.sourceUrl} target="_blank" rel="noreferrer">
                Modellquelle öffnen
              </a>
            </details>
          </section>
        )}
        {area === "Berechtigungen" && (
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
                <dt>Bedienungshilfen (Accessibility)</dt>
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
                Kehre nach der Änderung zu Everlast Coding Challenge zurück. Die
                App prüft den Status dann erneut.
              </p>
            )}
            {(permissions.microphone === "notDetermined" ||
              permissions.accessibility === "notDetermined" ||
              permissions.accessibility === "denied") && (
              <div className="app-actions">
                {permissions.microphone === "notDetermined" && (
                  <button
                    type="button"
                    disabled={permissionsDisabled}
                    onClick={props.onMicrophone}
                  >
                    Mikrofonzugriff anfragen
                  </button>
                )}
                {(permissions.accessibility === "notDetermined" ||
                  permissions.accessibility === "denied") && (
                  <button
                    type="button"
                    disabled={permissionsDisabled}
                    onClick={props.onAccessibility}
                  >
                    Freigabe anfragen
                  </button>
                )}
              </div>
            )}
          </section>
        )}
      </div>
    </section>
  );
}
