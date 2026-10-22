import { useEffect, useState } from "react";
import { LevelVisual } from "./LevelVisual";
import "./styles.css";

type SetupPhase =
  | "Missing"
  | "Checking"
  | "Downloading"
  | "Loading"
  | "Cancelling"
  | "Cancelled"
  | "Ready"
  | "Failed";
type SessionPhase =
  | "Idle"
  | "Starting"
  | "Recording"
  | "Finalizing"
  | "Delivering"
  | "Cancelling";
type PermissionStatus =
  "granted" | "denied" | "notDetermined" | "restricted" | "unavailable";
type SessionOutcome =
  | { kind: "local" | "empty" | "sent" | "cancelled" }
  | { kind: "held" | "failed"; message: string };

export interface AppViewProps {
  onDisclosureChange?: (expanded: boolean) => void;
  setup: {
    phase: SetupPhase;
    model: { name: string; downloadBytes: number; sourceUrl: string };
    downloadedBytes?: number;
    totalBytes?: number;
    canCancel?: boolean;
    error?: string;
  };
  session: {
    id?: number | null;
    phase: SessionPhase;
    text: string;
    outcome?: SessionOutcome;
    elapsedSeconds?: number;
    level?: number;
    maximumWarning?: boolean;
    maximumReached?: boolean;
  };
  permissions: {
    microphone: PermissionStatus;
    accessibility: PermissionStatus;
  };
  labels: { device: string; language: string; hotkey: string };
  pending?: boolean;
  settingsBusy?: boolean;
  error?: string;
  onDownload: () => void;
  onCancelSetup: () => void;
  onStart: () => void;
  onStop: () => void;
  onCancel: () => void;
  onCopy: () => void;
  onClear: () => void;
  onSettings: () => void;
}

const setupTitles: Record<SetupPhase, string> = {
  Missing: "Modell einrichten",
  Checking: "Modell wird geprüft",
  Downloading: "Modell wird geladen",
  Loading: "Spracherkennung wird gestartet",
  Cancelling: "Einrichtung wird abgebrochen",
  Cancelled: "Einrichtung abgebrochen",
  Ready: "Bereit zum Diktieren",
  Failed: "Modell konnte nicht eingerichtet werden",
};
const sessionTitles: Record<Exclude<SessionPhase, "Idle">, string> = {
  Starting: "Mikrofon wird gestartet",
  Recording: "Aufnahme läuft",
  Finalizing: "Diktat wird verarbeitet",
  Delivering: "Text wird übergeben",
  Cancelling: "Aufnahme wird beendet",
};
const outcomeTitles = {
  local: "Diktat ist fertig",
  empty: "Kein Sprachinhalt erkannt",
  sent: "Einfügebefehl gesendet",
  cancelled: "Diktat abgebrochen",
  held: "Text wurde zurückgehalten",
  failed: "Diktat konnte nicht abgeschlossen werden",
};

function bytes(value: number): string {
  return `${new Intl.NumberFormat("de-DE", { maximumFractionDigits: 1 }).format(value / 1_000_000)} MB`;
}
function duration(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

/** Presentation only: every state and action belongs to the native supervisor. */
export function AppView(props: AppViewProps) {
  const {
    setup,
    session,
    permissions,
    labels,
    pending = false,
    onDisclosureChange,
  } = props;
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [expanded, setExpanded] = useState(Boolean(session.text));
  const [lastText, setLastText] = useState(session.text);
  const [lastResult, setLastResult] = useState<number | null>(null);
  const completed =
    session.phase === "Idle" &&
    session.text &&
    (session.outcome?.kind === "local" ||
      session.outcome?.kind === "sent" ||
      session.outcome?.kind === "held");
  if (completed && session.id != null && session.id !== lastResult) {
    setLastResult(session.id);
    setExpanded(true);
  }
  // A genuinely new result opens itself; telemetry never reopens a closed drawer.
  if (lastText !== session.text) {
    setLastText(session.text);
    if (session.text) setExpanded(true);
  }
  useEffect(() => {
    onDisclosureChange?.(expanded || detailsOpen);
  }, [expanded, detailsOpen, onDisclosureChange]);
  const active = session.phase !== "Idle";
  const title =
    session.phase !== "Idle"
      ? sessionTitles[session.phase]
      : setup.phase !== "Ready"
        ? setupTitles[setup.phase]
        : session.outcome
          ? outcomeTitles[session.outcome.kind]
          : permissions.microphone === "granted"
            ? setupTitles.Ready
            : "Mikrofonzugriff erforderlich";
  const canStart =
    !active &&
    setup.phase === "Ready" &&
    permissions.microphone === "granted" &&
    !props.settingsBusy;
  const showDownload =
    !active && ["Missing", "Cancelled", "Failed"].includes(setup.phase);
  const detail =
    session.outcome?.kind === "held" || session.outcome?.kind === "failed"
      ? session.outcome.message
      : undefined;
  const downloading = setup.phase === "Downloading";
  const progressKnown =
    downloading &&
    setup.downloadedBytes !== undefined &&
    setup.totalBytes !== undefined &&
    setup.totalBytes > 0;

  return (
    <main className="dictation-app" aria-label="Everlast Coding Challenge">
      <header className="app-header">
        <button
          type="button"
          className="settings-button"
          aria-label="Einstellungen"
          onClick={props.onSettings}
        >
          <svg
            viewBox="0 0 24 24"
            width="18"
            height="18"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            aria-hidden="true"
          >
            <path d="m9 3-.5 2-2 1-2-.5-2 3 1.5 1.5v3L2.5 15l2 3 2-.5 2 1 .5 2h4l.5-2 2-1 2 .5 2-3-1.5-1.5v-3L20.5 9l-2-3-2 .5-2-1-.5-2z" />
            <circle cx="11.5" cy="12" r="3" />
          </svg>
        </button>
      </header>
      <div className="palette-main">
        <LevelVisual
          {...(session.phase === "Recording" && session.level !== undefined
            ? { level: session.level }
            : {})}
        />
        <section
          className="dictation-status"
          aria-labelledby="dictation-status-title"
        >
          <div role="status" aria-live="polite" aria-atomic="true">
            <h2 id="dictation-status-title">{title}</h2>
          </div>
          {session.maximumReached && (
            <p role="status" aria-live="polite">
              Die Aufnahme wurde an der 5-Minuten-Grenze beendet.
            </p>
          )}
          {active ? (
            <>
              {session.phase === "Starting" && (
                <p>Die Aufnahme beginnt, sobald das Mikrofon bereit ist.</p>
              )}
              {session.phase === "Finalizing" && (
                <p>Die Spracherkennung verarbeitet dein Diktat lokal.</p>
              )}
              {session.phase === "Delivering" && (
                <p>Das ursprüngliche Textfeld wird noch einmal geprüft.</p>
              )}
              {session.phase === "Cancelling" && (
                <p>Du kannst erneut starten, sobald das Mikrofon frei ist.</p>
              )}
              {session.phase === "Recording" && (
                <div className="recording-readout">
                  {session.elapsedSeconds !== undefined && (
                    <output aria-label="Aufnahmedauer">
                      {duration(session.elapsedSeconds)}
                    </output>
                  )}
                  <button
                    className="cancel-action"
                    type="button"
                    onClick={props.onCancel}
                  >
                    Abbrechen
                  </button>
                  {session.maximumWarning && (
                    <p role="status">
                      Die Aufnahme endet bei 5 Minuten. Bitte beende deinen
                      Gedanken.
                    </p>
                  )}
                </div>
              )}
            </>
          ) : (
            <>
              {setup.phase !== "Ready" && (
                <>
                  <p className="setup-summary">
                    Einmal einrichten, danach lokal und offline diktieren.
                  </p>
                  <details className="model-disclosure">
                    <summary>Modelldetails</summary>
                    <p>
                      {setup.model.name} · {bytes(setup.model.downloadBytes)}
                    </p>
                    <p>
                      Das Sprachmodell wird von{" "}
                      <a
                        href={setup.model.sourceUrl}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Hugging Face
                      </a>{" "}
                      geladen. Dazu kommt der{" "}
                      <a
                        href="https://huggingface.co/ggml-org/whisper-vad/tree/9ffd54a1e1ee413ddf265af9913beaf518d1639b"
                        target="_blank"
                        rel="noreferrer"
                      >
                        Silero-Sprachdetektor
                      </a>{" "}
                      (0,9 MB). Vorhandene geprüfte Dateien werden
                      wiederverwendet.
                    </p>
                  </details>
                </>
              )}
              {downloading && (
                <div className="download-progress">
                  <progress
                    aria-label="Modelldownload"
                    {...(progressKnown
                      ? { value: setup.downloadedBytes, max: setup.totalBytes }
                      : {})}
                  />
                  {progressKnown && (
                    <p>
                      {bytes(setup.downloadedBytes ?? 0)} von{" "}
                      {bytes(setup.totalBytes ?? 0)}
                    </p>
                  )}
                </div>
              )}
              {setup.phase === "Checking" && (
                <p>Dateigröße und Prüfsumme werden kontrolliert.</p>
              )}
              {setup.phase === "Loading" && (
                <p>
                  Das geprüfte Modell wird für die lokale Spracherkennung
                  vorbereitet.
                </p>
              )}
              {setup.phase === "Cancelling" && (
                <p>Die laufende Einrichtung wird sicher beendet.</p>
              )}
              {setup.phase === "Failed" && setup.error && (
                <p role="alert">{setup.error}</p>
              )}
              {setup.phase === "Ready" && !session.outcome && (
                <p>{labels.hotkey} startet und stoppt in jeder App.</p>
              )}
              {session.outcome?.kind === "empty" && (
                <p>
                  Der letzte Text bleibt verfügbar. Versuche es mit einer neuen
                  Aufnahme.
                </p>
              )}
              {detail && (
                <p
                  className="status-detail"
                  role={
                    session.outcome?.kind === "failed" ? "alert" : undefined
                  }
                >
                  {detail}
                </p>
              )}
            </>
          )}
        </section>
        <div className="app-actions palette-actions">
          {showDownload && (
            <button
              className="primary-action"
              type="button"
              onClick={props.onDownload}
              disabled={pending}
            >
              {setup.phase === "Failed" ? "Erneut versuchen" : "Modell laden"}
            </button>
          )}
          {!active && setup.canCancel && (
            <button type="button" onClick={props.onCancelSetup}>
              Einrichtung abbrechen
            </button>
          )}
          {!active && setup.phase === "Ready" && (
            <button
              className="primary-action"
              type="button"
              onClick={props.onStart}
              disabled={pending || !canStart}
            >
              {session.outcome ? "Erneut aufnehmen" : "Aufnehmen"}
            </button>
          )}
          {session.phase === "Recording" && (
            <button
              className="primary-action is-recording"
              type="button"
              onClick={props.onStop}
            >
              Stoppen
            </button>
          )}
          {active &&
            session.phase !== "Recording" &&
            session.phase !== "Cancelling" && (
              <button type="button" onClick={props.onCancel}>
                Abbrechen
              </button>
            )}
        </div>
      </div>
      {props.error && (
        <p className="status-detail" role="alert">
          {props.error}
        </p>
      )}
      {(permissions.microphone !== "granted" ||
        permissions.accessibility !== "granted") && (
        <section className="permissions" aria-label="Berechtigungen">
          <div>
            {permissions.microphone !== "granted" ? (
              <p>
                <strong>Mikrofon freigeben.</strong>{" "}
                {permissions.microphone === "restricted"
                  ? "Der Zugriff ist durch Systemeinstellungen eingeschränkt."
                  : "Für Aufnahmen ist der Zugriff noch nicht verfügbar."}
              </p>
            ) : (
              <p>
                <strong>Automatisches Einfügen aktivieren.</strong> Dein Text
                bleibt hier kopierbar.
              </p>
            )}
            {permissions.accessibility !== "granted" && (
              <p className="permission-secondary">
                Automatisches Einfügen ist noch nicht verfügbar.
              </p>
            )}
          </div>
          <button type="button" onClick={props.onSettings}>
            Berechtigungen prüfen
          </button>
        </section>
      )}
      <section
        className="transcript-section"
        aria-labelledby="transcript-label"
      >
        <button
          type="button"
          className="transcript-toggle"
          aria-expanded={expanded}
          aria-controls="last-transcript"
          onClick={() => setExpanded(!expanded)}
        >
          <span id="transcript-label">Letzter Text</span>
          <span aria-hidden="true">{expanded ? "−" : "+"}</span>
        </button>
        {expanded && (
          <div id="last-transcript">
            <div className="transcript-header">
              <h2>Letzter Text</h2>
              <div className="app-actions">
                <button
                  type="button"
                  onClick={props.onCopy}
                  disabled={!session.text || pending}
                >
                  Kopieren
                </button>
                <button
                  type="button"
                  onClick={props.onClear}
                  disabled={!session.text || pending}
                >
                  Löschen
                </button>
              </div>
            </div>
            <textarea
              aria-labelledby="transcript-label"
              value={session.text}
              readOnly
              spellCheck={false}
              placeholder="Dein nächstes Diktat erscheint hier."
            />
          </div>
        )}
      </section>
      <details
        className="context-disclosure"
        onToggle={(event) => setDetailsOpen(event.currentTarget.open)}
      >
        <summary>Aufnahmedetails</summary>{" "}
        <dl className="session-preferences">
          <div>
            <dt>Mikrofon</dt>
            <dd>{labels.device}</dd>
          </div>
          <div>
            <dt>Sprache</dt>
            <dd>{labels.language}</dd>
          </div>
          <div>
            <dt>Tastenkürzel</dt>
            <dd>
              <kbd>{labels.hotkey}</kbd>
            </dd>
          </div>
        </dl>
        <footer>Aufnahme und Spracherkennung bleiben auf diesem Mac.</footer>
      </details>
    </main>
  );
}
