# P05-Prüfkorpus und vollständige Referenzkonfiguration

Dieser Korpus dient der lokalen Qualitätsmessung. Er ist noch keine bestandene
P05-Abnahme. Die Audiodateien liegen ausschließlich im ignorierten Verzeichnis
`artifacts/corpus`; private Benutzeraufnahmen gehören nicht zu dieser Auswahl.

Die historischen JSON-Dateien bleiben unverändert. Neue Generatorausgaben
verwenden neue Pfade; Hinweise zur Format- und Hashbindung stehen in
[Entwicklerwerkzeuge](../../../docs/tooling.md).

## Reproduktion

Die festen Quellen, Downloadlinks, Hashes und Lizenzbelege stehen in
[`docs/verification/corpus-sources.md`](../../../docs/verification/corpus-sources.md).
Die vier FLEURS-Archive und TSVs heißen lokal
`artifacts/corpus/source/{de_de,en_us}-{dev,test}.{tar.gz,tsv}`.
Das Heidelberg-Archiv heißt `artifacts/corpus/source/hd_audio.tar.gz`.
Beide Builder prüfen die fest im Quelltext hinterlegten Hashes vor Verwendung.

```sh
node --import tsx scripts/corpus.ts --source artifacts/corpus/source --manifest artifacts/corpus/fleurs-base.json
node --import tsx scripts/short-corpus.ts artifacts/corpus/fleurs-base.json --source artifacts/corpus/source/hd_audio.tar.gz --output artifacts/corpus/manifest-ts.json
npm run test:quality
```

Die Auswertung verwendet fest eingebundene Unicode-13.0.0-Tabellen. Node führt
die TypeScript-Werkzeuge aus; eine Python-Installation ist nicht erforderlich. Die HD-Konvertierung
verwendet das vorhandene `/opt/homebrew/bin/ffmpeg`; Version und Binärhash stehen
im erzeugten Manifest. Abweichende eingefrorene Dateien werden nicht ersetzt.
Werkzeuge und Audiohashes müssen vor dem Modellvergleich festgeschrieben sein.

## Auswahl und Grenzen

Die Auswahl erfolgt deterministisch aus Metadaten, unabhängig von ASR-Ausgaben.
FLEURS-dev dient der Kalibrierung, FLEURS-test der späteren Abnahme. Pro Sprache
entstehen zentrale Fälle mit 5, 10 und 15 Sekunden durch ausschließlich hinten
angefügte Nullframes. Leise Varianten verwenden unveränderte Referenzen und
einen festen Verstärkungsfaktor. Sie sind keine zusätzlichen unabhängigen Sätze.
HD ergänzt vollständige natürliche Kurzäußerungen für acht/neun beziehungsweise
eight/nine mit den offiziellen getrennten Train-/Testlisten.

Stille und synthetisches Rauschen haben leere Referenzen und keinen WER-Nenner.
Der konstruierte deutsche 300-s-Fall verkettet vollständige Quellclips und
maskiert Pausen mit deterministischem Rauschen. Er ersetzt keinen natürlichen
fünfminütigen Monolog. Die Liste `requiredCoveragePending` hält verbleibende
Lücken fest, insbesondere exakte 30-s-Fälle, unabhängiges Anhören, Inhaltsanker, Raumgeräusche
und den echten Mikrofon-/Hotkey-Pfad.

## Messung

`reference-parameters-v1.json` dokumentiert die vollständige vorläufige
Referenzkonfiguration. Kalibrierung erfolgt ausschließlich auf `calibration`.
Parameteränderungen benötigen eine neue dokumentierte Konfiguration vor der
Abnahme; Ergebnisse der Abnahmemenge dürfen die Kalibrierung nicht steuern.

Der Entwickler-Probe akzeptiert ein Manifest mit ausgewählter Fallmenge:

```sh
EVERLAST_BENCHMARK_REVISION="$(git rev-parse HEAD)" cargo run -p everlast-dictation --release --example corpus_probe -- MODEL_PATH reference MANIFEST_JSON OUTPUT_JSON
node --import tsx scripts/quality.ts MANIFEST_JSON OUTPUT_JSON --output REPORT_JSON
```

Für das kleine Modell wird `small` übergeben. Ergebnisse und Metadaten sind neue
lokale Dateien; bestehende Ausgaben werden nicht überschrieben. Fehlende oder
fehlgeschlagene Fälle bleiben sichtbar. Ein erfolgreicher Werkzeugaufruf bedeutet
nicht, dass WER-, Anker- oder Leistungsgrenzen bestanden wurden.

Diese Probe misst den vollständigen Dateibackend-Pfad mit warmem Modelldienst.
Sie misst **nicht** die in §13 definierte Echtzeitlatenz L: Dafür fehlen zeitgetreue
Einspeisung vor Ring/DSP/Worker und Stop-bis-Text-Beobachtung. Auch Mikrofon,
Textübergabe, Speicher-/Abbruchabnahme und die vorgeschriebenen Wiederholungen
müssen separat nachgewiesen werden.

## Rechte und Namensnennung

Werkzeugcode bleibt unter der Projektlizenz. Die Korpusinhalte haben eigene
Lizenzen: FLEURS-Aufnahmen von Alexis Conneau et al./Google unter
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/); die enthaltenen
FLORES-101-Referenztexte von Naman Goyal et al. unter
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
Die im Manifest gespeicherte Referenztextsammlung ist entsprechend unter
CC BY-SA 4.0 weiterzugeben. Heidelberg Digits: Copyright 2019 Benjamin Cramer,
Yannik Stradmann, Johannes Schemmel und Friedemann Zenke, CC BY 4.0.
Quellenrevisionen und Transformationen sind je Fall ausgewiesen; Auswahl,
Formatkonvertierung, Padding, Lautstärkeänderung und konstruierte Fälle stammen
aus diesem Projekt. Details und Primärquellen stehen im verlinkten Quellenbericht.

## Ergänzte Kalibrierung v2

Die unveränderten 34 Kalibrierungsfälle werden um acht Kontrollen ergänzt:
Nullsignal, synthetisches Rauschen mit neuem Seed, Satz plus drei Sekunden
Stille und ein fest vorgewählter natürlicher Büroton, jeweils DE/EN. Die
[CC0-Quelle und ihr Labelvorbehalt](../../../docs/verification/room-noise-source.md)
sind getrennt dokumentiert. Die ursprüngliche Abnahmemenge bleibt unverändert.

```sh
node --import tsx scripts/calibration.ts --source-manifest tests/fixtures/corpus/manifest.json --room-source artifacts/corpus/source/freesound-456207-hq.mp3 --output artifacts/corpus/calibration-v2-ts.json
```

Der Builder prüft Quellmanifest, tatsächliche WAV-Bytes, Raumdatei und Decoder
gegen feste Hashes. Bestehende identische Dateien bleiben erhalten; andere
Dateiinhalte werden nicht überschrieben. Die Parameterdateien dokumentieren
den jeweiligen Codezustand und sind keine vom Probeprogramm eingelesenen
Laufzeitoptionen. Zur Wiederholung einer historischen Messung ist daher auch
die im Messbericht genannte Quellrevision zu verwenden. Die
Kalibrierungsberichte weisen
negative Kontrollresultate ausdrücklich aus; eingefroren bedeutet nicht abgenommen.
