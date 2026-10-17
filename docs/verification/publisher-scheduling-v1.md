# P05: begrenzter Vergleich zweier Publisher-Schedulingklassen

Code `1d4308a`, prospektiver Freeze `7d23293`. Einziger Originalprozess 20310,
Toolhandle 44458, terminal Exit 0; 19:07:49.174892–19:08:49.616634 UTC am
17.10.2026. Keine parallelen Builds, Tests oder Modelle während des Versuchs.

Beide vorab festgelegten Modi haben dieselbe bereits vorhandene 30s-WAV mit
480.000 PCM16-Monoframes bei 16 kHz vollständig durch den Capture-/DSP-Pfad
veröffentlicht. Reihenfolge: USER_INTERACTIVE-QoS, danach feste TC-Anforderung
(20 ms Periode, 500 µs Rechenbudget, 1 ms Constraint, preemptible). Der relative
Schlafpfad sowie 20-ms-Chunks, 10-ms-Toleranz, 1-ms-Maximalschlaf und 250-µs-Spin
sind unverändert. Keine Mikrofonaufnahme und keine Modellinferenz.

| Beobachtung | USER_INTERACTIVE | Time constrained |
|---|---:|---:|
| Erhaltene Publikationen | 1.500 | 1.500 |
| Größte Publikationsverspätung | 267.042 ns | 28.000 ns |
| Größter Publish-Aufruf | 29.250 ns | 27.792 ns |
| Größter Schlaf-Overshoot laut Feedsummary | 873.500 ns | 94.125 ns |
| Stop-Abweichung zur geplanten 30s-Grenze | 25.959 ns | 25.792 ns |
| Thread-CPU zwischen Vor-/Nach-Feed-Abfrage | 465.637.000 ns | 701.939.000 ns |
| Frühe / mehr als 10 ms verspätete Publikationen | 0 / 0 | 0 / 0 |

Beide Ergebnisse sind completed: genau ein Started/Finished und Stop, bestätigte
Startuhr, vollständige Samplegleichheit, Publisher- und Audioowner-Join; keine
OS-, Policy-, Beobachtungs-, Audio- oder Cleanupfehler. Alle 3.000 Publikationen
bleiben privat erhalten. Der [inhaltfreie Bericht](publisher-scheduling-v1.json)
enthält Rohdateihashes, sämtliche Policy-/CPU-Readbacks, vollständige Summaries,
Zeilengrenzen und kanonische Hashes beider Publikationslisten.

Die TC-Abfragen nach Einrichtung, vor/nach Feed und vor Threadende zeigen
übereinstimmende angeforderte Felder, default=false und Basicwert 2. Die
Baseline zeigt Basicwert 1 und Defaultpolicy. Der anfängliche TC-Pre-Policy-
Snapshot zeigt erwartungsgemäß ebenfalls Default/Timeshare. Da kein Timingfehler
auftrat, gibt es keine Outlier-Snapshots. Die sequenziellen Abfragen beweisen
keine kontinuierliche Abwesenheit zwischenzeitlicher Demotion; Basicwert 2 ist
nur mit dem angeforderten Aufbau vereinbar und kein unabhängiger RT-Beweis.

Die CPU-Deltas sind aufsummierte Thread-CPU über das Feedfenster, keine
Periodenmaxima und kein Nachweis, dass jede Periode das angeforderte Rechenbudget
einhält. Der TC-Modus benötigte in diesem Einzelversuch mehr gemessene CPU.
Feste Reihenfolge und je ein kurzer Lauf erlauben keine allgemeine Leistungs-
oder Kausalaussage. Der alte seltene Ausreißer wurde auch in der Baseline nicht
reproduziert; eine Reparatur ist damit nicht bewiesen.

Das Ergebnis erlaubt als nächsten begrenzten Schritt eine explizite Integration
der TC-Alternative in das Entwickler-Messwerkzeug für den vollständigen Runtime-
Pfad. Vor Modellläufen sind dessen Stop-Zeitstempel, Sitzungsidentität, Ready-
Reihenfolge und Ownerende zu prüfen und Binary/Parameter neu einzufrieren.
Produktionspfad und bisherige Replaydefaults bleiben unverändert. Kein L,
keine Korpus-, Installations- oder P05/P06-Gesamtqualifikation.
