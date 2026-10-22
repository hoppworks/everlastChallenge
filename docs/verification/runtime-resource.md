Aktueller Launcher: `scripts/run-runtime-resource.ts`, ausgeführt mit
`node --import tsx`. Der folgende Bericht und seine Kommandos bleiben historische
Belege. Für einen neuen Lauf zuerst die Parameter des aktuellen Probe-Binaries
mit `--print-parameters` beziehungsweise
`--print-mlx-reference-encoder-parameters` in eine neue Datei schreiben; alte
Parameter passen absichtlich nicht zu geändertem Code. Siehe
[Entwicklerwerkzeuge](../tooling.md).

# Wiederholte Runtime-Sitzungen und Speicherbeobachtung

Stand: 17.10.2026. Werkzeugbasis `d1a2527`, erster tatsächlicher Modelllauf
mit dem unveränderten Werkzeugcommit `25e8e9d`. Der Lauf ist unten getrennt
vom vorbereitenden Werkzeugnachweis dokumentiert; Ressourcenqualifikation und
vollständige Produktabnahmen bleiben offen.

Die feste Entwicklermessung verwendet eine einmal geladene Engine,
den seriellen Inferenzdienst und die echte Runtime samt Replayquelle vor Ring
und DSP. Fünf vollständige Stop-Sitzungen wärmen Modell und Pipeline auf.
Danach folgen exakt 100 Zyklen in der vorab festgelegten Reihenfolge
StopComplete, CancelCapture, CancelDecoderEntry (34/33/33).

StopComplete verarbeitet die ganze ausdrücklich ausgewählte Fixture; der
Text muss exakt der vorher festgehaltenen Modellbaseline entsprechen und wird
nicht ausgegeben. CancelCapture endet nach dem ersten 20-ms-Chunk.
CancelDecoderEntry hält einen Wrapper vor dem eigentlichen Engineaufruf,
widerruft die Sitzung, gibt den Aufruf frei und wartet dessen Ende ab.
Das belegt keine Unterbrechung eines bereits laufenden nativen Whisperkernels.
Cancelled bleibt ohne übernommenen Text. Jeder vollständige Decoderaufruf
vergleicht seine tatsächlich übergebenen Samples mit einer gemeinsam gehaltenen,
unveränderten Fixture. Diese zusätzliche konstante Fixture gehört zum gemessenen
Footprint. Für Capture-Cancel entsteht kein solcher Decodervergleich; daraus
wird keine Samplegleichheit dieses abgebrochenen Aufnahmeteils behauptet.

RSS wird nach Controller-Idle und beobachtetem Wrapper-Idle abgefragt.
Diese Beobachtung ist keine atomare Aussage über freigegebene Job-lokale Arcs
oder Allocator-Caches. Der nächste akzeptierte Messstart bestätigt durch den
Actor die Freigabe von Audioowner und Inferenzdienst; nach dem letzten
StopComplete folgt der echte Runtime-Join. Sämtliche Fehler und verbleibenden
nicht ausgeführten Zyklen werden erhalten, nicht zu erfolgreichen Samples
umgedeutet.

Das Werkzeug erfasst RSS in Byte und an zwei äußeren Messpunkten eine begrenzte
VM-Zusammenfassung. RSS ist kein Live-Heapmaß; Metal-/Allocator-Caches
und OS-Verhalten gehören zur Beobachtung. Rohwerte, Verhaltensprüfung und Einspeisejitter werden getrennt ausgewiesen.
Eine überschrittene Jittertoleranz bleibt im Setupstatus sichtbar, auch wenn
Ressourcen-/Cancelverhalten korrekt beobachtet wurden. Dieser Lauf liefert
keinen L-Vergleich und keinen vollständigen L-Rohdatensatz. Ein frei erfundener Wachstumsgrenzwert ersetzt weder
Leakanalyse noch die geforderte Ressourcenabnahme. Die eingebettete allgemeine
Runtimebaseline dient der Parameter-/Quellprovenienz; ihr Latenzprotokoll wird
hier nicht ausgeführt. Maßgeblich ist die eigene Ressourcen-Workload.

## Werkzeugnachweise vor dem Modelllauf

Die Standard- und Feature-Workspacechecks sowie Tests sind grün. Das neue
Example hat 20 bestandene Tests einschließlich tatsächlicher kontrollierter
Runtime-Zyklen, Busy-Start-Antworten, Samplemismatch trotz unverändertem Text,
widerrufener Decoderfehler, atomarer privater Berichtspublikation,
Parametervalidierung und OS-Metrikhelfer. Der Parametervalidator wurde zuvor
an der Annahme einer gültigen eingefrorenen Konfiguration rot nachgewiesen.

`scripts/run-runtime-resource.py` hält vor dem Kindprozess OS/Build, CPU/RAM,
Toolchain, Energiequelle, genaue Kommandozeile und Binary-/Parameter-/Fixture-/
Referenzhashes in einer privaten exklusiven `.host.json` fest. Es startet den
Probe über `/usr/bin/time -l`; Fortschritt und OS-Ressourcenstatistik bleiben
in `.command.log`. `.completion.json` verbindet Exitcode und Berichthashes.
Der Launcher gehört zu den vorab eingefrorenen Quellhashes.

Ein kontrolliertes `/usr/bin/true`-Kind ohne Bericht deckte zunächst einen
falschen Erfolg auf (Launcherexit 0). Nach Korrektur bleibt der tatsächliche
Kindexit 0 erhalten, aber `effectiveExitCode` ist 1 und `reportError` erklärt den
fehlenden Bericht. Die Wiederholung auf denselben Ausgabepfaden wird abgewiesen,
die alten Nachweise bleiben byteidentisch und privat (0600). Diese Kontrolle
lädt kein Modell und verwendet keine Audiodaten. Die RED/GREEN-Logs liegen lokal
unter `/tmp/everlast-resource-launcher-{red,green}.log`.

RSS-Baseline ist ausschließlich der erste Checkpoint nach den fünf Warmups.
Fehlt dessen RSS, bleiben diese Baseline und sämtliche entsprechenden Deltas
leer; ein späterer Messwert wird nicht rückwirkend zum Warmup erklärt.
Metrikfehler bleiben getrennt vom Verhaltensstatus erhalten. Die OS-Helfer
verwenden ein Zwei-Sekunden-Pollingbudget und akzeptieren höchstens 1 MiB
kombinierten Prozessoutput. Private Tempfiles können zwischen Polls kurz
überschießen; OS-Operationen selbst haben keine harte Echtzeitgarantie.

## Eingefrorener erster Kandidat

Vor Modellarbeit per Release-Binary `--print-parameters` exklusiv erzeugt:
[Resourceparameter v1](../../tests/fixtures/corpus/runtime-resource-parameters-v1.json),
SHA-256 `1887af1c32eb623500eec73d44fdb614565c349235f0e8ef8403724469d0959f`.
Release-Binary `target/release/examples/runtime_resource_probe`, SHA-256
`dea85cf4dfec6f80d8c7bbbc08630ba4fe6ee4005c31b8c2d328317003f656ec`.
Build: `cargo build --release --offline --locked -p everlast-dictation --features measurement --example runtime_resource_probe`.
Die Laufparameter müssen vor jedem Modellstart genau dem einkompilierten
Template entsprechen. Änderungen der Quellen verlangen einen neuen Freeze.

Der kombinierte unabhängige Code- und Methodikreview hat keine offenen Befunde.
Zum Werkzeugcommit waren ein tatsächlicher Modelllauf und dessen unabhängiger
Zahlenreadback noch offen; die folgende Ausführung ist ein eigener Nachweis.

## Tatsächlicher erster Lauf

Unveränderte Quellen/Binary und vorab eingefrorene Parameter, keine parallelen
Builds oder Tests. Vorab erfasster Host: Apple M5, 34.359.738.368 Byte RAM,
macOS 27.0.1/26A434, aarch64, rustc 1.93.0, AC Power. Das ist eine beobachtete
Energiequelle, kein kontrolliertes oder gemessenes Energieprofil. Startprovenienz
2026-10-17 13:49:35 UTC, Abschlussprovenienz 14:00:43 UTC. OS-time: 667,89 s real;
diese Dauer ist keine Diktatlatenz. Einmalige Engineinitialisierung 1.266.335 µs.

```sh
EVERLAST_BENCHMARK_REVISION=25e8e9d /usr/bin/python3 scripts/run-runtime-resource.py target/release/examples/runtime_resource_probe artifacts/models/ggml-large-v3-turbo-q5_0.bin reference artifacts/replay/neue-aufnahme.wav artifacts/replay/neue-aufnahme-baseline.txt tests/fixtures/corpus/runtime-resource-parameters-v1.json artifacts/replay/runtime-resource-v1.json
```

Modell `large-v3-turbo-q5_0`, SHA-256
`394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2`.
Die autorisierte private WAV enthält 131.072 Frames, 16 kHz Mono, 8,192 s,
SHA-256 `9a108246c7b1196e8203ee22ebacabe7927d18f7a37a5f2e7647437db7df069c`.
WAV, Baselinewortlaut und Rohberichte bleiben unversioniert. Der Textvergleich
ist Wiederholbarkeit gegen die vorherige Modellbaseline, keine unabhängige
Sprachqualifikation oder WER-Abnahme.

| Prüfung | Tatsächliches Ergebnis |
|---|---:|
| Warmup-Sitzungen / gewertete Lifecyclezyklen | 5 / 100 |
| StopComplete / CancelCapture / CancelDecoderEntry, nur Messzyklen | 34 / 33 / 33 |
| Erfolgreiche Zeilen einschließlich Warmups | 105 von 105 |
| Vollständige Decoderaufrufe gestartet / beendet / exakt verglichen | 72 / 72 / 72 |
| Sampleabweichungen / Decoderfehler | 0 / 0 |
| Exakte Stoptextvergleiche einschließlich Warmups | 39 von 39 |
| Cancel-Zeilen ohne übernommenen Text | 66 von 66 |
| Durch folgenden StartACK bestätigte vorherige Ownerfreigaben | 104 von 104 möglichen |
| Bestätigter finaler Runtime-Join | ja, 3361 µs beobachtet |
| RSS-Checkpoints / Metrikfehler | 101 / 0 |
| Größte über alle Feed-Zusammenfassungen gemeldete Chunkabweichung | 3.075.708 ns |

Jede Feed-Zusammenfassung meldet timingValid=true; die Publikationszähler
summieren sich auf 29.553. Das Werkzeug behält je Feed nur Zusammenfassung und
Maximum statt sämtlicher Einzelzeitpunkte. Auch ein Stop-Ingressbudget wird
hier nicht abgenommen: `setupTimingPassed=true` ist kein vollständiger L-Nachweis.
Keine Busy-Startantwort musste in diesem konkreten Lauf abgewartet werden;
die entsprechenden Negativfälle sind separat kontrolliert getestet.
Nach Join sind Decoder active=false/armed=false, 72 gestartet und beendet,
kein später Decoderfehler und keine Sampleabweichung. Der letzte Zyklus hat
keinen Folgestart und behauptet diesen auch nicht; sein angenommenes Local-
Ergebnis und der bestätigte Join bilden die letzte Abschlussgrenze.

## Beobachtete Speicherreihe und Grenzen

Alle folgenden RSS-Werte sind Bytes aus `ps`, keine Heapgrößen:

| RSS-Beobachtung | Byte |
|---|---:|
| Erster Checkpoint nach fünf Warmups | 732.446.720 |
| Kleinster / größter Checkpointwert | 665.911.296 / 733.577.216 |
| Nach Messzyklus 100 | 668.516.352 |
| Ende minus Warmup-Baseline | -63.930.368 |
| Spanne und Anstieg der letzten 20 Checkpoints | 49.152 |
| OS-time maximum resident set size | 786.939.904 |
| OS-time peak memory footprint | 797.754.856 |

Die RSS-Reihe steigt nicht durchgängig über die 100 Zyklen: Sie fällt zunächst
und liegt am Ende unter der Warmup-Baseline. Innerhalb der letzten 20 Punkte
bleibt jedoch ein kleiner Anstieg sichtbar. Dies beweist keine Leakfreiheit.
Die Messung hält bewusst eine begrenzte Menge Zeilen und Checkpoint-JSON im
Prozess. Deren Allokationen gehören zur gemessenen Entwicklung; ohne Objekt-/
Stackvergleich ist die verbleibende Änderung nicht dem Produkt oder dem
Messwerkzeug allein zugeordnet.

Die beiden `vmmap`-Zusammenfassungen melden Physical footprint 708.8M und
646.6M (Einheiten genau wie im Tool). Ihre MALLOC-ZONE-TOTAL-Zeilen nennen
116.668 beziehungsweise 119.122 Allokationseinträge und BYTES ALLOCATED 104.1M
beziehungsweise 104.3M. Das ist kein unabhängig bestimmtes Live-Objektinventar.
Resident in derselben Malloctabelle: 54.1M beziehungsweise 55.3M; diese Spalte
ist nicht mit BYTES ALLOCATED zu verwechseln. Physischer Footprint, RSS,
virtuelle Reservierung und Heapdaten dürfen nicht gleichgesetzt werden.
OS-time liefert Prozessressourcenstatistik nach Programmende, keine Aussage
über jederzeitige einzelne Modell-/Backendallokationen oder Heapwurzeln.

Verhalten, Timingzusammenfassungen und Prozessende sind erfolgreich beobachtet;
`resourceQualified` bleibt ausdrücklich false. Offen bleiben insbesondere
Heap-/Objektzuordnung, natürliche 300-s-Sprache mit voller Inferenz, beide
Modelle im vollständigen Qualitäts-/Ressourcenvergleich und native Mikrofon-/
Geräte-/installierte App-Prüfungen. Dieser Lauf schließt P03/P04/P05 nicht ab.

## Lokale Rohbelege

Alle vier Dateien sind exklusiv erzeugt, Modus 0600. Exitcode und effektiver
Launcherexit sind 0, launchError/reportError leer; Completion bindet die Hashes.

| Datei unter `artifacts/replay/` | SHA-256 |
|---|---|
| runtime-resource-v1.json | `7c9efbf289c268efaf60343b2a93276c67e6efcea79e268564bbd86da3d28443` |
| runtime-resource-v1.json.host.json | `5ca30a43de0949eeb66486ffc51e1a375373ea0add488a718f2a80be9a3eaae9` |
| runtime-resource-v1.json.completion.json | `6aeb806c439a94a7d3b512b5d13aa6c114e5b0ca04beb3f1322b59bfa0f0196f` |
| runtime-resource-v1.json.command.log | `7c98b11b84bf8c823b3111834cd9dd00220d8e9c9760a863e46358033eea070a` |
