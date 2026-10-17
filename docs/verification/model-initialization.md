# Modellbereitschaft in sechs neuen Prozessen

Stand: 17.10.2026, unveränderte Codebasis `7de795a`, Release auf Apple Silicon.
Dies misst die Modellinitialisierung bis zur beobachteten Bereitschaft des
Inferenzworkers. App-/GUI-Start, Hotkey-/Aufnahmestart, erste nichtleere
Sprachinferenz und Echtzeit-L sind andere Messgrenzen und bleiben offen.

## Vorab festgelegter Aufbau

Die vorhandene Corpus-Probe wird in sechs getrennten Prozessen seriell gestartet:
reference-01, small-01, reference-02, small-02, reference-03, small-03.
Keine Warmups, keine Wiederholung oder Auswahl nach Ergebnissen. Betriebssystem-
Dateicache und Treibercaches bleiben unverändert. „Kalt“ bedeutet hier neuer
Prozess und neue Modellkontexte, keine geleerten Systemcaches.

Die [Parameterdatei](../../tests/fixtures/corpus/model-init-parameters-v1.json)
entstand vor allen sechs Starts: SHA-256
`3443cead56d4751cdc4e7edcc605bd4d2653e64aca174b60df6a05ece3b7343c`.
Sie enthält die exakte Reihenfolge und vollständigen Kommandos, Modellhashes,
aktuelle Decoder-/VAD-/Energieparameter, Hostdaten und 25 Quellhashes.
Der unabhängige Plan-/Freeze-Readback bestätigte alle Werte vor Modellarbeit.
Vor jedem Start wurden Freeze, Binary, Manifest und die 25 Quellen erneut
byteweise beziehungsweise per SHA-256 gegen diesen Stand geprüft.

Der [einzige Manifestfall](../../tests/fixtures/corpus/model-init-manifest-v1.json),
SHA-256 `039d796bd4c028f9c06cbc3058b328687b427d5ce8c53e631d8502c9299f8363`,
ist der unveränderte synthetische Nullfall aus calibration-v2: 80.000 Nullframes,
PCM16 Mono/16 kHz, fünf Sekunden. WAV-SHA-256
`262d1f34f300560a4c82ba8127ddea9e1d72946f187fdbd3e8f70ee096077aec`.
Der leere Job folgt nach der gemessenen Bereitschaft und prüft nur Export und
Abschluss. Er ist keine Sprach-, Raumgeräusch- oder neue Korpusqualifikation.

Release-Build ohne Quelländerung:

```sh
cargo build --release --offline --locked -p everlast-dictation --features measurement --example corpus_probe
```

Binary-SHA-256:
`bdc85a31790792ed7a0390ea0cc16fa9ae3d9282e93e351fe1164b6cf36ba723`.
Jedes im Parameterdokument festgehaltene Kommando wurde mit
EVERLAST_BENCHMARK_REVISION=7de795a über `/usr/bin/time -l` aufgerufen.
Ausgaben verwenden neue private Dateien, kein vorhandenes Ergebnis wird ersetzt.
Ein Fehler hätte spätere Zeilen als unavailable und die gesamte Blockstatistik
als fehlend markiert; kein selektiver Retry oder Ausschluss.

Host vorab erfasst: Apple M5, 34.359.738.368 Byte RAM, macOS 27.0.1/26A434,
rustc 1.93.0, AC Power. Energiequelle ist kein kontrolliertes Energieprofil.
Während der sechs Starts liefen keine parallelen Builds, Tests oder Modelljobs
für dieses Vorhaben. Allgemeine Hintergrundlast wurde nicht instrumentiert.

## Tatsächliche Messgrenzen und Werte

Die vorhandene Probe setzt Instant unmittelbar vor InferenceService::load und
liest ihn nach beobachtetem ready. Darin enthalten sind Prüfung/Snapshot der
Modelldateien, Backend-/Modellinitialisierung und Bereitschaftspolling.
`loadMillis` ist mit as_millis nach unten auf ganze Millisekunden abgerundet;
das Pollingintervall ist 1 ms, OS-Scheduling gehört zur Beobachtung.
Es ist keine exakt am atomaren Ready-Store erfasste Backendzeit.

Der separat beobachtete Elternzeitraum reicht vom Start des time-Wrappers bis
zu dessen Ende. Er enthält zusätzlich Prozessstart, leeren Job, Cleanup und
Berichtsexport und darf nicht als reine Modellladezeit ausgegeben werden.
Die aufgezeichneten Wrapper-PIDs sind nicht die Zielprozess-PIDs; sechs neue
Subprozessaufrufe starten hier jeweils ein eigenes Probeprogramm, keinen Daemon.

| Lauf | Modellbereitschaft (ms) | Voller Elternzeitraum (ms, gerundet) | Service-Cleanup (ms) |
|---|---:|---:|---:|
| reference-01 | 1269 | 1922,2 | 3 |
| small-01 | 549 | 1017,5 | 2 |
| reference-02 | 1245 | 1627,8 | 3 |
| small-02 | 548 | 1020,7 | 2 |
| reference-03 | 1251 | 1645,7 | 3 |
| small-03 | 539 | 1005,7 | 1 |

| Modell | Einzelwerte (ms) | Median (ms) | Minimum–Maximum (ms) | Spannweite (ms) |
|---|---|---:|---|---:|
| large-v3-turbo-q5_0 | 1269, 1245, 1251 | 1251 | 1245–1269 | 24 |
| small-q5_1 | 549, 548, 539 | 548 | 539–549 | 10 |

Alle sechs Exits sind 0; jeder Export enthält genau den erwarteten leeren
synthetischen Fall. Load-, Service- und Cleanupfehler sind jeweils leer,
Cleanup vor Prozessende bestätigt. Der leere Job dauerte jeweils 1 ms laut
Metadaten; daraus folgt keine Aussage über den ersten echten Decoderaufruf.
Drei Werte je Modell sind keine universelle Startzeitgarantie oder p95-Schätzung.
Auch eine schnelle Initialisierung ersetzt keine Qualitäts-/Ressourcen-/L-
Qualifikation und entscheidet nicht die finale Modellwahl in P06.

## Belege

Der [unveränderte numerische Blockbericht](model-init-v1.json) hat SHA-256
`b4adf90cd7abbc3a6c7ffb14c79a494e3909a02841b0dd4cfc40b6375200abb6`.
Er enthält alle sechs Einzelwerte, beobachteten Prozesszeiträume, Wrapper-PIDs
und die Hashes sämtlicher Ergebnis-, Metadaten- und Kommando-Logdateien.
Die Rohdateien unter artifacts/model-init/v1 bleiben unüberschrieben, Modus 0600;
sie enthalten ausschließlich den synthetischen Leerfall, keinen Sprachwortlaut.
Die Quellen und der vollständige Messplan sind oben gehasht beziehungsweise
versioniert. Die neuen Artefakte sind Messnachweise, keine Produktcodeänderung.
P05 bleibt insbesondere wegen vollständiger Korpus-/Anker-, Echtzeit- und
Maximaldauer-/Ressourcenabnahmen offen.
