# Entwicklerprobe: VAD-Kalibrierung ohne Text

`src-tauri/examples/vad_probe.rs` untersucht ausschließlich die **42 Fälle** aus
`tests/fixtures/corpus/calibration-v2.json`. Festgehaltener Manifest-SHA256:
`514befba28e7ea14882cd13a71f971e5fb6c6131f914491b697741a027611a49`.
Andere Manifeste, Teilmengen oder Acceptance-Daten werden abgelehnt.

Voraussetzungen und Grenzen: [VAD-Quellenprüfung](vad-gate-source.md).
Das Modell `artifacts/models/ggml-silero-v5.1.2.bin` muss exakt 885.098 Bytes
und SHA256 `29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf`
haben. Die geprüften Bytes werden über eine eigene private Tempdatei geladen;
ein Austausch des ursprünglichen Modellpfads nach dem Hashcheck wird damit
vermieden. Bestehende WAV-Hash-/Format-/Frameprüfungen aus `corpus_probe/input.rs`
werden wiederverwendet.

Gezielte Checks ohne Modelllauf:

```sh
cargo test --locked --offline -p everlast-dictation --example vad_probe
cargo clippy --locked --offline -p everlast-dictation --example vad_probe -- -D warnings
```

Serialisierter Entwicklerlauf aus dem Repositoryroot:

```sh
cargo run --locked --offline --release -p everlast-dictation --example vad_probe -- artifacts/models/ggml-silero-v5.1.2.bin tests/fixtures/corpus/calibration-v2.json artifacts/corpus/vad-calibration-v2-raw.json
```

Der Probe startet keine Transkription und öffnet kein Mikrofon. Standard bleibt
`raw-pcm16-no-gain-no-trim`, Mono/16 kHz, CPU mit einem Thread. Kein Sprach-/
Taskwechsel des VAD-Modells und keine Schnitte. DE und EN sind ausschließlich
Fallmetadaten. Ein VAD-Kontext wird seriell wiederverwendet; die native API setzt
den LSTM-State bei jedem Call zurück.

## Vorab festgelegte zweite Variante

Der erste Raw-Pilot zeigte nach Mitteilung des Koordinators drei leise positive
EN-Fälle mit sehr kleinen VAD-Werten. Deshalb wird **vor dem nächsten Modelllauf**
genau eine unabhängige Variable hinzugefügt: optionales letztes CLI-Argument
`peak-normalized`. Ganze Aufnahme: `peak = max(abs(sample))`, festes Ziel
**0,95**, `gain = 0.95 / peak` bei `peak > 0`; bei Nullpeak Gain 1 und Samples
unverändert. Berechnung in f64, pro Ausgangssample eine Rundung nach f32.
Keine Begrenzung des Gains, kein Clipping, Trimming oder Frameversatz. Auch
laute Aufnahmen werden gegebenenfalls abgeschwächt. Dieselbe Regel gilt für
**alle 42 Fälle einschließlich Stille, Rauschen und Raumaufnahme**; keine
fallabhängige Normalisierung und keine zusätzliche Variante oder Schwellenänderung.
Damit wird die Auswirkung des Eingangspegels untersucht; ein Nutzen wird noch
nicht behauptet. Die Normalisierung verändert ausschließlich den privaten
VAD-Eingang, keine WAV-Datei oder Produktions-ASR.

```sh
cargo run --locked --offline --release -p everlast-dictation --example vad_probe -- artifacts/models/ggml-silero-v5.1.2.bin tests/fixtures/corpus/calibration-v2.json artifacts/corpus/vad-calibration-v2-peak-normalized.json peak-normalized
```

Neue Berichte verwenden Schema **2**, je Fall `gain`, `inputSignal` und
`outputSignal`; die ursprünglichen WAV-Hashes bleiben erhalten. `parameters`
enthält Variante, Zielpeak (bei Raw null), Nullpeak-Gain und explizit kein
Trimming/Clipping. `prepareMicros` erfasst Signalstatistik und optionale Skalierung
getrennt vom VAD-Call. `sourceRevision` übernimmt optional die Umgebungsvariable
`EVERLAST_BENCHMARK_REVISION`; `probeSourceHash` ist der SHA256 der beim Build
über `include_bytes!("vad_probe.rs")` eingebetteten Beispieldatei. Die Revision
allein beschreibt uncommittete Änderungen nicht vollständig.

Der bestehende Raw-Pilotbericht wird erhalten und erhält **keinen** nachträglich
zugeordneten Quellhash der neuen Fassung. Der Koordinator führt Raw und die neue
Variante mit der aktualisierten Metadatenfassung getrennt und serialisiert aus.

Die numerische JSON-Ausgabe enthält pro Fall ID, Sprache und Audiohash,
Frames/Dauer, RMS/Peak des unveränderten Eingangs, erwartete 512-Sample-Fenster
und echte Samples im letzten Fenster, alle Framewahrscheinlichkeiten, Maximum
und Anzahl Werte **>= 0,5**. Diese vorab festgelegte Zählgrenze dient nur der
Beschreibung, ist **keine kalibrierte Produktionsschwelle**. Keine Referenz-
oder Transkripttexte werden ausgegeben. Zeiten: Eingangsprüfung und VAD-Call
pro Fall sowie Initialisierung, Batch und Kontextfreigabe in Mikrosekunden.
Das misst weder RSS noch Echtzeitlatenz L oder eine harte Cancelgarantie.

Malformed Audio, falscher Modell-/Manifestpin, ungültige Scores, NaN/Inf,
falsche Scoreanzahl und API-Fehler werden niemals zu „Stille“. Erkannte
Fallfehler bleiben als numerisch textfreie Fehlercodes erhalten; der Prozess
endet danach mit Fehlerstatus. Die Ausgabe wird als neue atomar veröffentlichte
0600-Datei geschrieben und ersetzt keine bestehende Datei. Der Ausgabeordner
muss bereits existieren. Bei einem globalen Pin-/Initialisierungsfehler gibt
es keine neue Ergebnisdatei und keine Sprachentscheidung.

**Wichtige bekannte SDK-Grenze:** Im untersuchten C++-Stand kann ein interner
Frame-Computefehler dennoch `true` liefern und unberechnete Scoreplätze
hinterlassen. `Ok(())`, passende Länge und endliche Werte beweisen daher noch
keinen vollständigen Lauf. Jeder erfolgreiche Fall heißt ausdrücklich
`api_completed_untrusted`; jede Bewertung bleibt
`undecided-experiment-only`. Die Metadaten setzen
`completeExecutionGuaranteed=false` und `productionSilenceDecision=false`.
Vor einem echten Gate ist der Fehlerpfad separat zu korrigieren und zu testen.

Keine Schwellenentscheidung, keine Änderung des Produktpfads und keine P05-
Abnahme folgen aus der Probe. Die nach dem Implementierungsschritt tatsächlich
ausgeführten Raw-/Peak-Folgeläufe stehen in [vad-calibration.md](vad-calibration.md).
Modellfreie Tests prüfen lediglich Eingangs-/Numerik-/Ausgabegrenzen.
