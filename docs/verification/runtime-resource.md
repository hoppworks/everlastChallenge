# Wiederholte Runtime-Sitzungen und Speicherbeobachtung

Vorbereitung am 17.10.2026, Basis `d1a2527`. Noch kein tatsächlicher Modelllauf
für dieses neue Werkzeug und keine Ressourcenabnahme aus diesem Dokument.

Die beabsichtigte feste Entwicklermessung verwendet eine einmal geladene Engine,
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

Das Werkzeug soll RSS in Byte und an zwei äußeren Messpunkten eine begrenzte
VM-Zusammenfassung aufnehmen. RSS ist kein Live-Heapmaß; Metal-/Allocator-Caches
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
Ein tatsächlicher Modelllauf und dessen unabhängiger Zahlenreadback sind durch
diese Werkzeugprüfung noch nicht ersetzt.
