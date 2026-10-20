import { useEffect, useRef, useState } from "react";
import { AppView } from "./features/app/AppView";
import {
  command,
  connect,
  sessionCommand,
  saveSettings,
  clearTranscript,
} from "./ipc";
import { SettingsView } from "./features/settings/SettingsView";
import { RecoveryView } from "./features/settings/RecoveryView";
import type { Snapshot } from "./ipc";
export function App({ nativeHeader = false }: { nativeHeader?: boolean }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string>();
  const [pending, setPending] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRequest = useRef(0);
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
  if (snapshot.preferences.recoveryRequired)
    return (
      <RecoveryView
        reason={snapshot.preferences.error}
        pending={pending > 0}
        {...(error ? { error } : {})}
        onReset={() => run(() => command("settings_reset"))}
      />
    );
  const sessionAction = (name: "recording_stop" | "recording_cancel") => {
    if (snapshot.session.id !== null)
      run(() => sessionCommand(name, snapshot.session.id as number));
  };
  return (
    <>
      {!settingsOpen && (
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
            maximumReached: snapshot.session.maximumReached,
            elapsedSeconds: snapshot.session.elapsedMillis / 1000,
            level: snapshot.session.levelPermille / 1000,
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
          onClear={() => run(() => clearTranscript(snapshot.session.text))}
          onSettings={() => setSettingsOpen(true)}
        />
      )}
      {settingsOpen && (
        <SettingsView
          preferences={snapshot.preferences}
          {...(error || snapshot.error
            ? { error: error ?? snapshot.error ?? "" }
            : {})}
          permissionsBusy={pending > 0 || snapshot.session.phase !== "Idle"}
          setup={{
            phase: snapshot.setup.phase,
            model: snapshot.setup.model,
            ...(snapshot.setup.error ? { error: snapshot.setup.error } : {}),
          }}
          permissions={snapshot.permissions}
          busy={
            pending > 0 ||
            snapshot.settingsBusy ||
            snapshot.session.phase !== "Idle" ||
            ["Checking", "Downloading", "Loading", "Cancelling"].includes(
              snapshot.setup.phase,
            )
          }
          onSave={(draft) => run(() => saveSettings(draft))}
          onRefresh={() => run(() => command("devices_refresh"))}
          onMicrophone={() => run(() => command("microphone_request"))}
          onAccessibility={() => run(() => command("accessibility_request"))}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </>
  );
}
