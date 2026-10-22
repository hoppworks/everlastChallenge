# Entwicklerwerkzeuge

Die eigenen aktiven Werkzeuge verwenden TypeScript und laufen mit
`node --import tsx`. Die App verwendet Rust und React/TypeScript; sie benötigt
weder Node noch Python auf dem Zielrechner. Unveränderte Drittanbieterquellen
unter `vendor/` enthalten weiterhin deren ursprüngliche Hilfsprogramme.

| Aufgabe | Einstieg |
|---|---|
| WER und stille Ausgaben auswerten | `scripts/quality.ts MANIFEST RESULTS --output NEW_REPORT` |
| Modellpaare einschließlich menschlicher Inhaltsprüfung vergleichen | `scripts/compare-mlx-quality.ts MANIFEST BASELINE CANDIDATE --reviews REVIEWS --output NEW_REPORT` |
| Historischen Helper-Vergleich nachrechnen | `tools/measurement/summarize-helper-comparison.ts` |
| FLEURS-Korpus erzeugen | `scripts/corpus.ts --source DIRECTORY --manifest NEW_MANIFEST` |
| Natürliche kurze Ziffern ergänzen | `scripts/short-corpus.ts BASE --source ARCHIVE --output NEW_MANIFEST` |
| Kalibrierung ergänzen | `scripts/calibration.ts --source-manifest BASE --room-source AUDIO --output NEW_MANIFEST` |
| Explizites Messkommando mit Prozessgruppenbesitz ausführen | `scripts/controlled-probe.ts NEW_EXISTING_FOLDER REVISION -- COMMAND ARGS...` |
| Ressourcen-Probe ausführen | `scripts/run-runtime-resource.ts BINARY MODEL SELECTION WAV REFERENCE PARAMETERS NEW_OUTPUT` |
| Modell-Chunkpins erzeugen | `scripts/generate-model-chunk-hashes.ts MODEL_DIRECTORY NEW_OUTPUT.rs` |
| Unabhängigen Mel-DFT-Referenzfall erzeugen | `scripts/generate-whisper-mel-oracle.ts FILTERS.npz NEW_OUTPUT.json` |
| Lizenzinventar aus aufgelösten Paketen erstellen | `scripts/update-license-inventory.ts NEW_OUTPUT.json` |

Alle Ausgabepfade müssen neu sein. Korpus-Builder akzeptieren außerdem bereits
vorhandene, byteidentische Dateien. Sie überschreiben keine abweichenden
Ergebnisse. Archive werden begrenzt gestreamt und gegen vollständige Pins
geprüft; ausgewählte Links oder ungewöhnliche Archivmitglieder werden abgelehnt.

## Vergleichbarkeit

Die WER-Normalisierung bleibt auf Unicode 13.0.0 festgelegt. Die Tabellen in
`config/unicode-13.json` halten Groß-/Kleinschreibung, Zeichengruppen und Kontext
für das griechische Schlusssigma fest. Versionsstabile NFC-Normalisierung gilt
nur für damals bereits zugewiesene Zeichen. Gefrorene Vergleichsfälle prüfen
sowohl die Normalisierung als auch einzelne und aggregierte Kennzahlen.

Die drei Mel-Referenzfälle stimmen einschließlich Float32-Eingaben und
Ergebnisarrays exakt mit dem bisherigen DFT-Generator überein. Die 4.271 Byte
große `tests/fixtures/mlx/mel_filters.npz` ist das unveränderte, SHA-256-gepinnte
Filterarchiv aus dem in der Referenzdatei genannten OpenAI-Whisper-Commit.
Es wird nur im Entwickler-Test verwendet; der Herkunftsnachweis und die
OpenAI-Lizenz bleiben erhalten.

Die TypeScript-Builder können JSON anders formatieren als historische Generatoren.
Eine semantisch identische Auswahl ist deshalb kein Beleg identischer
Manifestbytes. Historische Manifeste, Parameter und Hashes bleiben unverändert;
neue Messungen brauchen neue, zu ihren Quellen passende Parameterdateien.
Der Modellpaarvergleich verwendet dafür ausdrücklich Schema 2 und neue
`pairSha256`-Werte. Alte menschliche Review-Quittungen gelten nicht automatisch.

Die früheren einzelnen P06-Kampagnenstarter und ihre nur dafür zuständigen
Adaptertests wurden stillgelegt. Ihre Quellen sind im
[historischen Stand](https://github.com/hoppworks/everlastChallenge/tree/b83d35b/scripts)
erhalten. Alte Messberichte dokumentieren genau diesen damaligen Code und seine
Python-Umgebung. Für neue Messungen gibt es einen gemeinsamen Prozessowner;
er führt nur das ausdrücklich angegebene Kommando aus. Zeitlimit, Abbruch,
Prozessgruppen-Abwesenheit und Stromquelle werden getrennt protokolliert.
Eine fehlgeschlagene oder erzwungen beendete Messung bleibt sichtbar und wird
nicht als qualifiziert ausgegeben. Der Owner ersetzt keine statistische
Versuchsplanung oder inhaltliche Qualitätsprüfung.

## Prüfungen

`npm run check:fast` prüft auch die TypeScript-Werkzeuge strikt und mit denselben
Promise-Regeln wie die Oberfläche. `npm run test:quality` führt ihre Verträge
sowie zwei modellfreie Tests der tatsächlichen nativen VAD-/Wrapperquellen aus.
Die restlichen Testkommandos stehen in `package.json`. Weder Korpusgenerierung
noch ein normaler Werkzeugtest startet automatisch eine ASR-Messkampagne.
