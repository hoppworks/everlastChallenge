# P05: zentraler Echtzeitlauf v3 – ungültiger Messaufbau

Stand:17.10.2026. Freeze42809f7, Messcode c394d9e, Releasebinary75af1392….
Einziger Originalprozess11721, Unified-Exec19331 terminalExit1;
18:08:22.055363–18:25:42.366526 UTC. Keine parallelen Builds/Tests/Modelle.
Nach terminalem Prozess begannen erst die vorbereiteten kontrollierten Tests.

181 geplante Zeilen:96 erfolgreich (ein Warmup und95 Messungen), eine
invalid_setup und84 unverändert unavailable. Fehler: Englisch5s, Lauf6,
Session97. Alle90 deutschen Messungen und fünf englische5s-Messungen waren
zuvor durchgelaufen. Kein Aggregat, L und Statistik bleiben null. Kein P05-/
P06-Abschluss und keine Qualitätsfreigabe aus einem erfolgreichen Teilblock.

| Erfasste Grenze | Wert |
|---|---:|
| Tatsächliche Publikationen |46.750|
| Frühe Publikationen |0|
| Publikationen über10ms |1|
| Maximale Abweichung |10.234.042ns|
| Größte Publishdauer im ganzen Lauf |132.334ns|
| Maximale Stopabweichung |179.416ns|
| Cleanup, tatsächlicher Join |3.022µs, erfolgreich|

## Neue Phasenevidenz

Die späte Publikation umfasst Frames51520–51840, geplant bei3.240.000.000ns.
Reservierung208ns; letzter Schlaf von3.237.494.334 bis3.250.231.084ns, bei
angeforderten1.000.000ns. Gemessene Schlafintervalllänge12.736.750ns,
Overshoot11.736.750ns. Kein Eintritt in den finalen Spin dieses Waits.
Publishbeginn3.250.231.959ns, Ende3.250.234.042ns, tatsächliche Publishdauer
2.083ns. Die zeitliche Überschreitung liegt schon vor der Publikation.

Diese monotone Wanduhrmessung lokalisiert den langen Abstand um den
Schlafaufruf; sie unterscheidet nicht Kernelweckzeit, Descheduling oder
Unterbrechung unmittelbar vor/nach dem eigentlichen Schlaf. Keine pauschale
Scheduler-/App-Nap-/Timer-Coalescing-Ursachenbehauptung. Öffentliches QoS war
bei allen97 ausgeführten Zeilen eingetreten und wieder beendet, ohne APIfehler.

Der [inhaltfreie Bericht](paced-six-reference-v3.json) bindet vier private
0600-Rohdateien, Freeze/Parameter/Manifest und alle181 Zeilen mit Messwerten
und Wait-Summaries. Unabhängiger vollständiger Readback bestätigte alle46.750
Publikationen,181Slots, tatsächliche Engineframes/RTF, Stop-/Text-/Terminal-
Uhridentität, QoS/Join und alle42 Quellpin-Einträge (40Dateien) aktuell sowie
gegen immutable c394d9e; Binär-, Modell-/VAD-, Smoke- und sechs Audio-/
Referenzbindungen stimmen. Hypothesen bleiben privat, Textqualität unbewertet.

Alte v1/v2-Fehler bleiben erhalten. Kein unveränderter Neustart dieses Laufs.
Als begrenzter nächster Diagnoseversuch kommt ein öffentlicher absoluter
Mach-Wartepfad mit unveränderten20ms/10ms/1ms/250µs-Grenzen infrage; zuerst
kurzer modellfreier Mechanismusvergleich, kein vorweggenommener Fix.
