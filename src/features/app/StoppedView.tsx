/** A terminal owner cannot accept setup or session commands. Keep its last text usable. */
export function StoppedView({
  text,
  cause,
  actionError,
  pending,
  onCopy,
}: {
  text: string;
  cause: string | null;
  actionError: string | undefined;
  pending: boolean;
  onCopy: () => void;
}) {
  return (
    <main className="dictation-app" aria-label="Everlast Coding Challenge">
      <h1>Anwendung wurde beendet</h1>
      <p>Bitte beende die App und öffne sie erneut.</p>
      {cause && <p role="alert">{cause}</p>}
      <section className="transcript-section" aria-labelledby="stopped-text">
        <h2 id="stopped-text">Letzter Text</h2>
        <textarea aria-labelledby="stopped-text" value={text} readOnly />
        <button type="button" onClick={onCopy} disabled={!text || pending}>
          Kopieren
        </button>
        {actionError && <p role="alert">{actionError}</p>}
      </section>
    </main>
  );
}
