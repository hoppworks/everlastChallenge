# Modellbereitstellung und getrennte Lizenzherkunft

Die aktuelle Bundlekonfiguration enthält Lizenztexte, keine Modellgewichte.
Der Setupdienst lädt nur nach ausdrücklicher Aktion die mit Größe und SHA-256
gepinnten Dateien von den im Code festgelegten Herausgeberrevisionen.
Dies ist keine Behauptung, dass Gewichte bereits im App-/DMG-Artefakt beiliegen.

Der aktive Hybrid lädt `large-v3-turbo-q5_0` und leitet daraus lokal den
F32-Encodercheckpoint ab; die Encoderberechnung verwendet FP16. Zusammen mit
Silero VAD sind dies die Modellbestandteile des aktuellen Produktpfads.
`small-q5_1` ist eine historische Vergleichsvariante und in der App nicht
auswählbar. [Abhängigkeiten](../dependencies.md) erklären Speicherbedarf und
Konvertierung.

Die ASR-Quellen `large-v3-turbo-q5_0` und `small-q5_1` stammen aus den für whisper.cpp
konvertierten OpenAI-Whisper-Modellen beim Herausgeber ggerganov/whisper.cpp,
Revision5359861c739e955e79d9a303bcbc70fb988958b1. Dessen exakt gepinnte
Modellkarte bezeichnet die Modelle als MIT. OpenAI erklärt Code und Gewichte
unter MIT; der hier separat erhaltene unveränderte Text
[openai-whisper-model-MIT.txt](openai-whisper-model-MIT.txt) enthält
Copyright2022OpenAI. Dieser Hinweis ist getrennt von
[whisper.cpp-MIT.txt](whisper.cpp-MIT.txt), dessen Copyright die ggml-Autoren
nennt. Die Herausgeberkarte, genaue Gewichtshashes und Original-Lizenzquelle
einschließlich abgerufenem OpenAI-Commit stehen im
[Komponenteninventar](third-party/components.json).

VAD: silero-v5.1.2 verwendet unverändert den bereits erhaltenen
[Silero-MIT-Text](silero-vad-MIT.txt); Modellrevision, Konverter-/Quellgrenzen
und tatsächlicher Gewichtshash bleiben in dependencies.md und dem VAD-
Quellnachweis dokumentiert. Eine Lizenzzuordnung beweist weder Modellqualität
noch akzeptierte Produkt-/Leistungswerte.

Falls spätere Release-Pakete zusätzlich Gewichtskopien enthalten, müssen diese
Bestandteile im endgültigen Artefaktinventar ausdrücklich erscheinen und ihre
begleitenden Lizenz-/Herkunftshinweise ebenfalls mitgegeben werden.

## Local text refinement model (not yet integrated)

The separate text worker pins `mlx-community/Qwen3-4B-Instruct-2507-4bit` at
`50d427756c6b1b2fe0c0a10f67fbda1fc8e82c1b`. Its shared
[manifest](../../crates/text-refinement-worker/src/model-manifest.json) binds
all 12 assets by size and SHA-256: 2,278,970,666 bytes in total. The installer
publishes only a complete verified set; it is not connected to application setup
yet. These weights are not included in the current app download.

The unmodified [Apache-2.0 license](qwen3-4b-Apache-2.0.txt) and
[conversion model card](qwen3-4b-mlx-model-card.txt) are preserved separately.
Their pinned URLs, hashes and attribution limits are recorded in the
[provenance receipt](third-party/qwen3-model-provenance.json). The card identifies
the base model and conversion tooling, but does not identify the exact base
weight revision used for conversion. The independently pinned license source
does not resolve that gap. Upstream usage examples in the preserved card are
attribution material, not an application dependency or installation instruction.
