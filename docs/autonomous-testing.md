# Autonomer Entwicklungs- und Testablauf

## Entwicklung ohne Fenster oder Freigaben

```sh
npm run test:autonomous
npm run test:autonomous:full
```

Das Standardprofil führt alle Rust-Library-Verträge einschließlich der
kontrollierten Aufnahme-/Übergabekette, die UI-Verträge und die Node-Testwerkzeuge
aus. Bis zu drei unabhängige Gruppen laufen parallel. Cargo-Gruppen teilen eine
exklusive Ausführungsspur und konkurrieren nicht um denselben Build-Ordner.
Das vollständige Softwareprofil ergänzt statische Prüfungen, Rust-Integrations-
verträge und Python-Qualitätswerkzeugtests. Native Darstellungstests verwenden dabei ausschließlich unsichtbare eigene Objekte. Der sichtbare Fenster-Lifecycle-Test gehört ausdrücklich nicht in diese Profile. Es startet keine neue Modellmessung.

Beide Befehle brauchen weder eine installierte oder laufende App noch ein
fokussiertes Fenster, Mikrofon, Modellgewichte oder macOS-Freigaben. Es gibt
keine Aufforderung zu einem Tastenkürzel, Klick oder Sprechtest. Der App-Testkanal
ist kein Bestandteil dieser Entwicklungsschleife. `--recipient-report` wird
hier nicht mehr akzeptiert: Eine Fokusvorbedingung darf Softwaretests nicht
blockieren.

```sh
npm run test:autonomous -- --plan
npm run test:autonomous -- --jobs 1
```

`--plan` zeigt ausschließlich die Befehle, ohne Tests oder Änderungen zu starten.
`--jobs 1` führt dieselben Prüfungen nacheinander aus. Die Profile ändern keine
Produktgrenzen. Bei einer fehlgeschlagenen Gruppe starten keine weiteren
Gruppen; bereits laufende Gruppen werden mit ihrem Ergebnis abgeschlossen.
Jede Gruppe bleibt zeitlich begrenzt. Signalabbruch und Timeout räumen die
zugehörigen Kindprozesse auf.

Jeder Lauf erhält einen neuen privaten Ordner unter `artifacts/autonomous-tests/`
mit JSON-Bericht und einzelnen Befehlslogs. Der Bericht bindet das gewählte
Profil, die tatsächlichen Befehle, Node-/Plattformdaten und die versionierten
sowie nicht ignorierten Nicht-Markdown-Dateien an einen SHA256. Er enthält
Exitcodes, Zeitpunkte, Loghashes und ausgeführte Rust-Testzahlen. Ein Cargo-Filter
mit null ausgeführten Tests gilt als Fehler. Nicht gestartete Gruppen bleiben
`skipped`; bestehende Berichte werden nicht überschrieben. Es gibt keine
stillschweigende Wiederholung oder Übernahme alter Resultate.

## Durchgängiger Test mit kontrollierten Betriebssystemeffekten

Der öffentliche JFK-PCM-Fixture läuft durch den echten CaptureHandle,
RecordingOwner/DSP, CpalAudio-Poll-/Joinpfad, Runtime und InferenceService. Ein
kontrollierter Decoder prüft die tatsächlich angekommenen PCM-Daten gegen einen
unabhängig gepinnten Hash. Die native Dienstschleife verarbeitet die wirklichen
Runtime-Kommandos; ein kontrollierter Betriebssystemadapter ersetzt nur das
Startfeld, die Freigabe-/Fokusantworten, die Zwischenablage und den letzten
Einfügeeffekt. Der Test prüft den beobachteten Inhalt des Ziel-Speichers, nicht
nur ein behauptetes `Sent`-Ergebnis.

Die bisherigen einzelnen Modulverträge bleiben bestehen. Der zusätzliche
Verbundtest darf weder `Captured`/`Delivered::Sent` anstelle des Dienstes
vortäuschen noch selbst den erfolgreichen Einfüge-Commit erzeugen. So wird
beispielsweise ein versehentlich entfernter tatsächlicher `send`-Aufruf erkannt,
auch wenn der Runtime-Zustand noch Erfolg meldet.

Das belegt die Softwarekette unter kontrollierten Außenbedingungen. Es belegt
keine reale AX-Erreichbarkeit, physische Hotkeyzustellung, Mikrofonqualität,
Whisper-Erkennungsqualität oder Latenz. Diese Grenzen bleiben ausdrücklich im
Bericht stehen.

## Gesonderte Systemabnahme

Der sichtbare Fenster-Lifecycle-Vertrag ist ausschließlich über `npm run test:os:windows` erreichbar. Dieser bewusst getrennte Befehl zeigt eigene native Fenster; er ist kein Teil des normalen oder vollständigen Softwareprofils.

Reale OS-Adapter werden gezielt und gebündelt geprüft, wenn eine Änderung sie
betrifft oder eine konkrete offene Systemfrage besteht. Derselbe manuelle
Durchstich wird nicht nach jeder Softwareänderung wiederholt. Vorhandene
passende Belege bleiben verwendbar. Standardmikrofon genügt; die vertagten
Anzeigeprüfungen bleiben vertagt. Fehlende OS-Nachweise bleiben offen und werden
nicht durch kontrollierte Tests als erledigt markiert. Sie blockieren weitere
Softwareentwicklung und deren Regressionssuite nicht.

## Weiterarbeiten bis zum gebündelten menschlichen Abschluss

Nach jedem Ergebnis wird die nächste ausführbare technische Aufgabe bearbeitet:
Produktfehler und ihre Regressionen, unabhängiger Review, Dokumentation,
Artefaktvorbereitung oder eine erlaubte native Prüfung. Ein ausstehender
physischer Hotkeynachweis ist keine Startbedingung für diese Aufgaben. Auch
technische Teile nachfolgender Pakete dürfen bei erfüllten fachlichen
Voraussetzungen vorbereitet und geprüft werden; ihr formaler Abschluss folgt
weiterhin den Paketabhängigkeiten.

Die verbleibenden menschlichen Schritte werden für einen gemeinsamen
Abschlussblock vorbereitet:

## Automatischer Zustandsbericht des eigenen Testfensters

Das Entwickler-Example `native_text_recipient` kann optional alle 250 ms
seinen eigenen AppKit-Zustand melden. Die aktivierte Version wurde als separate lokale Test-App tatsächlich geprüft; [der Laufbeleg](verification/native-recipient-observer-2026-10-19.md) bindet Texteingabe, Fokusabweichung, Datei-Rechte und regulären Abschluss. Die ältere Testziel-App enthält diese Ergänzung nicht.
Ohne `EVERLAST_RECIPIENT_REPORT_DIR` gibt es keinen Berichtsordner und keinen
Berichtstimer. Bei Aktivierung muss der absolute Ordner neu sein und sein
kanonischer Elternordner existieren. Die Implementierung legt ihn privat an,
prüft Eigentumsmarker und ersetzt nur ihre eigene `snapshot.json` atomar.

Berichtet werden eigene aktive App, Key-Window, tatsächlicher First Responder,
Zeitstempel, laufender Prozess und ausschließlich Kennzahlen zum fest eingebauten
öffentlichen FLEURS-Satz1517. Kein eingegebener Rohtext und keine Beobachtung
anderer Apps. Nach höchstens 7200 periodischen Beobachtungen endet der Timer;
normaler App-Quit schreibt soweit möglich einen gestoppten Endzustand.

Ein bereits laufendes, separat geprüftes Testziel lässt sich lesend prüfen:

```sh
npm run test:os:readiness -- /absoluter/testordner/snapshot.json
```

Dieser getrennte Diagnosebefehl lehnt fehlende, ungültige, ältere als zwei Sekunden, zukünftige,
gestoppte oder nicht aktiv fokussierte Zustände ab; die Softwaretests sind davon unabhängig. Ein Zeitvorsprung von höchstens 100 ms wird als Uhrtoleranz akzeptiert. Er startet
auch bei gültigem Bericht keine Aufnahme. Der Bericht ist eine eigene
Punktbeobachtung; er beweist weder das von Everlast beim Hotkey erfasste Feld
noch eine spätere automatische Einfügung. Diese Verbindung muss die echte
UI-Abnahme gesondert belegen. Das neue Fenster wird weder per CLI zur Steuerung
anderer Apps noch zum Umgehen von Computersteuerungsbeschränkungen verwendet.

Die Offline-Tests verändern keine Freigaben. Der ursprüngliche Prüfstand war
R6 ohne eingerichtete Signieridentität. Nach ausdrücklicher Freigabe wurde
inzwischen eine lokale Identität eingerichtet und zunächst Build A installiert.
Aktuell läuft der [Entwickler-Testbuild `f592fcd`](verification/test-control-2026-10-19.md)
mit derselben lokalen Identität; R6 bleibt als historisches ZIP verfügbar. Siehe
den [Signierungsbeleg](verification/local-signing-2026-10-19.md).

Die zwei lokal signierten Builds B und A haben eine kompatible Designated
Requirement. Beim tatsächlichen Wechsel blieben beide Rechte ohne erneute
Anfrage erhalten. Dieser Nachweis gilt für diese zwei Builds auf diesem Mac. Das Anlegen
oder Importieren einer Signieridentität benötigt nach dem Projektvertrag eine
gesonderte ausdrückliche Freigabe. Die App-ID allein behebt wechselnde
Ad-hoc-CDHashes nicht. Apples Hintergrund:
[Inside Code Signing: Requirements](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements).

## Rein lokaler Signierweg und vorbereiteter Vergleich

Für zwei anschließend signierte Kandidaten steht eine rein lesende Prüfung bereit:

```sh
node scripts/verify-signing-continuity.mjs /absolut/erster.app /absolut/zweiter.app
```

Beide Signaturen müssen gültig sein; Identifier und nichtleere Designated
Requirement müssen übereinstimmen, der Codehash muss verschieden sein und
Ad-hoc-Signaturen werden abgelehnt. Ein Erfolg bedeutet ausschließlich, dass
der Vergleich für einen tatsächlichen Rechte-Erhaltungstest geeignet ist.
`permissionsPreserved` bleibt im rein statischen Hilfsprogramm immer unbekannt.
Die getrennte reale Rechtebeobachtung steht im Signierungsbeleg. Der lokale Weg ersetzt
keine Developer-ID-Distribution oder Notarisierung.

## Lokale Builds nach der einmaligen Einrichtung

```sh
npm run release:local
```

Der Befehl liest den lokal eingerichteten Zertifikatsfingerprint aus der
Git-ignorierten `.everlast-signing.json`, baut ausschließlich die Apple-Silicon-
App und signiert sie mit dieser Identität. Private Schlüssel bleiben im
Login-Schlüsselbund. Apple-Konto-/Notarisierungsvariablen werden dem lokalen
Build nicht mitgegeben. Der Befehl installiert oder startet die App nicht.

Vor der Signierung werden die erwartete App-ID, der eine Mach-O-Hauptprozess,
Entitlements und das Fehlen zusätzlicher ausführbarer Bundles geprüft. Vorher
und nachher folgt die Signatur-/Mikrofondeklarationsprüfung. Ohne gültige lokale
Konfiguration stoppt der Befehl vor Build und Signierung. Für einen bereits
gebauten Kandidaten kann `node scripts/sign-local-bundle.mjs /absolut/Build.app`
verwendet werden. Es werden keine Vertrauenseinstellungen gesetzt.
