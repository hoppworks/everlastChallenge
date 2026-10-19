# A17: übertragbare Quellen und private Pasteboard-Prüfung

Stand: 19.10.2026. [Strukturierte Bindungen](a17-evidence-transfer-2026-10-19.json).

Der zeitnahe Ausführungslog in Commit `e73a9f5` dokumentiert die echte
private Pasteboard-Prüfung einschließlich Text/HTML-Restore, Fremdänderung,
Abbruch und veraltetem Cleanup. Eine gesonderte alte Konsolenausgabe wurde
nicht gefunden. Dieser historische Bericht wird als solcher erhalten und
nicht nachträglich zu einem heutigen Lauf erklärt.

Clipboard-, Paste-, Main-Delivery- und Probequellen sind gegenüber diesem
Commit bytegleich. Alle fünf gebundenen Dateien einschließlich Keyboard sind
gegenüber `f592fcd` und dem geprüften aktuellen `f87943c` bytegleich.
Keyboard unterscheidet sich vom älteren Commit; daraus wird keine historische
Layoutabnahme übertragen.

Im autorisierten technischen Prüfumfang wurde genau einmal die unveränderte
`clipboard_probe` gebaut und ausgeführt, um heutige Rohbelege zu erhalten:

- Build: `cargo build --locked --offline -p macos-integration --example clipboard_probe`, Exit 0.
- Lauf: PID 7322, Exit 0, 1.788 s; Prozess über seinen Handle beendet abgewartet.
- Neues privates `NSPasteboard` mit eindeutigem Namen; keine allgemeine
  Zwischenablage gelesen oder verändert, keine Tastaturereignisse gesendet.
- Text und HTML gesichert/ersetzt/wiederhergestellt; kontrollierte Fremdänderung
  bleibt samt `changeCount` erhalten. Abbruch verhindert Installation. Vorbereitung,
  Busy und veraltetes Cleanup sind geprüft; leeres Board und File-Promise bleiben
  getrennte Fälle.

Ausgabe-SHA256: `3d6d7dc754252bf3ca353cbfb0554185cfa6174968f0d2215dc83b148c6173a8`.
Die neue Konsolenausgabe sowie Build-, Binary- und Quellbindungen liegen unter
`artifacts/a17-current-private-pasteboard-v1/` und im strukturierten Bericht.

Damit ist der **private native Ownership-/Restore-Teil von A17** aktuell belegt.
Es ist kein installierter automatischer Einfügelauf und keine echte Nutzerkopie
während dessen Restore-Fenster. Die allgemeine Zwischenablage, Mikrofon, Modelle,
Rechte und installierte App waren an diesem Lauf nicht beteiligt. Kein vollständiger
P07-/A17- oder Gesamtpaketabschluss wird aus diesem Teilbeleg abgeleitet.
