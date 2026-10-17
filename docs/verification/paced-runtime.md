# Zeitgetreue Stop-bis-Text-Messung durch die Runtime

Stand: 17.10.2026; Basis `7cb34a3`, Kandidat `7cb34a3+paced-runtime-v1`.
Das Entwicklerwerkzeug verbindet die zeitgetreue Quelle mit dem produktiven
Audioowner, Controller und echten Inferenzdienst. Es ersetzt keine installierte
Mikrofon-/Hotkey-/Einfügeabnahme. P03/P04/P05 bleiben offen.

## Messgrenzen und Reset

Das nicht standardmäßig aktivierte Cargo-Feature `measurement` stellt
`MeasuredRuntime` bereit. Ausschließlich die Audioquelle wird ersetzt; Ring,
DSP, Aufnahmegrenzen, Drain/Flush, Controller und serieller Inferenzworker
bleiben beteiligt. Native Übergabe ist für diesen Messstart deaktiviert.
Es gibt keinen neuen Datei-IPC-Pfad in der App.

`t_stop` wird als erste Operation am Host-Eingang vor Shared-Lock und
Controller-Warteschlange erfasst. `t_text` entsteht unmittelbar nach der
akzeptierten finalen nichtleeren Controllerantwort, vor Snapshotkopie und
Beobachterpoll. Beide verwenden dieselbe monotone Uhr wie Capture-t0.
Falsche Session, Generation, Job-ID oder Samplebereich erzeugen keinen
Textzeitstempel; Empty, Fehler und Cancel ebenfalls nicht.

Der Messstart verlangt einen freien Controller, freien warmen Inferenzworker
und beendeten Audioowner. Er verwirft alte Ergebnisse, Text und bereitliegende
Replay-Ports und setzt die Messspur zurück. Normaler App-Start bleibt unverändert.
Der gemeinsame Clock-Feeder speist beide Probes in absoluten 20-ms-Schritten
plus letztem Rest ein, ohne auf DSP oder Decoder zu warten. Jitterbudget bleibt
10 ms; Wartefolge maximal 1 ms Sleep und zuletzt maximal 250 µs Spin.
Stop erfolgt unmittelbar nach der letzten Veröffentlichung vor Berichtsarbeit.

## Vor dem Pilot eingefrorene Konfiguration

Die Release-Binärdatei erzeugte per `--print-parameters` vor dem Modelllauf
[die Parameterdatei](../../tests/fixtures/corpus/paced-reference-parameters-v1.json).
SHA-256: `31806789ef7b19d46f13b7b3442cab10a12f43e063cc57585c2065bdad057cb1`.
Sie enthält Modell-/VAD-/Decoderparameter und Quellhashes von Runtime, Audio,
Clock, Inferenz, lokalen SDK-Korrekturen, Manifesten und Probe. Vor Modellarbeit
muss die gelesene Datei exakt dem einkompilierten Template entsprechen.
Ein geänderter Kandidat benötigt eine neue eingefrorene Datei.

Release-Werkzeug SHA-256:
`e8109ddd550cd5e6f3c570b6b8ff921aa045f47491b6457665e2f9c26e503fa1`.
Gebaut mit `cargo build --release --offline --locked -p everlast-dictation --features measurement --example paced_runtime_probe`.
Danach lief ohne parallele Builds/Tests:

```sh
EVERLAST_BENCHMARK_REVISION=7cb34a3+paced-runtime-v1 target/release/examples/paced_runtime_probe artifacts/models/ggml-large-v3-turbo-q5_0.bin reference artifacts/replay/neue-aufnahme.wav de tests/fixtures/corpus/paced-reference-parameters-v1.json artifacts/replay/paced-runtime-v1-results.json 1 artifacts/replay/neue-aufnahme-baseline.txt
```

Der Host meldete Apple M5, 34.359.738.368 Byte RAM, macOS 27.0.1/26A434,
AC Power und rustc 1.93.0. Energiequelle ist keine Messung eines Energieprofils,
und die ruhige Buildphase beweist keine allgemeine Hintergrundlastfreiheit.
Modell `large-v3-turbo-q5_0`, SHA-256
`394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2`.
Laden einschließlich Runtimebereitschaft: 1327 ms; kein Kaltstartvergleich.

## Tatsächliche Ergebnisse

Die autorisierte private WAV aus „Neue Aufnahme.m4a“ enthält 131.072 Frames
bei 16 kHz Mono, 8,192 s, SHA-256
`9a108246c7b1196e8203ee22ebacabe7927d18f7a37a5f2e7647437db7df069c`.
Audio und Transkript bleiben unversioniert. Beide Läufe erzeugten finalen lokalen
Text und stimmten exakt mit der vorherigen Modellbaseline überein. Diese
Baseline ist keine unabhängig gehörte Referenz und beweist nur Wiederholbarkeit.

| Lauf | t_stop ab t0 (ns) | t_text ab t0 (ns) | Stopp bis Text (ns) | Größter Chunkjitter (ns) | Stopjitter (ns) |
|---|---:|---:|---:|---:|---:|
| Warm-up | 8.192.015.167 | 9.223.152.334 | 1.031.137.167 | 226.042 | 15.167 |
| Messlauf 001 | 8.192.014.459 | 9.217.386.542 | 1.025.372.083 | 255.042 | 14.459 |

Je Lauf sind alle 410 Publikationen onBudget; Capture- und Controller-t0 stimmen
überein. Geordneter Runtime-Join bestätigt (6239 µs beobachtetes Cleanup).
Die Einzelfallstatistik umfasst nur den einen Messlauf: Median und p95 sind
1.025.372.083 ns, Spannweite null. Dies ist keine belastbare p95-Schätzung.
Insbesondere folgt daraus keine bestandene 1-s-Mediangrenze.

Alle Rohdaten bleiben lokal erhalten, beide Dateien Modus 0600:

- `artifacts/replay/paced-runtime-v1-results.json`, SHA-256
  `706082fb1c70d55d722db8b74b0cc95feedd649498377fcd71fb2e776e739bed`.
- `artifacts/replay/paced-runtime-v1-results.json.metadata.json`, SHA-256
  `629a384a96d6784e059f8a99cd79b20a2c602a82b304cf0d8912bfb7d5e06884`.

Der Bericht kennzeichnet `L: null`, `qualified: false` und
`thirtyMeasuredRunsCompleted: false`. Fehlgeschlagene Läufe werden erhalten;
nach einem Fehler werden weitere angeforderte Läufe als unavailable erfasst,
nicht heimlich wiederholt. Jede fehlerhafte Zeile oder unbestätigtes Cleanup
unterdrückt die Gesamtstatistik.

## Prüfungen und verbleibende Abnahme

Offen bleiben die sechs festgelegten zentralen Sprachfälle mit je 30 Läufen,
Kalt-/Warmstart, unabhängige Qualitäts-/Ankerprüfung, Ressourcen-/Heapmessung,
der natürliche 300-s-Fall und insbesondere die bekannte Endpunktrennsituation
zwischen 300-s-Wallclocklimit und letzter Replaypublikation. Die alten reinen
Audiopiloten in [paced-input.md](paced-input.md) bleiben historische Belege.
Es wurde kein App-/DMG-Bundle neu gebaut oder dessen Freigabe verändert.
