# Lokale Entwickler-Teststeuerung

Dies ist eine ausdrückliche Entwicklerschnittstelle, keine Produktschnittstelle
und kein Nachweis physischer/globaler Tastenkürzel. Das Cargo-Feature
`test-control` ist **standardmäßig deaktiviert**. Ohne Feature gibt es keinen
Endpunkt; mit Feature, aber ohne `EVERLAST_TEST_CONTROL_DIR`, ebenfalls weder
Endpunkt noch Abfragethread.

## Gezielt aktivieren

Eine separat gebaute und signierte Entwickler-App mit `test-control` verwenden.
Die normale App-Kopie vor einem Austausch erhalten. Im gestarteten Prozess
`EVERLAST_TEST_CONTROL_DIR` auf ein **vorhandenes kanonisches absolutes
Verzeichnis** desselben Benutzers mit Modus `0700` setzen.

Vor dem Start darin `config.json` als eigene reguläre Datei mit Modus `0600`
und höchstens 256 Byte anlegen:

```json
{"recipientPid":12345}
```

`12345` durch die positive PID eines separat gestarteten eigenen
Testempfängers ersetzen. Unbekannte Schlüssel werden abgelehnt. Die Datei
akzeptiert weder Programm noch Text, Bundle-ID, Mikrofon oder beliebigen Befehl.
Der Server verlangt einen neuen `control.sock`, überschreibt keine Datei und
setzt den Socket auf `0600`. Symlinks und nicht kanonische Verzeichnisse sind
unzulässig.

Beim Aufnahmestart muss die tatsächliche Vordergrund-PID `recipientPid`
entsprechen. Zulässig sind nur diese tatsächlichen Bundle-IDs:

- `de.hoppworks.everlast-native-recipient-hotkey-v1`
- `de.hoppworks.everlast-native-test-recipient`
- `de.hoppworks.everlast-native-autonomous-recipient`

Die konfigurierte PID allein ist keine Zielberechtigung; die native
Prozessidentität wird geprüft. Es wird keine App aktiviert. Einrichtung und
Runtime müssen bereit beziehungsweise idle sein, Mikrofon und Bedienungshilfen
bereits freigegeben, Secure Input inaktiv und das Mikrofon auf
**Systemstandard** gestellt. Die API bietet keine Berechtigungsdialoge,
Systemänderungen, beliebigen Fremdziele, Konfigurationsänderungen, Texteinspeisung
oder allgemeinen UI-/AX-Befehle. Die normalen Aufnahme-/Ziel-/Übergabeprüfungen
bleiben aktiv.

## Historisches Beispiel für Build und Start

Die folgenden Befehle dokumentieren die ursprüngliche Testumgebung vor dem
aktuellen MLX-Paketierungsweg. Insbesondere der frühere Appname und der direkte
Tauri-Build sind keine aktuelle vollständige Hybrid-Buildanleitung.
Der aktuelle normale Build steht unter [Entwicklung](development.md).
Für einen Teststeuerungs-Build müssen zusätzlich Feature, Metal-Bibliothek,
Signatur, Modellcache und bereits gewährte Systemrechte zusammenpassen.

Das Beispiel verlangt einen vorbereiteten Mac. Ein frischer Checkout enthält
weder Zertifikat noch Modelle oder Rechte. Die lokale Signieridentität und
`.everlast-signing.json` müssen vorhanden sein; die Befehle erzeugen keine
Identität und ändern kein Systemvertrauen. Normale `release`-/`release:local`-
Befehle aktivieren `test-control` nicht.

Alle Blöcke liefen aus derselben Shell ab Repositorywurzel. Bei einem Fehler
abbrechen. Eine laufende Everlast-App zuvor über Menü oder **⌘Q** beenden;
Fensterschließen allein gibt den globalen Hotkey nicht frei. Installierte App
und vorhandene ZIPs bleiben erhalten.

Privaten Arbeitsbereich und eigenen Empfänger erzeugen:

```sh
umask 077
repo_root="$(pwd -P)"
mkdir -p "$repo_root/artifacts"
dev_root="$(mktemp -d "$repo_root/artifacts/manual-test-control.XXXXXX")"
CARGO_TARGET_DIR="$dev_root/target" MACOSX_DEPLOYMENT_TARGET=27.0 cargo build --locked --release \
  -p macos-integration --example native_text_recipient --target aarch64-apple-darwin
recipient_app="$dev_root/Everlast Native Recipient.app"
mkdir -p "$recipient_app/Contents/MacOS"
cp "$dev_root/target/aarch64-apple-darwin/release/examples/native_text_recipient" \
  "$recipient_app/Contents/MacOS/native_text_recipient"
cat > "$recipient_app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>de.hoppworks.everlast-native-autonomous-recipient</string>
<key>CFBundleExecutable</key><string>native_text_recipient</string>
<key>CFBundleName</key><string>Everlast Native Recipient</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>27.0</string>
</dict></plist>
PLIST
signing_id="$(node -p 'JSON.parse(require("node:fs").readFileSync(".everlast-signing.json", "utf8")).signingIdentity')"
codesign --force --sign "$signing_id" --timestamp=none --options runtime "$recipient_app"
codesign --verify --deep --strict "$recipient_app"
```

Historische Entwickler-App mit ausdrücklich aktiviertem Feature bauen/signieren:

```sh
CARGO_TARGET_DIR="$dev_root/target" MACOSX_DEPLOYMENT_TARGET=27.0 npm run tauri -- build \
  --bundles app --target aarch64-apple-darwin --features test-control
dev_app="$dev_root/target/aarch64-apple-darwin/release/bundle/macos/Everlast Dictation.app"
node scripts/sign-local-bundle.mjs "$dev_app"
```

Nur den neuen Empfänger starten, seine tatsächliche PID erfassen und danach
die Entwickler-App mit dem Steuerverzeichnis in **ihrer eigenen Umgebung**
starten. Der kurze kanonische `/private/tmp`-Pfad vermeidet das macOS-Pfadlimit
für Unix-Sockets. Er ist neu, privat und enthält nur diese Testkonfiguration:

```sh
EVERLAST_RECIPIENT_REPORT_DIR="$dev_root/recipient-report" \
  "$recipient_app/Contents/MacOS/native_text_recipient" \
  > "$dev_root/recipient.log" 2>&1 &
recipient_pid=$!
control_dir="$(mktemp -d /private/tmp/everlast-control.XXXXXX)"
chmod 700 "$control_dir"
printf '{"recipientPid":%s}\n' "$recipient_pid" > "$control_dir/config.json"
chmod 600 "$control_dir/config.json"
EVERLAST_TEST_CONTROL_DIR="$control_dir" \
  "$dev_app/Contents/MacOS/everlast-dictation" > "$dev_root/app.log" 2>&1 &
dev_pid=$!
```

Auf abgeschlossene Einrichtung und `control.sock` warten, dann Status lesen:

```sh
node scripts/test-control.mjs --control-dir "$control_dir" status
```

Die Variable nur beim **Node-Client** zu setzen aktiviert keinen Endpunkt in
einer laufenden App. Der direkte Programmstart statt Finder/`open` stellt eine
eindeutige PID und die richtige Prozessumgebung sicher. Bei Prozessende private
Logs prüfen, weder PID noch Socket wiederverwenden. Nach dem Test beide Apps
normal beenden; benötigte Nachweise privat erhalten.

Vor einem echten Start in das bearbeitbare Empfängerfeld klicken und dessen
Bericht auf `frontmostIsSelf=true` prüfen. `applicationActive`/`windowKey`
allein reichen nicht. Ein Terminalbefehl verlagert den Vordergrund. Eine kurze
Verzögerung lässt Zeit, das Empfängerfeld vor dem Befehl zu fokussieren:

```sh
(sleep 5; node scripts/test-control.mjs --control-dir "$control_dir" start \
  > "$dev_root/start.json") &
```

Danach nur die tatsächlich gemeldete, nicht leere `controlledSessionId` für
Stop/Cancel verwenden. Keine `sessionId` erraten. Vor beabsichtigter Übergabe
muss erneut der Empfänger fokussiert sein; dieselbe Verzögerung kann für einen
einzelnen `stop SID`-Befehl dienen. Abgelehnte Starts werden nicht automatisch
wiederholt. Dieses Beispiel belegt für sich weder Aufnahme, Fokus noch
Textempfang und ersetzt keinen physischen Tastentest.

## Befehle und Antworten

Jede Verbindung trägt eine UTF-8-JSON-Anfrage mit abschließendem Zeilenumbruch,
höchstens 1.024 Byte einschließlich Umbruch. Es folgen genau eine ebenso
abgeschlossene JSON-Antwort und EOF. Unbekannte Schlüssel und doppelte Felder
werden abgelehnt. `schemaVersion` muss `1` sein.

```json
{"schemaVersion":1,"op":"status"}
{"schemaVersion":1,"op":"start"}
{"schemaVersion":1,"op":"stop","session":7}
{"schemaVersion":1,"op":"cancel","session":7}
```

`session` ist ausschließlich für `stop` und `cancel` erforderlich und positiv;
bei `status` und `start` ist es verboten. Der Node-Client verlangt außerdem
eine in JavaScript exakt darstellbare ganze Zahl.

```sh
node scripts/test-control.mjs --control-dir /absolute/private/directory status
node scripts/test-control.mjs --control-dir /absolute/private/directory start
node scripts/test-control.mjs --control-dir /absolute/private/directory stop 7
node scripts/test-control.mjs --control-dir /absolute/private/directory cancel 7
```

Alternativ liest der Client das Verzeichnis aus `EVERLAST_TEST_CONTROL_DIR`.
`--timeout-ms` hat den Standard 3.000, zulässig sind 10–30.000. Keine automatische
Wiederholung. Ohne Antwort oder bei Verbindungs-Timeout ist das Ergebnis
unbekannt: vor einem weiteren Befehl den Status prüfen.

Erfolg: `{ "schemaVersion":1, "ok":true, "op":"status", "snapshot":{...}, "error":null }`.
Ablehnung: `ok:false`, `snapshot:null` und eine Fehlerzeichenkette; fehlerhafte
Anfragen dürfen `op:"invalid"` liefern. Die CLI bereinigt Serverfehlermeldungen
und endet bei Ablehnung oder Protokoll-/Transportfehler mit einem Fehlercode.

Der Snapshot enthält genau diese skalaren Felder:

| Feld | Bedeutung |
|---|---|
| `setupReady`, `microphoneGranted`, `accessibilityGranted` | Aktuelle Bereitschaft/Rechte, keine Rechteanforderung |
| `phase` | `Idle`, `Starting`, `Recording`, `Finalizing`, `Delivering` oder `Cancelling` |
| `sessionId` | Aktuelle positive Runtime-Sitzungs-ID oder null |
| `controlledSessionId` | ID des tatsächlich angenommenen Starts dieses Endpunkts, sofern sie noch übereinstimmt; sonst null |
| `controlledStartPending` | Die asynchrone Startbestätigung steht aus |
| `outcome` | null, `local`, `empty`, `sent`, `cancelled`, `held` oder `failed` |
| `publicSentenceMatches` | Der Text entspricht nach Zusammenfassen von Leerraum dem festen öffentlichen Satz |
| `transcriptCharacters` | Zeichenanzahl, kein Textinhalt |
| `actionError` | Sichtbarer Aktionsfehler vorhanden, ohne Fehlerinhalt |

Der feste öffentliche Satz lautet: „Forschung im Bereich KI umfasst die
Herstellung von Maschinen zur Automatisierung von Aufgaben, die intelligentes
Verhalten erfordern.“ Es wird weder Rohtranskript noch beliebiger Vergleichstext
angenommen/ausgegeben. Textgleichheit beweist keinen Empfang; dafür ist die
eigene Telemetrie des unabhängigen Empfängers gesondert zu prüfen.

## Besitz und begrenztes Ende

Start ist eine asynchrone Annahme, kein Umschalten. Status abfragen, bis die
**kontrollierte** Sitzungs-ID vorliegt; eine geänderte aktuelle ID allein beweist
keinen Besitz. Der Runtime-Actor bestätigt die ID vor der nächsten Anfrage.
Desktop verwaltet pending/closed/session in einem gemeinsamen Mutex-Zustand.
Ablauf und Bestätigung können sich dadurch nicht verpassen. Shutdown schließt
die Annahme synchron; eine späte erfolgreiche Bestätigung beendet dieselbe Sitzung.

Doppelte/überlappende Starts werden abgelehnt. Ein asynchron abgelehnter Start
löscht pending/deadline und erlaubt einen neuen **ausdrücklichen** Versuch.
Stop/Cancel müssen die eigene ID nennen und zusätzlich die Runtime-Prüfung
gegen veraltete Sitzungen bestehen. Fremde manuell gestartete Sitzungen sind
nicht abbrechbar.

Der Endpunkt endet nach 15 Minuten. Ein angenommener Start hat ab Annahme
60 Sekunden einschließlich der Bestätigungswartezeit. Ablauf schließt weitere
Starts dauerhaft und fordert den Abbruch der eigenen Sitzung an, auch bei
später Bestätigung; reguläres App-Ende ebenso. Dies sind vom Hauptthread und
Eventloop bediente Fristen, keine harten Echtzeit-Unterbrechungen nativer Arbeit.
Nach 60 Sekunden kann der Status bis zum 15-Minuten-Ende lesbar bleiben.
Der Socket wird nur entfernt, solange seine Inode zur eigenen passt. Nach einem
Absturz einen neuen privaten Pfad verwenden, keinen alten Socket überschreiben.

Hintergrund-I/O hat eine 250-ms-Lesefrist für die gesamte Anfrage, einen
begrenzten Kanal mit einer wartenden Anfrage, zwei Sekunden für die
Hauptthreadantwort und 250 ms Schreibfrist. Hauptthreadabfragen warten nicht
auf Clients. Der Start von App oder Endpunkt allein beginnt keine Aufnahme.

## Nachweisgrenze

Transporttests verwenden echte lokale Unix-Sockets für Rechte, Kollisionen,
fehlerhafte Anfragen, Datenschutzprojektion, Sitzungsbesitz, asynchrone
Ablehnung/erneuten Start und begrenzten Ablauf mit kontrolliertem Handler.
Desktop-/Runtime-Tests prüfen Actor-Bestätigung und Shutdown. Das belegt keine
Mikrofonaufnahme, fremde AX-Ziele, physische Tastenkürzel oder Textempfang.
Signierte App-Läufe und ihre Quellen-/Binärbindungen bleiben getrennte Nachweise.

## Nur lesende Feldprüfung

```sh
node scripts/test-control.mjs --control-dir /absolute/private/directory inspect
```

`inspect` akzeptiert weder Sitzung noch Ziel. Es verwendet nur die konfigurierte
PID und prüft davor und danach Kernelidentität sowie eine der drei eigenen
Bundle-IDs. Bedienungshilfen müssen bereits freigegeben sein; Vordergrund ist
nicht erforderlich. Es startet keine Aufnahme, aktiviert kein Fenster, erfasst
kein Übergabeziel, ändert keine Einstellung und erteilt keine Berechtigung.
Diagnoseergebnisse können nicht als spätere Übergabeberechtigung wiederverwendet
werden.

Der Snapshot hat 17 feste skalare Felder: Typ, PID, Rolle mit vier möglichen
Werten, Lesezustände für Rolle/Unterrolle/Aktivierung, ganzzahlige Fehlercodes,
interpretierte secure/enabled-Flags, Ersetzungsfähigkeiten für `AXValue` und
`AXSelectedText` samt Fehlercodes sowie Eignung und Grund. Werte, ausgewählter
Text, Titel und beliebige Zeichenketten werden nicht gelesen. Unbekannte Typen,
Zustände oder Zusatzfelder werden abgelehnt. Die Projektion gibt keine weiteren
internen Felder frei. Lesezugriffe sind begrenzt; die 15-Minuten-Frist bleibt.

Die Produktprüfung akzeptiert eine bekannte gewöhnliche Textrolle mit positiver
Wert- oder Auswahlersetzungsfähigkeit ohne ausdrücklichen deaktivierten Zustand.
Ein beschreibbarer Auswahlbereich allein beweist keine Bearbeitbarkeit.
Rechtefehler, geschützte Felder, Fremdprozesse, ausdrückliche Negativbefunde und
vollständig unbekannte Fähigkeiten bleiben abgelehnt. Die optionale spätere
Produktprüfung des Textempfangs bleibt davon getrennt: ihr Snapshot lebt nur
im AX-Besitzer und wird über diesen Endpunkt nicht ausgegeben.

## Unabhängigen Textempfang bestätigen

Actor-Zustand Idle mit `outcome=sent` ist keine Empfangsbestätigung. Die
250-ms-Veröffentlichung des Empfängers kann noch vor der Actor-Antwort liegen.
Nach einem kontrollierten Lauf wartet diese optionale Lesediagnose auf eine
spätere Sequenz und einen Zeitstempel nach dem Abschluss desselben Empfängers:

```sh
node scripts/recipient-delivery.mjs /absolute/recipient/snapshot.json /absolute/attempt.json
```

Der Versuchsnachweis enthält recipientPid, letzte Empfängersequenz, endedAtUtc
und abgeschlossenen Actor-Snapshot. Der Leser akzeptiert begrenzte eigene
reguläre Dateien und gibt nur feste Diagnosefelder, Grenzen und Hashes aus.
Er startet keine Aufnahme und erteilt keine Zielberechtigung. Benötigte
Telemetrie bytegenau kopieren, da der Herausgeber den Snapshot regelmäßig ersetzt.
Ein [tatsächlich erfolgreicher Lauf](verification/native-delivery-success-2026-10-19.md)
trennt Erkennung des öffentlichen Satzes, einmaligen Textempfang und globale
Tastenkürzel; in diesem Lauf wurden nur die ersten beiden belegt.
