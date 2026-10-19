# Autonomer Entwicklungs- und Testablauf

## Ein Befehl ohne macOS-Freigaben

Aus dem Repository-Root:

```sh
npm run test:autonomous
```

Der Einstieg führt statische Prüfungen, Kernverhalten, Ziel-Eignungsverträge,
Runtime-Verbundtests, kontrollierte native Übergabeverträge sowie Replay-PCM-
und Stopp-Verträge aus. Er benötigt weder Mikrofonaufnahme noch Modellgewichte,
App-Installation oder Accessibility-Freigabe. `measurement` aktiviert hier nur
explizit ausgewählte Testpfade; ignorierte Modelltests werden nicht gestartet.
Kein mehrstündiger Qualitätslauf und keine Änderung der ausgelieferten Referenz.

Jeder Lauf erhält einen neuen privaten Ordner unter `artifacts/autonomous-tests/`
mit JSON-Bericht und einzelnen Befehlslogs. Der Bericht bindet die Befehle,
Node-/Plattformdaten und den Inhalt der versionierten sowie nicht ignorierten
Nicht-Markdown-Dateien an einen SHA256. Er enthält tatsächliche Exitcodes,
Zeitpunkte, Loghashes und ausgeführte Rust-Testzahlen. Fehlgeschlagene Schritte
stoppen die Folge; nachfolgende Schritte bleiben ausdrücklich übersprungen.
Ein Cargo-Filter mit null ausgeführten Tests gilt als Fehler. Alte Berichte
werden nicht überschrieben. Es gibt keine automatische Wiederholung und kein
stillschweigendes Umdeuten alter Ergebnisse zu neuen Nachweisen.

Der öffentliche JFK-Fixture-Verbund benutzt den echten CaptureHandle,
RecordingOwner/DSP, CpalAudio-Poll-/Joinpfad, Runtime und InferenceService. Die
PCM-Prüfung erfolgt im kontrollierten Decoder gegen einen unabhängig gepinnten
Hash. Er prüft Stop und genau eine Übergabe mit Originaltoken, Abbruch mit
verspätetem Decoderergebnis sowie widerrufene Übergabe bei abgelehntem Startziel.
Decoder und nativer Endpunkt bleiben kontrolliert: kein Whisper-Qualitäts-,
Latenz-, Mikrofon-, OS-Ziel- oder tatsächlicher Einfügenachweis daraus.

## Automatischer Zustandsbericht des eigenen Testfensters

Das Entwickler-Example `native_text_recipient` kann optional alle 250 ms
seinen eigenen AppKit-Zustand melden. Die aktivierte Version ist noch nicht
als App installiert; die vorhandene Testziel-App enthält diese Ergänzung nicht.
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
npm run test:autonomous -- --recipient-report /absoluter/testordner/snapshot.json
```

Der Einstieg lehnt fehlende, ungültige, ältere als zwei Sekunden, zukünftige,
gestoppte oder nicht aktiv fokussierte Zustände vor dem Testlauf ab. Ein Zeitvorsprung von höchstens 100 ms wird als Uhrtoleranz akzeptiert. Er startet
auch bei gültigem Bericht keine Aufnahme. Der Bericht ist eine eigene
Punktbeobachtung; er beweist weder das von Everlast beim Hotkey erfasste Feld
noch eine spätere automatische Einfügung. Diese Verbindung muss die echte
UI-Abnahme gesondert belegen. Das neue Fenster wird weder per CLI zur Steuerung
anderer Apps noch zum Umgehen von Computersteuerungsbeschränkungen verwendet.

Die installierte R6 bleibt unverändert. Vorhandene Freigaben werden für diese
Offline-Tests nicht angefasst. Die lesende Signieridentitätsprüfung am
19.10.2026 ergab null gültige Codesigning-Identitäten; es wurde kein Zertifikat
erzeugt, importiert oder im Schlüsselbund verändert.

Für zukünftige installierte Builds ist eine konstante Signieridentität mit
kompatibler Designated Requirement vorgesehen. Erst ein tatsächlicher
Zwei-Build-Test mit erhaltenen Freigaben belegt deren Beständigkeit. Das Anlegen
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
`permissionsPreserved` bleibt immer unbekannt, bis der echte App-Wechsel mit
Rechtebeobachtung ausgeführt wurde. Der vorbereitete lokale Weg ersetzt
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
