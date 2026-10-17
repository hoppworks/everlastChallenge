import { useEffect, useRef, useState } from "react";
import { AppView } from "./features/app/AppView";
import { command, connect, sessionCommand } from "./ipc";
import type { Snapshot } from "./ipc";
export function App({ nativeHeader = false }: { nativeHeader?: boolean }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string>();
  const [pending, setPending] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRequest = useRef(0);
  const settingsHeading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (settingsOpen) settingsHeading.current?.focus();
  }, [settingsOpen]);
  useEffect(() => {
    let disposed = false;
    let disconnect: (() => void) | undefined;
    connect(
      (value) => {
        if (!disposed) {
          setSnapshot(value);
          // Consume at the subscription boundary: React may batch several
          // snapshots, including a newer request followed by an older value.
          if (value.settingsRequest > settingsRequest.current) {
            settingsRequest.current = value.settingsRequest;
            setSettingsOpen(true);
          }
        }
      },
      (reason) => {
        if (!disposed) setError(String(reason));
      },
    )
      .then((stop) => {
        if (disposed) stop();
        else disconnect = stop;
      })
      .catch((reason: unknown) => {
        if (!disposed) setError(String(reason));
      });
    return () => {
      disposed = true;
      disconnect?.();
    };
  }, []);
  const run = (action: () => Promise<void>) => {
    setError(undefined);
    setPending((count) => count + 1);
    action()
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setPending((count) => count - 1));
  };
  if (!snapshot)
    return (
      <main className="dictation-app">
        {!nativeHeader && <h1>Everlast Dictation</h1>}
        <p role="status" aria-live="polite">
          Anwendung wird gestartet …
        </p>
        {error && <p role="alert">{error}</p>}
      </main>
    );
  const sessionAction = (name: "recording_stop" | "recording_cancel") => {
    if (snapshot.session.id !== null)
      run(() => sessionCommand(name, snapshot.session.id as number));
  };
  return (
    <>
      <AppView
        nativeHeader={nativeHeader}
        setup={{
          phase: snapshot.setup.phase,
          model: snapshot.setup.model,
          downloadedBytes: snapshot.setup.downloadedBytes,
          totalBytes: snapshot.setup.totalBytes,
          canCancel: snapshot.setup.canCancel,
          ...(snapshot.setup.error ? { error: snapshot.setup.error } : {}),
        }}
        session={{
          phase: snapshot.session.phase,
          text: snapshot.session.text,
          maximumWarning: snapshot.session.maximumWarning,
          ...(snapshot.session.outcome
            ? { outcome: snapshot.session.outcome }
            : {}),
        }}
        permissions={snapshot.permissions}
        labels={snapshot.labels}
        pending={pending > 0}
        settingsBusy={snapshot.settingsBusy}
        {...(error || snapshot.error
          ? { error: error ?? snapshot.error ?? "" }
          : {})}
        onDownload={() => run(() => command("model_download"))}
        onCancelSetup={() => run(() => command("model_cancel"))}
        onStart={() => run(() => command("recording_start"))}
        onStop={() => sessionAction("recording_stop")}
        onCancel={() => sessionAction("recording_cancel")}
        onCopy={() => run(() => command("transcript_copy"))}
        onClear={() => run(() => command("transcript_clear"))}
        onSettings={() => setSettingsOpen(!settingsOpen)}
      />
      {settingsOpen && (
        <section className="dictation-app" aria-label="Einstellungen">
          <h2 ref={settingsHeading} tabIndex={-1}>
            Berechtigungen
          </h2>
          <p>
            Aufnahme und Spracherkennung erfolgen lokal. Zum automatischen
            Einfügen werden zusätzlich die Bedienungshilfen benötigt.
          </p>
          <div className="app-actions">
            <button
              type="button"
              disabled={pending > 0 || snapshot.settingsBusy}
              onClick={() => run(() => command("microphone_request"))}
            >
              Mikrofonzugriff anfragen
            </button>
            <button
              type="button"
              disabled={pending > 0 || snapshot.settingsBusy}
              onClick={() => run(() => command("accessibility_request"))}
            >
              Bedienungshilfen freigeben
            </button>
            <button type="button" onClick={() => setSettingsOpen(false)}>
              Schließen
            </button>
          </div>
        </section>
      )}
    </>
  );
}
