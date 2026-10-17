# Phasenmessung der Replay-Wartezeit

Stand: 17.10.2026; Basis 29e37b8. Entwicklerwerkzeug, keine Produktabnahme.

Nach den ungültigen Echtzeitversuchen v1/v2 erfasst der Replay-Feed zusätzliche
monotone Zeitpunkte: vor/nach Reservierung, Eintritt/Austritt der Wartephase,
letzten Sleep-Aufruf mit Sollzeit sowie Eintritt und größte beobachtete
Zeitlücke im finalen Spin. Die Zusammenfassung zählt sämtliche Publikationen
und hält die größten Phasenintervalle. Einzelheiten werden nur für ungültige
Publikationen gespeichert; die bestehende Grenze von 15.000 Records bleibt.
Der Audio-Probe-Bericht übernimmt die Zusammenfassung auch dann, wenn die
anschließende Beobachtung des Audio-Owners scheitert. Vor einem erfolgreichen
Feed steht dort null, damit fehlende Beobachtung nicht als Nullmessung erscheint.

Absolute Chunkziele, 20-ms-Blöcke, maximal 1-ms-Sleep, 250-µs-Finalspin und
10-ms-Toleranz bleiben unverändert. Keine Dateiausgabe im Feed. Die zusätzlichen
Uhrabfragen ändern den Messaufbau und erfordern einen neuen Freeze. Alte
Fehlversuche werden dadurch weder erklärt noch nachträglich qualifiziert.

Die Intervalle schließen Instrumentierung, Descheduling und Unterbrechungen ein.
Sie lokalisieren beobachtete Verzögerung, beweisen aber keine Kernel-, Timer-
oder Schedulerursache. Ein bestandener kurzer Lauf belegt keine dauerhafte
Timingstabilität. Der modellfreie Audio-Probe verwendet keinen QoS-Override
und ist nicht mit dem vollständigen QoS-Referenzversuch gleichzusetzen.

## Prüfung

Vier injizierte Clock-Diagnosefälle zunächst RED, dann GREEN; vollständige
Audio-Tests 35/35 und Probe-Tests Audio 8/8, Runtime 19/19, Reference 23/23.
Zusätzliche JSON-Regression am Audio-Probe: fehlende Zusammenfassung RED,
nach Übernahme GREEN (8/8). Der Test prüft explizit die Erhaltung nach einem
Finish-Beobachtungstimeout. Unabhängiger Review der Clock ohne Befunde;
Ergänzung des Exportpfads separat nachgeprüft. Ein echter kurzer Lauf wird
mit neu eingefrorenem Build und Eingabe getrennt dokumentiert.
