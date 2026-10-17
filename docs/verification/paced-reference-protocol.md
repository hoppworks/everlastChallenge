# Sechs zentrale Fälle in einer warmen Runtime

Stand: 17.10.2026. `paced_reference_probe` ergänzt die bisherige Einzelfallprobe
um den vollständigen zentralen Vergleich aus §13. Hier wird das Messwerkzeug
beschrieben, noch kein ausgeführter Latenzvergleich oder P05-Abschluss.

## Feste Eingaben und Reihenfolge

[Die sechs zentralen Fälle](../../tests/fixtures/corpus/central-reference-v1.json)
haben SHA-256 `244b2cd766874855174699f8568dbb09ae664380a98b272bf6588293f44a36b5`.
Ihre IDs und Audiohashes waren bereits vor der ersten Abnahmeinferenz in
`acceptance-reference-parameters-v1.json` festgelegt. Die unveränderten
Fallobjekte stammen aus dem 90er-Abnahmekorpus; dies sind keine sechs
zusätzlichen unabhängigen Sprachaufnahmen.

Der Adapter akzeptiert genau DE/EN × 5/10/15 s, saubere Abnahmefälle mit
eindeutigen IDs, Audioassets und nichtleeren begrenzten Referenzen. Tatsächliche
WAV-Hashes, PCM16-Mono-16-kHz-Format und Framezahlen werden vor Modellarbeit
geprüft. Alle sechs dekodierten Eingaben umfassen zusammen 960.000 f32-Samples
(3.840.000 Byte Nutzdaten); dies ist keine Gesamtspeicherobergrenze.

Ein einziger `InferenceService` und eine `MeasuredRuntime` bleiben für die
Konfiguration warm. Zuerst genau ein markierter Warm-up auf DE5, danach
30 gewertete Läufe je Fall in der Reihenfolge DE5/10/15, EN5/10/15: 181 Slots.
Keine Wiederholung oder neue Aufwärmphase zwischen Fällen. Der vorhandene
zeitgetreue Feeder, Audioowner, DSP, Controller, Inferenzpfad und die vorhandenen
Stop-/Text-Zeitstempel bleiben beteiligt.

## Fehler, Auswertung und private Ergebnisse

Ein fehlerhafter oder unerwartet leerer Lauf beendet die weitere Ausführung;
die verbleibenden Slots werden als nicht ausgeführt mit Ursache aufbewahrt.
Jeder Slot bindet Fixture-ID/-Hash, Sprache, Dauer und Laufnummer. Es entsteht
keine Statistik über einen erfolgreichen Teilbestand. Auch Warm-up-Fehler,
fremde/fehlende Slots und unbestätigter abschließender Join unterdrücken die
Gesamtstatistik.

Erst nach bestätigtem Cleanup berechnet das Werkzeug die sechs Mediane und
Nearest-Rank-p95-Werte sowie deren ungewichtetes Medianmittel `L` in ns.
Die absoluten Grenzen (je Fall Median ≤1 s und p95 ≤1,5 s) werden zusätzlich
als eigene Kriterien ausgewiesen. Ein vollständig ausgeführter Messlauf ist
kein automatisches Bestehen dieser Grenzen.

Die veröffentlichten Referenzen dienen später der unveränderten WER- und
Originalinhaltprüfung. Sie sind keine bytegenaue Modellbaseline; der Timinglauf
verlangt deshalb keine exakte Zeichenübereinstimmung. Sämtliche Hypothesen
bleiben für die gesonderte Qualitätsprüfung erhalten. Das Werkzeug behauptet
weder geprüfte Sprachqualität noch native Mikrofon-/Hotkey-/Zielübergabe,
Ressourcenabnahme oder Gesamtqualifikation.

Hypothesen und numerische Metadaten werden jeweils atomar als neue Dateien
mit Modus 0600 publiziert; vorhandene Belege werden nicht ersetzt. Das Paar
ist keine dateiübergreifende Transaktion. Ein Fehler der zweiten Publikation
wird gemeldet; die bereits publizierte erste Datei bleibt als Beleg erhalten.

## Geplanter Aufruf nach Werkzeugprüfung und Parameterfreeze

Release-Build ausschließlich des Entwicklerwerkzeugs:

```sh
cargo build --release --offline --locked -p everlast-dictation --features measurement --example paced_reference_probe
```

`--print-parameters` gibt das einkompilierte Template aus. Vor dem ersten
Modelllauf muss es in eine neue Parameterdatei geschrieben und zusammen mit
Binary-, Manifest- und Modellhashes geprüft werden. Das Template bindet
Messprotokoll und beteiligte Quellen; eine abweichende Datei wird abgewiesen.
Danach lautet die explizite CLI:

```text
paced_reference_probe MODEL_PATH reference|small SIX_CASE_MANIFEST PARAMETERS_JSON NEW_OUTPUT_JSON
```
