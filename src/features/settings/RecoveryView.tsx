import { useEffect, useRef, useState } from "react";

interface RecoveryViewProps {
  reason: string | null;
  error?: string;
  pending: boolean;
  onReset: () => void;
}

export function RecoveryView({
  reason,
  error,
  pending,
  onReset,
}: RecoveryViewProps) {
  const [confirming, setConfirming] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const confirmButton = useRef<HTMLButtonElement>(null);
  const resetButton = useRef<HTMLButtonElement>(null);
  const previouslyConfirming = useRef(false);
  useEffect(() => {
    if (confirming) confirmButton.current?.focus();
    else if (previouslyConfirming.current) resetButton.current?.focus();
    previouslyConfirming.current = confirming;
  }, [confirming]);
  useEffect(() => {
    heading.current?.focus();
  }, []);
  return (
    <main className="dictation-app" aria-labelledby="settings-recovery-heading">
      <h1 id="settings-recovery-heading" ref={heading} tabIndex={-1}>
        Einstellungen können nicht geladen werden
      </h1>
      <p>
        Die gespeicherten Einstellungen können nicht verwendet werden. Aufnahme und
        Modelleinrichtung bleiben gesperrt, bis du die Einstellungen
        zurücksetzt.
      </p>
      {reason && <p>{reason}</p>}
      <p>
        Beim Zurücksetzen werden Mikrofon, Sprache, Tastenkürzel und
        Modellauswahl auf ihre Standardwerte gesetzt. Bereits geladene
        Sprachmodelle bleiben erhalten.
      </p>
      {error && <p role="alert">{error}</p>}
      {confirming ? (
        <section aria-label="Zurücksetzen bestätigen">
          <p>
            Deine bisherigen Einstellungen werden ersetzt. Möchtest du die
            Standardwerte wiederherstellen?
          </p>
          <div className="app-actions">
            <button
              ref={confirmButton}
              type="button"
              className="primary-action"
              disabled={pending}
              onClick={() => {
                if (!pending) onReset();
              }}
            >
              Zurücksetzen bestätigen
            </button>
            <button
              type="button"
              disabled={pending}
              onClick={() => setConfirming(false)}
            >
              Abbrechen
            </button>
          </div>
          {pending && <p role="status">Einstellungen werden zurückgesetzt …</p>}
        </section>
      ) : (
        <div className="app-actions">
          <button
            ref={resetButton}
            type="button"
            disabled={pending}
            onClick={() => setConfirming(true)}
          >
            Einstellungen zurücksetzen
          </button>
        </div>
      )}
    </main>
  );
}
