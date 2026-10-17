# P05: korrigierte initiale No-Speech-Zeile

Stand: 17.10.2026. Diese Messung ändert gegenüber [v2](calibration-v2.md)
ausschließlich die Auswahl der initialen SOT-Logit-Zeile im lokal gepinnten
whisper-rs-sys. Energiegrenzen, Modelle und Decoderparameter bleiben gleich.
Die Ursachenprüfung steht in [whisper-no-speech-source.md](whisper-no-speech-source.md);
Provenienz und minimaler Patch in [PATCHES.md](../../vendor/whisper-rs-sys/PATCHES.md).

## Festgeschriebener Versuch

Parameter: [`reference-parameters-v3.json`](../../tests/fixtures/corpus/reference-parameters-v3.json),
SHA256 `76a5dd60e5c18ed98c144708eb20422752ee70f41f7e6dfd64e577421a5c6882`.
Alle 42 Fälle aus `calibration-v2.json` (SHA256
`514befba28e7ea14882cd13a71f971e5fb6c6131f914491b697741a027611a49`)
liefen je Modell seriell durch einen warmen InferenceService, mit frischem
WhisperState je Job. Die Metadatenkennung ist
`c247563+whisper-sot-f369a8ba`; dies bezeichnet ausdrücklich einen Arbeitsstand.
Beide Dienste meldeten 42 erfolgreiche Jobs ohne Lade-, Service- oder Cleanupfehler.
Die separate Abnahmemenge wurde nicht transkribiert.

## Ergebnis

| Modell | DE sauber | EN sauber | DE leise | EN leise | unerwünschte Ausgaben / 6 Leerkontrollen |
|---|---:|---:|---:|---:|---:|
| large-v3-turbo-q5_0 | 8/218 (3.67 %) | 5/207 (2.42 %) | 3/121 (2.48 %) | 2/114 (1.75 %) | 4/6 |
| small-q5_1 | 18/218 (8.26 %) | 9/207 (4.35 %) | 9/121 (7.44 %) | 3/114 (2.63 %) | 1/6 |

Fehler/Wörter auf den ursprünglichen 34 Sprachfällen sind gegenüber v2 identisch.
Satz plus 3 s Stille ergibt bei beiden Modellen DE 0/26 und EN 2/21 Fehler.
Das kleine Modell reduziert unerwünschte Ausgaben von vier auf eine; das
Referenzmodell bleibt bei vier. Auch reine Satzzeichen zählen als Ausgabe.
**v3 ist weiterhin nicht qualifiziert.** Eine korrekte No-Speech-Wahrscheinlichkeit
ist kein unabhängiger Sprachdetektor und erlaubt keine pauschale Nachfilterung.

Dateibackendzeiten sind keine Echtzeitmessung L. Ressourcen, Abbruch,
natürliche fünfminütige Sprache, vollständige Abnahmemenge und Hörprüfung
bleiben ebenso offen wie die übrigen P05-Abnahmen. Englisch bleibt gesperrt.

## Numerische SDK-Regression und ihre Grenzen

Das Entwicklerbeispiel `no_speech_regression` vergleicht die SDK-Segmentwerte
mit einer unabhängigen SOT-Softmax aus sicheren Single-Token-Decodes. Es prüft
Sprache und Büroton, jeweils ohne Präfix und mit einem Präfix, das ein früheres
SOT enthält; beide Varianten laufen mit frischem und wiederverwendetem Full-State.
Es gibt keine automatische rollende Historie. Diagnoseparameter unterscheiden
sich bewusst vom Produkt: `no_speech_thold=1.0` hält Segmente beobachtbar und
`temperature_inc=0` verhindert Fallbacks. Dies ist kein Qualitätslauf.

Die unveränderte Grenze ist `1e-12 + 0.001 * abs(oracle)`. Auch winzige
Referenzmodell-Wahrscheinlichkeiten müssen damit falsche uniforme/letzte Zeilen
unterscheiden. Zusätzlich verlangt der Bericht einen Abstand der SOT- von der
letzten Initialzeile von über zehn Toleranzen in jeder Präfixvariante.

Der tatsächlich ausgeführte numerische Regressionsnachweis ist:

| Modell / Backend | Original-CPP | korrigiertes CPP |
|---|---:|---:|
| Referenz / Metal | 0/8 bestanden, Exit 1 | 8/8 bestanden, Exit 0 |
| Small / CPU | 0/8 bestanden, Exit 1 | 8/8 bestanden, Exit 0 |

Vor jedem CPP-Wechsel wurde das Release-sys-Paket gereinigt, neu gebaut und
sein aktiver OUT_DIR-Quellhash geprüft. Die Originaldatei stammte aus dem
gepinnten Registryarchiv. Ein `finally` stellte gepatchten Quelltext und Binary
wieder her; der CPU-Green-Lauf nach Wiederherstellung bestand erneut 8/8.
Ein früherer Versuch mit veraltetem Buildcache wurde vor Inferenz abgebrochen
und zählt nicht als RED. Ein erster Referenz-Prototyp mit lockerer absoluter
Toleranz zählt ebenfalls nicht zum Nachweis.

**Offene numerische Diagnose:** Small auf Metal besteht bei derselben strengen
Grenze nur 6/8 Vergleiche. Die beiden Präfix-Sprachbeobachtungen weichen relativ
um rund 0,137 % ab. Ein unveränderter zweiter Metal-Lauf reproduziert sämtliche
Beobachtungen exakt. Auf CPU beträgt die größte relative Abweichung etwa
0,0011 %. Dies belegt eine backend-/batchabhängige numerische Abweichung zwischen
Single-Token-Oracle und Full-Decode; ihre genaue Ursache ist nicht abschließend
isoliert. Weder ein bewiesener Metal-Fehler noch allgemeine numerische Gleichheit
werden behauptet. Die Toleranz wurde nicht an diese Messwerte angepasst;
beide gescheiterten Metal-Berichte bleiben erhalten. Der eng begrenzte
SDK-Nachweis ist CPU-Small plus Referenz-Metal, keine Small-Metal-Qualifikation.

Reproduktion (bereits vorhandenes, SHA-geprüftes Modell; nach dem in PATCHES.md
beschriebenen Clean/Build):

```sh
cargo build --release --locked --example no_speech_regression
EVERLAST_BENCHMARK_REVISION='COMMIT_OR_SOURCE_ID' target/release/examples/no_speech_regression artifacts/models/ggml-small-q5_1.bin small tests/fixtures/corpus/calibration-v2.json artifacts/corpus/new-small-cpu-report.json cpu
EVERLAST_BENCHMARK_REVISION='COMMIT_OR_SOURCE_ID' target/release/examples/no_speech_regression artifacts/models/ggml-large-v3-turbo-q5_0.bin reference tests/fixtures/corpus/calibration-v2.json artifacts/corpus/new-reference-metal-report.json
```

Ausgabepfade müssen neu sein. Die Dateien enthalten ausschließlich numerische
Diagnosewerte und werden mit 0600 angelegt. Das optionale letzte Argument `cpu`
ändert nur das Diagnosebackend; ohne Argument wird Metal verwendet.

## Lokale Messartefakte

Rohtexte bleiben unter dem ignorierten `artifacts/corpus`.

| Datei | SHA256 |
|---|---|
| calibration-reference-v3-results.json | `4901eb25893b4c37cecdff884342c302c4216fe3ab884bd53b6138e9d4519a51` |
| calibration-reference-v3-results.json.metadata.json | `ba06c595896bb47302d65fab502d75d1dad681560768fda015c49c04fb4504d7` |
| calibration-reference-v3-report.json | `145d298f52fc67e2808fc927483c57c2c29dbab53329d9222a0757df87f10f62` |
| calibration-small-v3-results.json | `f9843d4d8b9af81beb23128458b2f24cc8daa98944616772d9998ba448068ca9` |
| calibration-small-v3-results.json.metadata.json | `882e7e8db6dbc17a8f9da84ed8c56dfa98de953787f50bc9201524c57336625c` |
| calibration-small-v3-report.json | `4417bf4bbd81d7ae8bfe982ee8de03e643785d831078bd0747df8ac57516717b` |
| no-speech-regression-reference-original.json | `27491393d2ce8766665bf79f2e1688d0fa190e151abd65666399b5d6cc8fd386` |
| no-speech-regression-reference-patched.json | `dda2c3e629de7ea8f217d23c60547fe1382ca62c5aa1aed09b9e882d0df62a45` |
| no-speech-regression-small-original-cpu.json | `e613da7fb6effe2c6ec064dc9f7bc1caad160c8bb83f76112133c33427dbf944` |
| no-speech-regression-small-patched-cpu.json | `80b02452b59e8236e03fbd4414c7e8505aeafc167fe0b575f26064a1cb681919` |
| no-speech-regression-small-restored-cpu.json | `80b02452b59e8236e03fbd4414c7e8505aeafc167fe0b575f26064a1cb681919` |
| no-speech-regression-small-patched.json | `b99a66d484a2808883ce81d59eb44d2681d80ca2c0e4fc7760f2cd0a1a2b292d` |
| no-speech-regression-small-patched-metal-repeat.json | `b99a66d484a2808883ce81d59eb44d2681d80ca2c0e4fc7760f2cd0a1a2b292d` |
