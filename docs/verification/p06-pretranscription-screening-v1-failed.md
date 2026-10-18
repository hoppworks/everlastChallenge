# P06 – erstes natives Vorabtranskriptions-Screening ungültig

Stand:18.10.2026. **Keine Qualitäts- oder Latenzentscheidung möglich.**
Der Versuch wurde vor dem ersten Candidate-Prozess beendet und wird weder
fortgesetzt noch als erfolgreiche Teilmessung gewertet.

[Prospekt](p06-pretranscription-screening-v1.md),
[Freeze](../../tests/fixtures/corpus/p06-pretranscription-screening-run-v1.json),
[vollständiges Fehlerarchiv](p06-pretranscription-screening-v1-failed.json).
Source4d786a7, Freezecommiteda0fa2, Exec93117 Exit1.

## Erhaltener Umfang

Von400 geplanten Fallprozessen wurden33 im ersten Referenzblock gestartet.
32 bestanden die Ausführungsvalidierung; Fall33 `fleurs-de-test-1693`
scheiterte. Die übrigen367 Prozesse sind ausdrücklich nicht ausgeführt.
Alle800 geplanten Zeilen bleiben erhalten:65 mit Rohstatusok, eine
invalid_setup,734unavailable. Nur64 Zeilen der ersten32 Fälle tragen eine
vollständige erfolgreiche Validierung; der weitere Rohstatusok gehört zum
Warmup des insgesamt gescheiterten33. Falls.

Alle99 Rohdateien und400 Fallabschlüsse sind privat erhalten. Die
ursprünglichen Kommandos, Hashbindungen und Reihenfolge bleiben unverändert.
Kein automatischer Retry, kein Falltausch und kein Candidate-Prozess.
Alle eigenen Prozesse sind beendet; die eigene Wachhalteanforderung ist
aufgelöst. Owner-Join und Aufräumen waren erfolgreich.

## Konkreter Fehler

Fall33: Start04:48:26,494394UTC, Ende05:16:32,630244UTC. Der Warmup war
zeitlich gültig. Im Messlauf verletzten vier Audiopublikationen die feste
10-ms-Grenze. Die erste sollte bei6,180s abgeschlossen sein und erschien
bei6,269922084s:89,922084ms zu spät. Ein angeforderter1-ms-Sleep dauerte
102,807125ms; danach holten drei weitere Publikationen auf und waren noch
69,941667ms,49,950667ms und29,957750ms verspätet.

Reservation und Publikation selbst waren kurz, und die vier gespeicherten
Ausreißer-Snapshots zeigen weiterhin die verlangte TC-Policy. Stop war nur
2625ns verspätet und damit innerhalb des Budgets. Vollständige142080Frames,
gültige Auftragsidentitäten, Endtext und Join bleiben erhalten; sie heilen
die ungültige Eingabezeitachse nicht. Die generische Meldung „input or stop“
bezieht sich hier nachweislich auf Input, nicht auf Stop.

## Unabhängige Systembeobachtung

Das macOS-Powerlog meldet um06:48:35MESZ während dieses Fallprozesses
`Clamshell Sleep` auf Akku, anschließend DarkWake um06:58:30 und07:16:28.
Die eigene caffeinate-Instanz hatte eine aktive PreventUserIdleSystemSleep-
Anforderung. Diese verhindert keinen durch geschlossenen Deckel ausgelösten
Schlaf. Keine globale Energieeinstellung wurde verändert.

Bereits alle33 Hostbeobachtungen vor Modellinitialisierung meldeten
Battery Power. Der Launcher prüfte den Netzbetrieb nicht als Startbedingung.
Das ist ein zusätzlicher Mangel der Versuchsvorbereitung. Die beobachteten
Sleep-/Wake-Ereignisse erklären die erhebliche Wanduhrunterbrechung; eine
exakte Zuordnung eines Powerlog-Zeitpunkts zu einem monotonen Publikations-
Zeitstempel oder eine CPU-/GPU-Fehlerursache wird nicht behauptet.

## Konsequenz

V1 bleibt unverändert als fehlgeschlagener Versuch archiviert. Keine
Qualitätsauswertung aus den32 Fällen als Ersatz für das vollständige Corpus,
keine Latenzfreigabe, keine Ablehnung oder Aktivierung der noch gar nicht
gemessenen Vorabtranskriptionsvariante.

Vor einem neuen, gesondert eingefrorenen Versuch muss der Launcher Netzstrom
vor dem ersten Modellstart und an den Fallgrenzen prüfen und die tatsächliche
Hostbeobachtung berücksichtigen. Akkubetrieb oder fehlende Beobachtung muss
die Ausführung stoppen. Geöffneter Deckel bleibt eine äußere Voraussetzung.
Schwellen, Pausenregeln, Eingaben und Pflichtumfang werden nicht gelockert.
