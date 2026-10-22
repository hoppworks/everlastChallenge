# Autonomer lokaler Testzugang, 19.10.2026

Ein lokaler Entwicklerzugang ersetzt den
physischen Tastendruck für wiederholte Funktionstests. Der installierte Testbuild
aus `f592fcd` wurde mit `test-control` und derselben lokalen Signieridentität
gebaut. Normaler Produktionsbuild und regulärer Hotkey bleiben unverändert;
der neue Zugang ist standardmäßig nicht einkompiliert.

## Tatsächlich geprüft

- Unix-Socket-Transport und CLI wurden mit echten lokalen Socketverbindungen
  geprüft. Fehler, unzulässige Nachrichten und Antwortgrenzen werden abgefangen.
- Der Runtime-Aktor quittiert die exakte von ihm erzeugte Sitzung. Ein zweiter
  Start während derselben Sitzung stoppt diese nicht versehentlich.
- Sitzungsbesitz, ausstehender Start und Ablauf werden unter einem gemeinsamen
  Mutex beobachtet. Verspätete Startquittung und Ablauf können den Abbruch nicht
  gegenseitig übersehen. Eine abgelehnte asynchrone Startanfrage gibt den Zugang
  für einen bewussten neuen Versuch frei.
- Der unabhängige Review fand drei entsprechende Lifecycle-/Nebenläufigkeits-
  Probleme; alle wurden korrigiert und gezielt nachgeprüft.
- `npm test`, normale statische Prüfung und Clippy mit `test-control` bestanden.
  Hinzu kommen vier Desktop-, sechs Transport-, acht CLI-Tests und der konkrete
  Runtime-Test für Sitzungsquittung und doppelten Start.
- Die alte, per UI nicht mehr erreichbare Testinstanz wurde über Activity
  Monitor mit **Quit** regulär beendet; kein Force Quit. Ihr Ende wurde am PID
  geprüft, bevor der neue signierte Build installiert wurde.
- Im laufenden installierten Testbuild antwortet der echte Steuerkanal.
  Status meldet Modell bereit, Mikrofon und Accessibility freigegeben.
  Keine neue Freigabe war erforderlich.

## Noch keine erfolgreiche automatische Übergabe

Zwei ausdrücklich ausgelöste Startversuche wurden vor der Mikrofonaufnahme
abgelehnt: Das konfigurierte Testziel war nach der nativen Vordergrundprüfung
nicht das aktive Ziel. Der neue eigene Testempfänger macht die Abweichung
sichtbar: AppKit kann `applicationActive=true` und `windowKey=true` melden,
während `NSWorkspace` gleichzeitig `frontmostIsSelf=false` meldet.
Die Vorprüfung verlangt nun ausdrücklich auch `frontmostIsSelf=true`.

Ein eigener nativer Knopf kann die Aktivierung anfordern, ohne fremde Apps oder
Text zu lesen. Eine sichere tatsächliche Vordergrundzuordnung ist dadurch auf
diesem Desktop bislang nicht belegt. Daher ist einmalig ein echter Klick in das
eigene Testfeld angefordert; **kein Tastenkürzel** und keine neue OS-Freigabe.
Bis dieser Zustand tatsächlich beobachtet wird, bleibt die Aufnahme aus.

Die Schnittstelle ermöglicht Start, Stopp, Abbruch und Status ohne physisches
Kürzel. Der erfolgreiche vollständige Aufnahme-/Einfügelauf sowie der separate
echte OS-Hotkey-Nachweis bleiben offen. Dieser Befund schließt kein Paket ab und
senkt keine Abnahmegrenze.

Rohbelege: `artifacts/test-control-v1/installation.json`,
`artifacts/test-control-v1/start-rejected.json`,
`artifacts/offline-continuation-v1/test-control-*.log` und der laufende eigene
Empfängerbericht unter `artifacts/recipient-automation-v1/live/snapshot.json`.
Der Bericht enthält nur eigene Metadaten und Kennzahlen zum festgelegten
öffentlichen Testsatz; keinen gesprochenen Rohtext.

[Strukturierter aktueller Nachweis](test-control-2026-10-19.json) bindet
installierten Binaryhash, Testprotokolle und tatsächliche Statusantwort.

## Spätere lokale Tests ohne physischen Tastendruck

Danach wurde der [native lokale Aufnahmeablauf](native-local-control-2026-10-19.md)
ohne Nutzeraktion ausgeführt: Start, Stopp mit leerem Ergebnis, erneuter Start
und Abbruch. Zusätzlich wies der Testzugang den Abbruch einer nicht von ihm
gestarteten echten Aufnahme zurück. Die vorherigen Aussagen „keine Aufnahme“
beziehen sich auf die beschriebenen abgelehnten externen Startversuche.
Die automatische externe Übergabe ist weiterhin nicht belegt.
