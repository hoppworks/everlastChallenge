# Zeitlich begrenzte automatische Prüfläufe

`scripts/overnight-runner.mjs` führt ausschließlich die in einem JSON-Plan
angegebenen Befehle aus. Das Werkzeug sucht keine Tests, startet keine App,
fordert keine Tastendrücke an, nimmt kein Audio auf, lädt keine Modelle und
ändert keine Berechtigungen. Fehler werden nicht automatisch wiederholt.
Benötigt werden Node und Prozessgruppen unter macOS oder Linux. Die eigenen
Tests verwenden kurze, harmlose Node-Kindprozesse:

```sh
node --test scripts/overnight-runner.test.mjs
node scripts/overnight-runner.mjs --plan /absolute/plan.json --state /absolute/private-run-state
```

Befehle müssen ohne Interaktion ausführbar sein. Der Runner schließt stdin und
öffnet keine Shell. Er koordiniert Prozesse, ist aber keine Sandbox: Nur
vertrauenswürdige Programme und Skripte eintragen. Ein Berechtigungsdialog wird
dadurch nicht unbeaufsichtigt bedienbar. Bei Zeitüberschreitung beendet der
Runner die eigene Prozessgruppe; unabhängige Arbeit kann weiterlaufen.

## Planformat

```json
{
  "schemaVersion": 1,
  "deadline": "2026-10-21T07:00:00Z",
  "jobs": [
    {
      "id": "runner-contract",
      "argv": ["/absolute/path/to/node", "--test", "scripts/overnight-runner.test.mjs"],
      "cwd": "/absolute/path/to/checkout",
      "timeoutMs": 30000,
      "dependsOn": [],
      "inputs": ["scripts/overnight-runner.mjs", "scripts/overnight-runner.test.mjs"],
      "outputs": [],
      "resourceGroup": "software"
    }
  ]
}
```

Das Beispiel ist eine Vorlage, kein ausführbarer Messauftrag. Pfade ersetzen,
eine zukünftige Frist wählen und die genauen begrenzten Befehle angeben. Die
Beispielfrist entspricht dem 21.10.2026 um 09:00 Uhr in Europe/Berlin.

Jeder Auftrag benötigt eine eindeutige sichere ID, einen absoluten Programmpfad,
ein vorhandenes absolutes Arbeitsverzeichnis, ein positives Zeitlimit,
Abhängigkeiten, mindestens eine reguläre Eingabedatei und eine ausdrückliche
Ausgabeliste; diese darf leer sein. Ein- und Ausgabepfade sind relativ zum
Arbeitsverzeichnis oder absolut. Verzeichnisse und automatische Quellsuchen
werden nicht unterstützt. Alle relevanten Skripte, Quellen, Konfigurationen,
Toolchain-Pins, Fixtures, Modelle und Bibliotheken unter `inputs` aufführen;
später benötigte Ergebnisse unter `outputs`. Eine unvollständige Liste belegt
keine vollständige Abdeckung. Eine Manifestdatei bindet nicht automatisch die
darin referenzierten Dateien.

`env` ergänzt Zeichenkettenwerte in der geerbten Umgebung. Die gesamte wirksame
Umgebung fließt in den Fingerabdruck ein, ihre Werte stehen nicht im Journal.
Befehle und Logs können vertrauliche Inhalte enthalten; dafür ein privates
Ausgabeverzeichnis verwenden und Geheimnisse aus argv fernhalten.
`resourceGroup` ist eine optionale beschreibende Bezeichnung.

**Alle Aufträge laufen seriell**, einschließlich Cargo- und GPU-Aufträgen.
Externe Builds und Messungen müssen zusätzlich angehalten werden; der Runner
besitzt keine fremden Prozesse. Getrennte Zustandsverzeichnisse sind unabhängig
und dürfen nicht für konkurrierende Messkampagnen verwendet werden. Eine
Sperrdatei sichert den exklusiven Besitz eines Zustandsverzeichnisses.

Abhängigkeiten werden vor ihren Verbrauchern eingeordnet. Fehlende
Abhängigkeiten, Zyklen, doppelte oder unsichere IDs, Shells, ungültige Zeitlimits
und unbekannte Felder werden vor dem Start abgelehnt. Fehlende Dateien oder
fehlgeschlagene Starts betreffen den jeweiligen Auftrag und seine Folgeaufträge.

## Nachweise und Fortsetzen

Das Zustandsverzeichnis enthält unveränderliche `plan-<sha256>.json`-Kopien,
eine aktuelle `summary.json`, während des Laufs `runner.lock` und für jeden
Versuch ein eigenes Verzeichnis mit `result.json` und `output.log`.
Vor dem Start wird ein laufender Versuch gespeichert, nach dem Start zusätzlich
PID und Prozessgruppenidentität. Abgeschlossene Nachweise enthalten argv,
Arbeitsverzeichnis, Zeitlimit, Zeitstempel, Abhängigkeiten, Dateihashes,
Fingerabdruck, PID, Exitcode/Signal, Aufräumstatus, Loghash und bei Erfolg die
Ausgabehashes. Zusammenfassungen werden atomar ersetzt; frühere Versuche bleiben.

Zum Fortsetzen denselben Befehl und dasselbe Zustandsverzeichnis verwenden.
Erfolge werden nur wiederverwendet, wenn Auftragsdefinition, Dateibytes,
aufgelöste Pfade, Dateimodi, Programm, Umgebung, Runner, Plattform/Architektur
und Abhängigkeitsfingerabdrücke einschließlich Ausgaben übereinstimmen. Logs
und Ausgaben müssen mit passenden Hashes vorhanden sein. Änderungen erzeugen
einen neuen Versuch, ohne alte Ergebnisse zu ersetzen. Verändert ein
scheinbar erfolgreicher Befehl eine deklarierte Eingabe oder fehlen Ausgaben,
gilt er als fehlgeschlagen.

Unveränderte Fehler, Zeitüberschreitungen und sauber unterbrochene Versuche
werden nicht automatisch wiederholt. Zunächst Ursache klären und betroffene
Eingaben oder Befehle ändern. Fehlende Nachweise zu einem unveränderten Fehler
müssen untersucht werden. Laufende oder prüfbedürftige Versuche werden nicht
blind neu gestartet; eine vorhandene Sperre wird nicht übernommen. Nach einem
ungewöhnlichen Runner-Ende sind Besitzer, Kindprozesse und fachlicher
Sitzungszustand zu prüfen. Journal oder PID allein beweisen keine Beendigung.
Erst nach bestätigtem Prozessende und geprüftem Aufnahme-/Controllerzustand den
alten Zustand archivieren und einen neuen Versuch vorbereiten.

## Zeitlimits und Aufräumen

Ein Auftrag startet nur, wenn sein vollständiges Zeitlimit plus eine Sekunde
Aufräumreserve vor die absolute Frist passt. Nach dem Hashen wird erneut
geprüft. Hashes werden mit begrenztem Puffer und Fristprüfung berechnet.
Bei Zeitüberschreitung erhält nur die neue eigene Prozessgruppe SIGTERM,
nach 150 ms SIGKILL; die Bestätigungsfrist beträgt 500 ms. Unerwartete
Nachfahren nach erfolgreichem Ende des Elternprozesses werden beendet und als
Fehler erfasst. SIGINT/SIGTERM beenden die aktuelle Gruppe und verhindern weitere
Starts. Unbestätigtes Aufräumen verhindert Folgearbeit. Bei Systemschlaf oder
einem blockierten Betriebssystem ist kein exakter Endzeitpunkt garantiert.

Kindprozesse müssen in ihrer eigenen Prozessgruppe bleiben. Keine Daemons
starten und Ausführung nicht per `open` an LaunchServices abgeben: Solche
Prozesse benötigen einen eigenen kontrollierten Lebenszyklus.

## Native Prüfblöcke und physische Tastenkürzel

Der Runner erweitert das native Controllerprotokoll nicht. Ein nativer
Prüfblock braucht einen eigenen begrenzten Wrapper mit frischem privaten
Steuerverzeichnis, eigenem Empfänger und Entwicklerprozess, Bereitschafts- und
Fokusprüfung, bestätigter Sitzungs-ID, frischen Empfängerbeobachtungen und
explizitem Aufräumen. Die Teststeuerung endet nach 15 Minuten, eine angenommene
Aufnahme spätestens nach 60 Sekunden und der Empfängerbericht nach 30 Minuten.
Vor diesen Grenzen abschließen; für einen neuen Block frische Prozesse starten.
Keine 300-Sekunden-Aufnahme über den 60-Sekunden-Kanal schicken. Ein unbekanntes
Startergebnis erfordert eine Statusprüfung vor einem weiteren Start.

Der kontrollierte Ablauf belegt Softwareverhalten, keine physische
Tastaturzustellung, Rechte einer neuen App-Kopie oder Geschwindigkeitsvorteile
zwischen Apps. Fehlende Voraussetzungen werden als blockiert dokumentiert;
unabhängige Softwareprüfungen können fortgesetzt werden. Ein physischer
Tastentest benötigt eine eigene Beobachtung.

## Ein vorbereiteter nativer Kurztest

`scripts/controlled-native-smoke.mjs` führt einen begrenzten Prüfblock mit einer
**bereits vorbereiteten** Entwickler-App und einem eigenen Empfänger aus.
Es startet keine der Apps, ändert weder Fokus noch Rechte oder Einstellungen
und akzeptiert kein beliebiges Ziel oder Freitext:

```sh
node scripts/controlled-native-smoke.mjs \
  --control-dir /private/tmp/own-fresh-control \
  --recipient-report /absolute/own-recipient/snapshot.json \
  --fixture /absolute/corpus/audio/fleurs-de-dev-1517.wav \
  --output-dir /absolute/private-evidence/new-smoke \
  --endpoint-started-at 2026-10-20T22:00:00Z
```

Den Zeitstempel durch den tatsächlichen Startzeitpunkt des frischen Endpunkts
ersetzen; nach zwölf Minuten wird er abgelehnt. Das Elternverzeichnis muss
kanonisch und vorhanden, das Ausgabeverzeichnis neu sein. Konfiguration,
Empfängerbericht und Audio müssen private reguläre Dateien desselben Benutzers
sein. Das kanonische private Steuerverzeichnis enthält nur die Empfänger-PID
in seiner Konfiguration. Die serverseitigen AX-Prüfungen bleiben maßgeblich.
Vor dem Start werden ein frischer Bericht ohne vorhandene Vorkommen,
tatsächlicher Vordergrund-/Fenster-/Feldfokus und eine passende aktive PID verlangt.

Akzeptiert wird ausschließlich der historische öffentliche deutsche
FLEURS-Satz 1517, Länge 10,56 Sekunden, SHA-256
`b892cf9dc80c437f0a78c6f261d83dda196252e31a1e415032981472d3ea9137`.
Die Datei liegt relativ zum Repository unter
`artifacts/corpus/audio/fleurs-de-dev-1517.wav`. Implementierungstests erfassen
kein Audio. Ein tatsächlicher CLI-Lauf startet den Systemstandard-Mikrofonpfad
der vorbereiteten App und spielt die geprüfte öffentliche Datei mit
`/usr/bin/afplay` ab.

Der Treiber sendet genau einmal `start-host`, verlangt Bestätigung und passende
Sitzungs-ID im Zustand Recording, spielt die Datei ab, stoppt ausdrücklich
diese Sitzung und wartet auf Idle sowie eine spätere unabhängige
Empfängerbeobachtung mit genau einem Vorkommen. `sent` allein reicht nicht.
40 Sekunden Arbeitszeit plus höchstens fünf Sekunden Aufräumen bleiben unter
der nativen 60-Sekunden-Grenze. Transport, Wiedergabe, Zustandswarten und
Empfängerlesen sind begrenzt. SIGINT/SIGTERM stoppen die Wiedergabe und räumen
nur eine bestätigte passende Sitzung auf. Eine verlorene Startantwort wird als
unbekannt/blockiert erfasst, nicht wiederholt, übernommen oder blind abgebrochen.

Der äußere Runner muss die vollständigen 45 Sekunden plus Start-/Aufräumreserve
zulassen. Ein vorzeitiges SIGKILL des Clients beweist kein Aufräumen der separat
vorbereiteten App. Nach ungewöhnlichem Client-Ende deren Zustand gesondert
prüfen; die nativen Fristen bleiben bestehen.

`result.json` hält Start-/Wiedergabe-/Endgrenzen, Statusereignisse, Sitzungs-ID,
Aufräumstatus und Quellen-/Konfigurations-/Fixturebindungen fest. Empfängerbytes
werden separat nummeriert gespeichert, bevor periodische Veröffentlichungen
sie ersetzen. Ein echter erfolgreicher Lauf belegt Mikrofon und kontrollierte
Übergabe mit `physicalHotkey=false`; injizierte Transporttests belegen nur die
Treiberlogik. Fehlende Voraussetzungen ergeben ein blockiertes Ergebnis oder
einen CLI-Fehler, keine Aufforderung zu Klicks, Sprache oder Tastendrücken.
